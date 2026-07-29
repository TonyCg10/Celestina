# Magnetita — instrucciones de la app

Este directorio contiene sólo el cliente QML/CXX-Qt fino y su packaging.

## Frontera de responsabilidad

- No añadas dominio, protocolo, transporte ni lógica del demonio aquí. Eso vive
  en `../celestina-rs/crates/magnetita-core`, `magnetita-net` y `magnetitad`.
- La UI solicita acciones y refleja únicamente el estado confirmado por el
  demonio; no mantiene una segunda verdad optimista.
- Conserva `org.celestina.Devices1` compatible. Extiende los diccionarios
  `a{sv}` con claves nuevas; no elimines ni cambies métodos, claves o semánticas
  existentes que consumen Siderita y el panel.
- Los componentes compartidos de `celestina-style` se consumen mediante los
  symlinks existentes y se registran en `build.rs`; nunca los copies localmente.

## Servicio delicado

- Para reinstalar `magnetitad`, ejecuta primero
  `systemctl --user stop magnetitad`; sustituye el binario y después usa
  `systemctl --user start magnetitad`. Copiar sobre un proceso vivo puede dejar
  ejecutándose la versión anterior.
- No reinicies el servicio en ráfagas: puede dejar colgada la conexión KDE
  Connect del teléfono.
- No borres `~/.config/magnetita/trust.json`, fuerces un reemparejado ni cambies
  el estado persistente salvo petición explícita del autor.

## Verificación mínima

- Ejecuta primero `bash ../scripts/check-architecture-contract.sh`.
- Para Rust/build: `cargo fmt --check`, clippy con `-D warnings` y
  `cargo build --release --locked`; ejecuta tests del crate afectado en
  `../celestina-rs` cuando cambie dominio o contrato.
- Para QML: confirma registro en `build.rs`, build release y arranque offscreen
  sin `TypeError`/`ReferenceError`. Un timeout vivo prueba arranque, no acciones.
- Cambios de D-Bus o plugins requieren tests del productor y comprobación de los
  consumidores; hardware, pairing y transferencia sólo se declaran verificados
  después de una prueba real con el teléfono.
