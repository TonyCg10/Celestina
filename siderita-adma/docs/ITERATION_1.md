# Siderita — Primera Iteración

> Corte medido el 2026-07-20. La elección Rust + QML/Qt Quick sigue siendo
> provisional hasta medir un staging limpio y el menú sobre Wayland real.

## Qué existe en este corte

El ejecutable nuevo es un host Rust. CXX-Qt genera el borde necesario para
registrar el QObject y el módulo QML; no hay lógica de dominio escrita a mano
en C++. El prototipo C++/CMake anterior continúa en el árbol únicamente como
referencia y no forma parte del host Cargo.

La porción funcional es deliberadamente de solo lectura:

- abre HOME o un path local pasado como primer argumento;
- navega atrás, adelante y arriba, permite volver a HOME o refrescar, y conecta
  los botones laterales Atrás/Adelante del ratón con el mismo historial;
- permite escribir una ruta absoluta, relativa o con `~/` desde la barra de
  ubicación; sólo la incorpora al historial cuando el escaneo tiene éxito;
- ofrece Alt+Izquierda/Derecha/Arriba, Ctrl+L, Ctrl+F, Ctrl+H y F5;
- ordena por nombre, tamaño, modificación o tipo, en ambas direcciones, con
  carpetas siempre primero y selección estable al reordenar;
- escanea en un único worker y publica el resultado en el hilo Qt;
- rechaza tanto snapshots como errores pertenecientes a una navegación vieja;
- filtra por texto con 120 ms de debounce y muestra u oculta dotfiles;
- conserva tokens opacos aunque una fila desaparezca temporalmente por filtro;
- expone loading, vacío, error, selección y estado de navegación;
- permite seleccionar con clic o flechas y entrar en carpetas con doble clic o
  Enter;
- no crea, abre, renombra, copia, mueve ni elimina archivos.

`QStringList` es una adaptación temporal. Una revisión explícita invalida los
bindings auxiliares de token/tipo/subtítulo cuando cambia la proyección. El
siguiente modelo debe ser un `QAbstractListModel` con roles nativos para evitar
duplicar nombres y realizar varios invokables por delegate.

Todavía no están implementados URI `file://`, watcher, breadcrumb segmentado,
apertura de archivos, staging de instalación ni tests de UI.

## Vidrio interno

`GlassContextMenu` sigue siendo un `Menu` real con `MenuItem`, foco, Escape,
flechas y Enter. Usa `Popup.Item` para permanecer dentro de la escena de la
ventana. Su fondo recibe una referencia explícita a `contentLayer`; nunca busca
ni captura la ventana del compositor.

La captura está acotada al rectángulo del popup, se toma una vez al mostrarlo y
usa `ShaderEffectSource` con `live: false`, `recursive: false` y escala 0,66.
Un único `MultiEffect` aplica blur con `blurMax: 24`. El efecto permanece activo
durante las transiciones de entrada y salida, pero no existe trabajo gráfico
continuo con el menú cerrado. Hay un fallback translúcido si el efecto no puede
capturar contenido. En la interfaz I1 el vidrio y el movimiento quedan siempre
activos: se retiraron los interruptores de prueba.

Esto demuestra la arquitectura, no todavía la calidad ni el coste del blur en
el compositor objetivo. El p95 de frame necesita una captura Wayland real.

## Iconografía del sistema

Los controles solicitan iconos mediante nombres estándar freedesktop como
`go-previous`, `go-next`, `go-up`, `go-home`, `view-refresh`, `folder` y
`text-x-generic`. Qt los resuelve con el tema de iconos anunciado por la sesión,
por lo que Siderita puede seguir Adwaita, Breeze, Papirus u otro tema instalado
sin depender de sus bibliotecas ni copiar el pack.

Ocho SVG monocromos mínimos viajan incrustados como fallback. Sólo se usan si el
tema o el entorno aislado no resuelve un nombre; juntos ocupan menos de 2 KiB en
fuentes y evitan botones vacíos. Los glifos Unicode anteriores ya no forman
parte de la navegación ni de las filas.

## Dependencias y límites

El runtime Rust fija CXX 1.0.176 y CXX-Qt 0.9.1, y activa sólo `qt_gui`,
`qt_qml` y `qt_quickcontrols` en `cxx-qt-lib`, con
`default-features = false`. No activa
`full`, HTTP, imágenes, serde, chrono, UUID, Qt Concurrent, WebEngine,
Multimedia ni frameworks KDE/GNOME.

`cxx-gen` está fijado a 0.7.176 porque CXX 1.0.176 y un generador 0.7.198
producen nombres de símbolo incompatibles. La familia se debe actualizar al
unísono, nunca una crate aislada.

El grafo observado contiene 39 paquetes en aristas normales. Al habilitar
`qt-minimal`, normal + build llega a 221 paquetes porque el bootstrap añade el
downloader, TLS y descompresión. Son dependencias de compilación, no del proceso
Siderita. `qt-minimal` sirve para bootstrap/CI; producción debe preferir Qt
compartido por la suite.

La UI necesita Qt 6.8 o posterior por `Popup.Item`; este corte se compiló y
ejecutó con Qt 6.10. El bootstrap descargado ocupó 1.440.152.981 bytes, pero es
un SDK de desarrollo que contiene grandes archivos estáticos de tooling y no
representa el cierre instalado. Tampoco se debe contar `target/` como payload.

El empaquetado deberá usar allowlist. `qmlimportscanner` puede descubrir estilos
Basic, Fusion, Material, Imagine, Universal y otros aunque el host fuerce Basic;
copiarlos todos convertiría una dependencia de build conveniente en bloat real.

## Construcción y comprobaciones

Con un Qt compartido visible para CXX-Qt:

~~~sh
cargo build --release --locked
~~~

Para el bootstrap experimental usado en este entorno:

~~~sh
cargo build --release --locked --features qt-minimal
~~~

Comprobaciones ejecutadas:

~~~sh
cargo test --workspace                         # en ../celestina-rs
cargo check --features qt-minimal
cargo clippy --features qt-minimal --all-targets -- -D warnings
cargo build --release --locked --features qt-minimal
sh -n scripts/measure-i1.sh
~~~

Los 30 tests del workspace Rust pasan. QML cache, bridge, check, Clippy y release
pasan; el release corregido permaneció activo sin warnings durante la prueba
offscreen. El warning genérico de CXX-Qt sobre `ld.bfd` no impidió enlazar este
artefacto.

## Medición preliminar

Artefacto: release stripped, SHA-256
`d58fa8a832494bce9fe0a4f06c61092726d98db903e8921fa39af86e2d553f9a`.
Entorno: Linux x86_64, Qt 6.10 bootstrap, plataforma offscreen y backend
software. Son una ejecución por escenario; no sustituyen tres repeticiones en
Wayland ni contabilizan memoria de GPU.

| Métrica | Presupuesto I1 | Resultado observado | Estado |
|---|---:|---:|---|
| Binario aislado stripped | ≤ 20 MiB de payload instalado | 1.701.152 B (1,62 MiB) | Pasa provisionalmente; falta staging |
| Cierre del primer install Qt | ≤ 250 MiB | Pendiente; no equivale al SDK ni a `ldd` | Pendiente |
| HOME, PSS medio durante 60 s | ≤ 120 MiB | 41.491 kB (40,52 MiB) | Pasa preliminar |
| HOME, CPU de un núcleo durante 60 s | ≤ 1 % | 0,000 % | Pasa preliminar |
| HOME, hilos estabilizados | sin pool por núcleo | 3 | Pasa preliminar |
| Fixture 10k, PSS medio durante 10 s | ≤ 250 MiB | 47.583 kB (46,47 MiB) | Pasa preliminar |
| Fixture 10k, CPU de un núcleo | sin trabajo periódico | 0,000 % | Pasa preliminar |
| Menú blur, frame time p95 | ≤ 16,7 ms | No medido | Pendiente |

El ELF tiene 8 entradas `NEEDED` directas. `ldd` resolvió 43 archivos que suman
87.653.048 bytes; después de cargar QML se observaron 25 archivos dentro del
árbol Qt/ICU, con 70.335.888 bytes de tamaño de archivo. Estas cantidades son
inventarios inferiores del escenario ejercitado, no memoria residente ni cierre
de instalación. `QtNetwork` y `QtOpenGL` aparecen transitivamente aunque
Siderita no implemente red ni una API OpenGL propia.

## Protocolo reproducible

El script no inicia ni mata la aplicación. Se adjunta a un PID ya estabilizado:

~~~sh
scripts/measure-i1.sh BINARY [PID [SEGUNDOS]]
~~~

Variables opcionales:

- `SIDERITA_SCENARIO`: etiqueta del caso medido;
- `SIDERITA_APP_STAGE`: staging exclusivo de la app;
- `SIDERITA_CLOSURE_STAGE`: staging del primer cierre de instalación;
- `SIDERITA_QT_ROOT`: raíz Qt para separar archivos mapeados.

Reporta tamaño y hash, `NEEDED`, `RPATH/RUNPATH`, `ldd`, PSS/RSS, CPU por delta
de ticks, hilos, context switches como proxy e inventario único de archivos
mapeados. Los context switches no se rotulan como wakeups. Frame p95 queda fuera
del script y debe obtenerse con QML Profiler o trazas de frames sobre Wayland.

## Criterio de decisión

Los números de CPU y memoria justifican continuar el experimento; todavía no
ratifican Qt/QML para el resto de la suite. La decisión se toma sólo después de:

1. instalar un staging allowlisted con únicamente Basic y los plugins usados;
2. medir tres veces HOME, 10k y menú blur on/off en la misma sesión Wayland;
3. verificar teclado, contraste, animaciones e iconos temáticos de forma
   interactiva;
4. comparar cierre del primer install y coste marginal al compartir Qt;
5. sustituir `QStringList` por un modelo con roles y volver a medir.
