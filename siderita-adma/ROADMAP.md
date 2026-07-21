# Roadmap de Siderita

## Ahora — Primera iteración (I1/S0): slice veraz, visual y medible

**Resultado:** una instalación staging abre HOME, un path o una URI local en
una vista moderna de solo lectura; un menú contextual demuestra glass interno
real y el informe de recursos permite ratificar o descartar Qt/QML con datos.

### Estado del corte 2026-07-20

Ya existen el host Rust, QObject CXX-Qt, worker único, navegación HOME/path,
filtro, ocultos, selección estable, estados veraces y el menú glass con fallback
y animaciones breves fijadas para I1. El release aislado y los escenarios
HOME/10k están medidos en offscreen; véase
[docs/ITERATION_1.md](docs/ITERATION_1.md).

Siguen abiertos el staging, URI local, watcher, breadcrumb real, modelo Qt con
roles, validación interactiva y frame p95 blur on/off. Por
eso Qt/QML continúa marcado como **Primera Iteración provisional**.

### Incluye

- `celestina-core`, `siderita-core` y `siderita-qt` neutrales hasta el borde Qt,
  con PathBuf/OsString, EntryId, generaciones y tokens opacos;
- un executor de escaneo acotado con cancelación y cierre determinista;
- Qt Quick/QML como UI y un adaptador CXX-Qt mínimo, ambos provisionales en I1;
- una capa de contenido separada del overlay y un `GlassSurface` compartible;
- menú `Popup.Item` con captura, blur interno, tinte, borde y fallback;
- HOME, path y file URI; volver, avanzar, subir y breadcrumb;
- filtro, orden por nombre/tamaño/fecha/tipo, ocultos, selección estable y
  refresh;
- loading, vacío, error y watcher degradado claramente visibles;
- transiciones breves que queden completamente inactivas en reposo;
- inventario de dependencias y baseline de tamaño, memoria, CPU, threads,
  wakeups, arranque y frame time con blur activado y desactivado.

### Excluye

- crear, renombrar, copiar, mover, pegar, trash y borrar;
- abrir/ejecutar archivos, DnD operativo y clipboard de archivos;
- montajes, cross-filesystem, tabs, previews, thumbnails y búsqueda global;
- editor, visor, reproductor o cualquier lógica de otra app;
- Qt Concurrent, WebEngine, Multimedia y frameworks KDE/GNOME;
- optimizar benchmarks sintéticos antes de tener el slice instalado.

### Trabajo

1. Crear un baseline Git recuperable y declarar toolchains Rust/Qt reproducibles.
2. Fijar el contrato versionado de `celestina-rs` y exponer un QObject CXX-Qt
   mínimo, sin trasladar dominio ni IO a C++.
3. Completar watcher y shutdown sobre las primitivas Rust ya iniciadas.
4. Construir una sola vista primaria y sus estados veraces; mantener lista
   compacta como opción provisional.
5. Crear el módulo visual versionado y demostrar `GlassSurface` en el menú
   contextual sin capturarse a sí mismo, con animaciones y fallback.
6. Añadir fixtures vacío/normal/1k/10k y fallos de watch/path; A→B y ráfagas ya
   tienen cobertura del core.
7. Generar release staging, inventariar el grafo dinámico/QML y medir en la
   misma máquina tres veces después de estabilizar cada escenario.
8. Registrar el resultado: ratificar Qt/QML o reabrir el frontend antes de S1.

### Presupuesto provisional de I1

- payload exclusivo de Siderita release, stripped e instalado: **máximo 20 MiB**;
- cierre atribuible del primer install Qt sobre una imagen base: **máximo 250 MiB**;
- HOME en reposo durante 60 s: **máximo 120 MiB PSS y 1 % de CPU promedio**;
- fixture de 10k entradas estabilizada: **máximo 250 MiB PSS**;
- ningún escaneo periódico, animación decorativa perpetua ni pool Rust
  proporcional al número de núcleos;
- apertura del menú con blur a 60 Hz: frame time p95 objetivo **≤ 16,7 ms** en
  el equipo de referencia, comparado también con blur desactivado.

PSS evita contar varias veces las bibliotecas compartidas. El informe separa
payload propio, módulos Qt compartibles y cierre total; no se justificará una
regresión diciendo únicamente que el uso de toda la suite es “marginal”. Los
límites se pueden endurecer con evidencia; para aflojarlos hace falta una
decisión explícita con el beneficio visual o funcional demostrado.

### Salida

- build/test/install parten de un entorno declarado y no usan paths hermanos
  en un release;
- los tests de core cubren generaciones, cancelación, ráfagas, hardlinks,
  nombres no UTF-8 y tokens;
- HOME/path/URI muestran loading, snapshot o error recuperable correctamente;
- A nunca se publica después de navegar a B y watch perdido degrada de forma
  visible;
- homónimos, hardlinks y nombres no UTF-8 no colapsan identidad;
- teclado, vidrio, animaciones e iconos del tema funcionan sobre 1k items;
- el menú desenfoca contenido de la app, no una copia de sí mismo, y conserva
  contraste mediante su superficie translúcida de fallback;
- el inventario no contiene dependencias fuera del allowlist sin justificación;
- todas las cifras del presupuesto quedan anexadas al artefacto medido y cada
  límite pasa, o Qt/QML queda marcado como no ratificado;
- cerrar con trabajo activo no deja threads ni callbacks tardíos.

### Presupuesto: lectura preliminar

| Métrica | Límite | Corte actual |
|---|---:|---:|
| Binario stripped aislado | 20 MiB | 1,62 MiB; staging pendiente |
| Primer cierre de instalación | 250 MiB | Pendiente |
| HOME PSS medio, 60 s | 120 MiB | 40,52 MiB, una ejecución offscreen |
| HOME CPU de un núcleo, 60 s | 1 % | 0,000 %, una ejecución offscreen |
| 10k PSS medio | 250 MiB | 46,47 MiB, una ejecución offscreen |
| Menú blur p95 | 16,7 ms | Pendiente en Wayland real |

## Después — S1: operaciones sin pérdida silenciosa

Crear, renombrar, copiar, mover y enviar a Trash solo sobre fixtures
desechables, con preflight, conflictos, cancelación y resultado por item. Un
origen nunca se borra tras copia parcial o sin revalidación.

## Más adelante — S2: gestor diario interoperable

Añadir restore XDG, cross-filesystem, volúmenes, handlers seguros, DnD,
FileManager1, accesibilidad completa y presupuestos de uso diario.

## No objetivos

No añadir cloud/red, indexador global, archive VFS, plugins, IDE, terminal,
daemon de suite ni features de otras aplicaciones antes de completar el gestor
local declarado.
