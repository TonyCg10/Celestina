# celestina-rs — instrucciones del workspace

Este archivo complementa las reglas del repositorio para todo el workspace Rust.

## Límites y dependencias

- Mantén los crates de dominio independientes de Qt, QML, Niri y de las apps.
- Las dependencias apuntan hacia dentro: aplicación/puente → IO, operaciones o
  transporte → core. En particular, `magnetita-core` no conoce red ni D-Bus;
  `magnetita-net` posee el transporte y `magnetitad` la integración del sistema.
- Añade una dependencia sólo en la capa que realmente la necesita y documenta
  en `Cargo.toml` por qué merece entrar en el cierre.

## Seguridad y corrección

- Usa errores tipados con `Display` y `source`; no conviertas fallos esperables
  en `panic!`, `unwrap` o `expect` de producción.
- Acota toda lectura y tamaño declarado por red. Sanea cualquier texto remoto
  antes de convertirlo en nombre de archivo o ruta.
- En operaciones de archivos, nunca elimines el origen antes de verificar el
  destino. Un fallo o cancelación revierte cualquier destino parcial.

## Verificación

- Todo comportamiento nuevo incluye pruebas en el mismo cambio.
- Antes de cerrar trabajo en este árbol ejecuta:

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

