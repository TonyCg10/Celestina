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
- `Main.qml` recibe `reducedMotion` del host y lo publica en
  `CelestinaTheme.reducedMotion`. Toda animación nueva o modificada queda
  instantánea o desactivada en ese modo; no añadas un segundo flag local. No
  confundas esta ruta implementada con una auditoría completa de movimiento
  heredado o con validación interactiva real.

## Superficie integrada de Grafita

- Siderita consume `grafita-core`; no duplica buffer, undo, clasificación de
  texto, conflicto ni guardado, y no importa QML de la aplicación Grafita.
- `Espacio` prueba el contenido fuera del hilo GUI: texto editable abre el modal
  simple de Grafita; los demás tipos conservan Quick Look. Doble clic/Enter en
  texto lanza la aplicación Grafita completa; no reutiliza el modal.
- El modal integrado sólo adapta estado/acciones del core. Bloquea la carpeta,
  contiene y restaura foco, y no puede cerrarse sucio sin Guardar, Descartar o
  Cancelar.
- Ningún sondeo, lectura o guardado de documento corre en el hilo Qt. Respuestas
  de worker llevan generación/revisión y una respuesta obsoleta no publica
  estado ni limpia cambios nuevos.

## Superficie integrada de Fluorita

- Siderita consume `fluorita-core`/`fluorita-engine`; no duplica catálogo,
  playback, extracción de artwork ni reglas de trailer, y no importa QML de la
  aplicación Fluorita.
- `Espacio` sobre imagen/vídeo/audio abre el player mínimo; doble clic/Enter
  abre la aplicación completa y comienza ese item. Los demás tipos conservan el
  flujo de Grafita o Quick Look que les corresponda.
- Navegar sólo lee thumbnails/covers estáticos. El engine pesado se carga de
  forma perezosa para una petición explícita y mantiene como máximo una preview
  viva por host; cambiar selección o cerrar cancela y libera su sesión.
- Un thumbnail es PNG freedesktop estático. Un tráiler es reproducción corta y
  cancelable, nunca un fichero publicado fingiendo cumplir ese estándar.
- El modal bloquea la carpeta, contiene/restaura foco y publica sólo estado
  confirmado por el engine; Play/Pause/Seek solicitados siguen pendientes hasta
  confirmación.

## Matriz mínima de verificación

| Cambio | Evidencia obligatoria |
| --- | --- |
| Rust de dominio | Tests en el crate de `celestina-rs`; `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace` allí. |
| Puente Rust/C++ o `build.rs` | Formato, clippy y tests de Siderita; `cargo build --release --locked`; comprobar registro/QRC. |
| QML no visual | `bash ../scripts/check-architecture-contract.sh`, `qmllint` con los import paths del build actual, `cargo build --release --locked` y `scripts/smoke.sh`; registra el comando exacto de lint usado. |
| Puntero de una superficie que flota sobre el contenido | Lo anterior más `scripts/qml-tests.sh`: un caso en `tests/qml` que pulse los tres botones, pase el cursor y barra sobre la caja y compruebe que nada llega al contenido de debajo. Leer el árbol no prueba entrega de eventos. |
| Apariencia, foco, modal o movimiento | Lo anterior más inspección de la superficie real; teclado completo, modal abierto y `reducedMotion` encendido/apagado. Una captura offscreen no valida blur ni interacción. |
| API compartida | Cumplir además la matriz de `../celestina-style/AGENTS.md` y reconstruir/probar Siderita como consumidor. |
