# Grafita — delta local

El `AGENTS.md` de la raíz sigue aplicando. Este archivo añade las reglas del
editor independiente y de su contrato compartido con Siderita.

## Fronteras

- `../celestina-rs/crates/grafita-core` es la única fuente de verdad para
  clasificación de texto, documento, posiciones, selección, edición, undo/redo,
  savepoint, conflicto y guardado. No contiene Qt, QML ni decisiones de una UI.
- `grafita/src/` adapta el core a CXX-Qt y coordina workers; `grafita/qml/`
  compone la aplicación completa. Siderita mantiene otro adaptador y una UI
  simple propios: ninguno importa QML del otro ni copia reglas del core.
- La máquina de estados abrir/editar/guardar/cerrar —incluida la caducidad por
  generación y revisión— vive en `grafita-core::session`. Un host no la
  reimplementa: pide un `Outcome`, ejecuta su `Job` en el worker y actúa sobre
  su `Event`. Lo único que cada host añade es el marshaling Qt y el texto que
  ve la persona: los mismos resultados tipados se redactan distinto en un modal
  dentro del gestor de archivos y en un editor que se llama a sí mismo.
- Extensión y MIME pueden elegir icono o resaltado, nunca decidir si el archivo
  es texto. La prueba canónica es por contenido y encoding en `grafita-core`.
- Grafita abre documentos, no proyectos. No introducir árbol de archivos,
  builds, debugger, LSP, terminal ni sistema de plugins.

## Datos y guardado

- Los bytes, terminadores de línea y encoding detectado pertenecen al documento.
  No normalizar silenciosamente al abrir o guardar.
- Un flujo de bytes que no pueda mapearse reversiblemente no se presenta como
  editable. Ampliar encodings exige selección explícita, nunca heurística
  estadística.
- Guardar sigue el contrato del ROADMAP: temporal hermano, metadata
  reproducible, `fsync`, revalidación de identidad y rename atómico sobre el
  objetivo resuelto. Toda negativa previa al rename conserva el original.
- Sondeo, lectura, `stat` y guardado nunca bloquean el hilo GUI. Workers son
  acotados, propios, cancelables cuando corresponda y unidos al cerrar.
- Resultados de apertura llevan generación y resultados de guardado llevan la
  revisión escrita. Una respuesta obsoleta no sustituye el documento actual ni
  limpia una edición posterior.

## Dos superficies

- En Siderita, `Espacio` sobre texto editable abre el editor modal integrado;
  doble clic/Enter abre la aplicación Grafita completa. No intercambiar estas
  acciones ni convertir Quick Look entero en Grafita.
- La UI integrada sí edita y guarda, pero limita su chrome a documento, estado,
  undo/redo, guardar y cierre protegido. Tabs y configuración pertenecen a la
  aplicación independiente cuando el roadmap los autorice.
- Un modal sucio no desaparece: ofrece Guardar, Descartar y Cancelar, contiene
  el foco, bloquea la superficie inferior y restaura el foco al cerrar.

## Verificación mínima

- Ejecuta primero `bash ../scripts/check-architecture-contract.sh`.
- Core: `cargo fmt --all --check`, Clippy del workspace con `-D warnings` y
  tests completos de `celestina-rs`, además de fixtures textuales sin depender
  de extensión.
- Host Grafita: registro QML, `qmllint`, build, smoke y prueba offscreen; teclado,
  foco, IME, apariencia y accesibilidad requieren una sesión Wayland real.
- Consumidor Siderita: su matriz local completa, incluyendo build/smoke y prueba
  real de `Espacio` frente a doble clic/Enter.

