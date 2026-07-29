# Siderita — delta local

El `AGENTS.md` de la raíz sigue aplicando. Este archivo añade únicamente las
reglas propias de la app CXX-Qt/QML.

## Fronteras y composición

- `src/` contiene estado de UI, puentes CXX-Qt e integración del escritorio. La
  lógica de dominio que pueda probarse sin Qt vive en `../celestina-rs`; QML no
  decide operaciones de archivos ni duplica reglas del dominio.
- `cpp/` solo cubre huecos concretos de cxx-qt y cada shim explica por qué es
  necesario.
- `Main.qml`, `PickerWindow.qml` y `qml/views/` coordinan. Una región, delegate,
  diálogo o menú con responsabilidad propia se extrae a un componente nombrado
  con API estrecha: propiedades requeridas y señales; no alcanza ids ajenos ni
  recibe la ventana/controlador completos si bastan datos o acciones puntuales.
- Todo QML añadido, renombrado o eliminado se refleja en la única lista
  `QML_FILES` de `build.rs`. `CelestinaTheme` y `CelestinaIcons` conservan su
  registro singleton; QML y QRC deben quedar también bajo `rerun-if-changed`.
- Un componente de `celestina-style` se consume mediante symlink relativo al
  archivo canónico, nunca mediante copia, y se registra junto con sus QRC.

## Interacción verificable

- Usa primero los controles compartidos. Un control local interactivo debe tener
  paridad ratón/teclado, rol/nombre/acción `Accessible` y un `visualFocus`
  inequívoco cuando el foco procede del teclado.
- Un modal bloquea puntero y atajos de la vista subyacente, mueve y contiene el
  foco, ofrece cancelar/Escape cuando corresponda y restaura el foco al cerrar.
- `CelestinaTheme.reducedMotion` aún es deuda de STYLE-1. Antes de añadir o
  modificar una animación, aterriza ese contrato compartido y haz que quede
  instantánea o desactivada en modo reducido; no finjas que el token ya existe.

## Matriz mínima de verificación

| Cambio | Evidencia obligatoria |
| --- | --- |
| Rust de dominio | Tests en el crate de `celestina-rs`; `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace` allí. |
| Puente Rust/C++ o `build.rs` | Formato, clippy y tests de Siderita; `cargo build --release --locked`; comprobar registro/QRC. |
| QML no visual | `bash ../scripts/check-architecture-contract.sh`, `qmllint` con los import paths del build actual, `cargo build --release --locked` y `scripts/smoke.sh`; registra el comando exacto de lint usado. |
| Apariencia, foco, modal o movimiento | Lo anterior más inspección de la superficie real; teclado completo, modal abierto y `reducedMotion` encendido/apagado. Una captura offscreen no valida blur ni interacción. |
| API compartida | Cumplir además la matriz de `../celestina-style/AGENTS.md` y reconstruir/probar Siderita como consumidor. |
