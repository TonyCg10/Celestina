# Fluorita — delta local

El `AGENTS.md` de la raíz sigue aplicando. Este archivo añade las reglas de la
biblioteca/reproductor y de su superficie mínima dentro de Siderita.

## Fronteras

- `../celestina-rs/crates/fluorita-core` posee identidad y tipo de media,
  catálogo/proyecciones, capacidades, estado de reproducción confirmado,
  generaciones y contratos de artwork/preview. No contiene Qt ni decodificación.
- `../celestina-rs/crates/fluorita-engine` posee metadata/índice IO, decode,
  reproducción y recursos derivados detrás de una interfaz estrecha y medida.
- `fluorita/src/` adapta los crates a CXX-Qt; `fluorita/qml/` compone Galería,
  Música y reproducción completa. Siderita mantiene otro adaptador y un modal
  mínimo: ninguno importa QML del otro ni duplica reglas del core/engine.
- Galería y Música son proyecciones de raíces locales configuradas, no un gestor
  de archivos ni permiso para rastrear todo el sistema.

## Recursos derivados

- Un thumbnail freedesktop siempre es PNG estático: imagen reducida, frame de
  vídeo o cover embebido. Se publica atómicamente con la clave/metadata que ya
  consume Siderita.
- Un tráiler de vídeo es una preview viva, corta, bajo demanda y cancelable; no
  se escribe fingiendo ser un thumbnail estándar. Sólo uno por host puede estar
  activo salvo evidencia y autorización posteriores.
- Navegar listas no inicializa el backend pesado. Decode/playback arranca sólo
  al solicitar player, trailer o extracción pendiente y se cierra de forma
  determinista.
- Ningún frame, cover o trailer obsoleto puede publicarse tras cambiar fuente,
  selección o identidad del archivo; cada trabajo lleva generación y valida
  source identity/mtime.

## Dos superficies

- En Siderita, `Espacio` sobre imagen/vídeo/audio abre Fluorita mínima; doble
  clic/Enter abre la aplicación completa y comienza ese item.
- La superficie mínima muestra sólo contenido, estado honesto, transporte
  soportado, seek/volumen cuando apliquen y cierre. Galería, Música, fuentes y
  configuración pertenecen a la aplicación independiente.
- El modal bloquea la carpeta, contiene/restaura foco y cancela/cierra su sesión
  al salir. Un click de Play/Pause/Seek es pendiente hasta confirmación del
  engine.

## Seguridad y rendimiento

- Metadata, nombres, dimensiones, duración y contenido son entrada hostil:
  aplica límites de bytes, pixels, tiempo, profundidad y cantidad antes de
  reservar o decodificar.
- Scans son acotados, cancelables, incrementales y fuera del hilo GUI. Quitar un
  item del catálogo nunca borra su archivo fuente.
- Una dependencia multimedia pesada entra sólo tras el spike medido y aprobación
  del autor, con justificación inline en `Cargo.toml`.
- `unsafe` sigue prohibido; un hueco de render/FFI exige la excepción previa y
  aislada que manda el contrato raíz, no un `allow` local.

## Verificación mínima

- Ejecuta primero `bash ../scripts/check-architecture-contract.sh`.
- Core/engine: fmt, Clippy del workspace con `-D warnings`, tests completos y
  fixtures límite/cancelación/staleness.
- Host Fluorita: registro QML, `qmllint`, build, smoke y offscreen; playback,
  frame pacing, teclado, foco y apariencia requieren Wayland real.
- Consumidor Siderita: su matriz completa y una prueba real que distinga
  `Espacio` de doble clic/Enter sin cargar el engine durante navegación normal.
