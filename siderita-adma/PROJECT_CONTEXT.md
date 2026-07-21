# Siderita

> Verdad operativa revisada el 2026-07-20. El trabajo próximo vive en
> [ROADMAP.md](ROADMAP.md).

## Propósito

Siderita es el administrador de archivos independiente de la suite Celestina:
moderno, minimalista, fácil de usar y coherente con el lenguaje glassmorphic
del entorno, pero instalable y usable fuera de la sesión propia.

Su responsabilidad es navegar, organizar y recuperar archivos locales y
removibles. No incorpora editor, visor, reproductor, panel ni gestor de
dotfiles; se comunica con ellos mediante estándares de escritorio.

## Estado actual

- La rama master no tiene commits y todo el producto está sin seguimiento.
- El prototipo C++/Qt anterior se eliminó del repositorio (respaldado aparte);
  el host Rust del `qml/i1` es ahora la única implementación.
- El workspace independiente [celestina-rs](../celestina-rs/README.md) contiene
  `celestina-core`, `siderita-core`, `siderita-qt` y
  `celestina-dotfiles-core`; formato, Clippy y todos sus tests pasan con Rust
  1.85.1.
- El core ya preserva identidad Unix/no UTF-8, rechaza generaciones viejas y
  aporta navegación, filtro, orden, invalidación de watcher y un executor
  acotado con cancelación/join.
- El host Cargo, el QObject CXX-Qt y el módulo `qml/i1` ya forman un slice de
  solo lectura: HOME/path, historial, subir, refresh, filtro, ocultos, estados,
  selección estable y un worker de escaneo.
- El entorno base no aporta Rust ni Qt de desarrollo. La integración se validó
  con Rust 1.85.1 y Qt 6.10 temporales, sin instalarlos en el sistema.
- Los 30 tests de core, check, Clippy, generación QML, release y arranque
  offscreen pasan. No existen todavía tests de UI, CI ni staging de instalación.
- El host Rust del `qml/i1` es autónomo: no importa CelestinaStyle y define sus
  tokens de diseño en `qml/i1/CelestinaTheme.qml`.
- `qml/i1/GlassContextMenu.qml` implementa la captura interna acotada y sin
  render continuo. La inspección interactiva y frame p95 en Wayland siguen
  pendientes antes de ratificar el stack.

## Comandos

~~~sh
scripts/run-i1.sh                                      # compila y ejecuta el host I1 en Wayland
cargo build --release --locked                         # con Qt compartido visible a cxx-qt
cargo build --release --locked --features qt-minimal   # bootstrap de Qt (CI/sin Qt del sistema)
desktop-file-validate siderita-adma.desktop
~~~

Requieren Rust y Qt de desarrollo, ausentes en el entorno base; véase
[docs/ITERATION_1.md](docs/ITERATION_1.md).

## Mapa actual

| Ruta | Responsabilidad |
|---|---|
| src/main.rs / controller.rs | host Rust y QObject de la Primera Iteración |
| qml/i1/ | UI I1, tokens de diseño y menú glass medible |
| qml/i1/CelestinaTheme.qml | tokens: color, radio, tipografía, motion y glass |
| scripts/run-i1.sh | compila y ejecuta el host I1 |
| scripts/measure-i1.sh | inventario ELF y medición de proceso Linux |
| docs/ITERATION_1.md | corte implementado, dependencias y resultados medidos |
| ../celestina-rs/crates/siderita-core | dominio Rust de solo lectura |
| ../celestina-rs/crates/siderita-qt | contrato estable para la futura capa QML |

El núcleo vive fuera de este repositorio para que otras aplicaciones consuman
crates versionados sin compartir internals de Siderita.

## Riesgos confirmados

- El prototipo C++ anterior (pérdida silenciosa en copy/move, truncado en
  create, identidad por nombre mostrado, navegación async sin generaciones,
  jobs sin cancelación/join) se eliminó; esos riesgos ya no viven en el árbol.
  El núcleo Rust los evita por diseño: tokens opacos, generaciones,
  cancelación/join y solo lectura en I1.
- Qt puede aumentar mucho el cierre instalado o mantener trabajo gráfico
  innecesario si no se limita y mide desde el primer slice.

## Decisiones y límites

| Decisión | Motivo |
|---|---|
| Siderita conserva roadmap y release propios | La suite integra contratos, no internals |
| Familia de crates Rust en un workspace independiente | Cada dominio se prueba sin toolkit y las apps conservan releases propios |
| **Primera iteración provisional:** Rust + Qt Quick/QML + CXX-Qt mínimo | QML ofrece efectos internos, animación, controles y accesibilidad maduros; se ratifica sólo si supera el presupuesto medido |
| C++ queda reducido al código generado por el bridge | Host, dominio, IO, estado y coordinación pertenecen a Rust |
| Vidrio interno vive en QML | Menús y paneles usan `Popup.Item`, una captura del contenido inferior y `MultiEffect`, con fallback translúcido |
| Iconos por nombres freedesktop y fallback SVG mínimo | La suite respeta el tema global sin depender de KDE/GNOME ni empaquetar un icon pack completo |
| Blur exterior de la ventana es opcional | Niri puede decorarlo, pero no sustituye el efecto de los objetos dentro de la app |
| Dependencias Qt bajo allowlist | Partir de Core/Gui/Qml/Quick/QuickControls2 y `QtQuick.Effects`; no añadir Concurrent, WebEngine, Multimedia, KDE o GNOME sin una necesidad y una medición |
| La suite amortiza el runtime, no el despilfarro de cada app | Se miden por separado cierre del primer install, payload exclusivo y coste marginal por aplicación |
| El lenguaje visual y de movimiento diferencia a Siderita | La funcionalidad de un gestor de archivos es esperada; la experiencia debe sentirse propia |
| S0 es estrictamente de solo lectura | Primero hay que eliminar estado viejo e identidad ambigua |
| Nombre visible nunca es identidad | Homónimos, rename y nombres Unix no UTF-8 deben conservarse |
| Watcher invalida y rescan manda | Las notificaciones pueden perderse o agruparse |
| Nunca borrar origen antes de verificar destino | Evita pérdida silenciosa en moves cross-filesystem |
| Integración por XDG/freedesktop | URI, MIME, Trash, DnD y handlers no son APIs privadas de la suite |
| Estilo moderno compartido por contrato | Tokens, componentes y assets versionados con fallback; nunca importar otro source tree |

Qt/QML deja de ser la opción de la siguiente iteración si el slice excede los
límites de [ROADMAP.md](ROADMAP.md), introduce módulos no justificados o exige
trabajo continuo en reposo. En ese caso se mide el mismo slice con una UI más
ligera antes de ampliar funcionalidad.

## Decisión abierta inmediata

La lista compacta queda elegida para I1. La decisión abierta es si Qt/QML supera
el staging allowlisted y la comparación blur on/off sobre Wayland real. Antes de
ampliar funcionalidad también se reemplazará `QStringList` por roles nativos;
cuadrícula, thumbnails y doble panel permanecen fuera de S0.
