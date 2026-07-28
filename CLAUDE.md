# CLAUDE.md — reglas de trabajo para agentes en este monorepo

Celestina es la suite de escritorio personal de un solo autor (sesión
Niri/Wayland). Hoy son **dos apps reales** que prueban la tesis del monorepo —
apps que se integran compartiendo crates y estilo sin reescribirse:

- **Siderita** — gestor de archivos (v1.0.x, en uso diario).
- **Magnetita** — enlace de móvil, KDE Connect v8 desde cero (v1.0.x).

`celestina/` (el shell) es un stub mínimo **sin valor todavía**: no invertir ahí
salvo petición explícita del autor. `fluorita/` y `grafita/` son **solo
contratos escritos** (README + ROADMAP, cero código): jamás escribir código en
ellos — escribir el contrato no es empezar el proyecto.

## Mapa — y la trampa estructural

- `celestina-rs/` — **todo el dominio Rust** de la suite (workspace, 8 crates),
  incluido el 80 % de Magnetita: `magnetita-core` (protocolo puro),
  `magnetita-net` (UDP/TCP/TLS), `magnetitad` (el demonio). Cualquier trabajo
  "en Magnetita" casi siempre toca estos crates, no `magnetita/`.
- `magnetita/` — solo la app QML fina (cliente D-Bus del demonio) + packaging.
- `siderita/` — la app CXX-Qt: `src/` puente e integración D-Bus/XDG, `qml/`
  vistas, `cpp/` shims para huecos de cxx-qt, `scripts/run.sh` (build + install).
  **Es un package fuera del workspace**: no hereda sus lints; mantiene los suyos.
- `celestina-style/` — el lenguaje visual: `CelestinaTheme.qml` (tokens) +
  componentes compartidos.
- Contrato entre apps: D-Bus **`org.celestina.Devices1`** (bus
  `org.celestina.Magnetita`), servido por `magnetitad`, consumido por la app
  Magnetita, Siderita y el panel. Para ampliarlo, añade claves al dict `a{sv}`
  (es extensible por diseño); no rompas métodos existentes.

## Dónde va el código nuevo

- Lógica de dominio testeable sin UI → un crate de `celestina-rs`, con tests
  inline. Los cores no importan tipos Qt/QML/Niri.
- Estado de UI y puentes → `src/` de la app (CXX-Qt). Presentación → `qml/`.
- C++ a mano solo para lo que cxx-qt no cubre, con comentario justificando por
  qué (patrón `siderita/cpp/entrymodel.h`).
- Toda dependencia de terceros nueva lleva comentario de justificación en su
  `Cargo.toml` (convención existente). Vetadas sin necesidad medida: tokio,
  openssl, Qt Concurrent/WebEngine/Multimedia, librerías KDE/GNOME.

## Reglas Rust (el repo hoy cumple todas; no bajar el listón)

- `unsafe` prohibido (`forbid` en el workspace). No añadir `#[allow]`: hoy hay
  cero en todo el repo, igual que TODO/FIXME/HACK.
- Cero `unwrap`/`expect`/`panic!` en rutas de producción. Únicas excepciones:
  `.lock().unwrap()` de mutex, y `.expect()` demostrablemente infalible **con
  comentario que lo pruebe**.
- Errores tipados (enum + `Display` + `source`). D-Bus siempre best-effort: un
  bus caído degrada a vacío, nunca tumba ni bloquea.
- Escrituras a disco = pérdida cero: **nunca borrar el origen antes de
  verificar el destino**; rollback de destinos parciales en fallo o cancelación
  (patrón `siderita-ops`).
- Entrada de red hostil por defecto: toda lectura con tope de bytes, todo
  string de red usado como ruta se sanea (patrón `safe_filename` en
  `magnetitad`), tamaños declarados por el par con techo de cordura.
- Un feature aterriza con sus tests `#[cfg(test)]` en el mismo cambio.

## Modularidad — nada de monolitos

- Un archivo no nace para crecer sin techo: pasadas **~800 líneas** (Rust o
  QML), se extrae antes de añadir — un componente QML por región de UI con API
  mínima (propiedades + señales, sin alcanzar ids de fuera), un módulo Rust por
  responsabilidad (patrón existente: `siderita/src/controller/{scan,fileops,trash}.rs`).
- El bloque `#[cxx_qt::bridge]` es la excepción inevitable (cxx-qt 0.9 exige
  declarar propiedades e invocables en un solo bloque): el *contrato* puede ser
  largo, pero la *lógica* jamás vive dentro — cada invocable delega en un
  módulo.
- Un QObject no acumula dominios sin fin: si una superficie nueva no comparte
  estado con el controlador, va en su propio QObject (precedente:
  `FileManager1Service` y `FileChooserPortal` son objetos aparte).
- Todo componente QML nuevo se registra en el `build.rs` de su app
  (`QML_FILES`) — un QML sin listar compila "bien" sin llegar al binario.
- Al instanciar un componente, la propiedad inyectada **no puede llamarse
  igual** que el id que le pasas: `sortMenu: sortMenu` resuelve a la propia
  propiedad del componente (undefined), no al id — la clase de bug del fix de
  clics (9e19b6d). Usa nombres distintos o un alias del host
  (`property alias viewTopBar: topBar`). `siderita/scripts/smoke.sh` y la CI
  cazan el patrón.
- Deuda nombrada: `siderita/qml/FolderView.qml` (~1,6k) y
  `siderita/qml/Sidebar.qml` (~1,3k) siguen sobre el techo (~800). Prohibido
  crecerlos; al tocarlos, extrae primero la pieza que vas a tocar.

## Estilo y UI — cómo no volver a fabricar un botón ilegible

- **Prohibido escribir colores en QML de apps** (ni hex ni nombres). Todo color
  sale de `CelestinaTheme`. Única excepción documentada: el relleno-máscara
  interno de `GlassSurface`.
- Los tokens funcionan **en pares superficie → tinta (foreground)**. El acento
  es **azul One UI `#387aff`**, solo para elementos interactivos/activos
  (seleccionado, checked, enlaces, botón primario); nunca como blanco decorativo.
  Cada superficie opaca lleva su token de tinta explícito; los lavados
  translúcidos comparten `text`/`textMuted`. Pares correctos:
  - `canvas` / `card` / `elevated` → su tinta `canvasInk` / `cardInk` /
    `elevatedInk` (todas ≈ `text`); `surface*` / `controlFill` / `inputFill` /
    `badgeFill` / cristal (`glassTint`) → `text` (secundario: `textMuted`)
  - `accent` → **`accentInk`** (así lo hace `CelestinaButton` primary; el viejo
    par `accent → canvas` MURIÓ con el acento blanco)
  - `danger`/`success`/`warning` como fondo sólido → `dangerInk`/`successInk`/
    `warningInk`; banner `dangerFill` → `dangerFillInk`
  - Nomenclatura: el contrato (DESIGN §6.9) llama a estos pares `on*`
    (`onAccent`…), pero QML reserva el espacio `on<Mayúscula>` para manejadores
    de señal, así que viajan como `<superficie>Ink` (`accentInk` = `onAccent`).
- **No reconstruir controles.** Un botón ES `CelestinaButton`
  (`primary`/`destructive`), un campo ES `CelestinaTextField`, una superficie
  flotante ES `GlassSurface`/`GlassCard`, un menú ES `GlassContextMenu` +
  `GlassMenuItem`. Si un control existente casi sirve, se extiende el
  compartido, no se clona en la app.
- **Compartir = symlink, nunca copia.** Los `.qml` del estilo se enlazan con
  `ln -s` relativo dentro de `qml/` de la app y se registran en `build.rs`
  (patrón `magnetita/qml/`). Copiar un archivo del estilo crea deriva
  silenciosa: si encuentras una copia, conviértela en symlink.
- Tocar `CelestinaTheme`/componentes = tocar todas las apps: busca consumidores
  (grep) antes de cambiar semántica. El esquema es **datos, no comentarios**: la
  paleta vive en el objeto `ColorScheme schemeDark` (Rosé Pine y el bloque
  comentado se retiraron en S1). Un token de superficie nuevo se añade como
  **rol de `ColorScheme`** con su par de tinta (`<algo>` + `<algo>Ink`); el
  `required` obliga a que todo esquema lo defina. Un esquema claro futuro es una
  instancia `ColorScheme` nueva (un `flip` de `scheme:`), jamás un intercambio de
  comentarios. Los tres niveles: `ref.*` (primitivas, las apps nunca las tocan) →
  esquema (roles) → tokens `sys` planos que consumen las apps.
- `CelestinaTheme.fallbackIcon()` solo funciona si la app registra
  `qml/icons.qrc` en su `build.rs` (Siderita lo hace; Magnetita no — no usarlo
  allí sin registrarlo antes).
- Componente nuevo en `celestina-style`: los ya especificados en DESIGN §6.8
  aterrizan con su **primer consumidor real** (precedente: `CelestinaSwitch` y
  `ListSection` con los ajustes de Magnetita); cualquier otro exige ≥2 apps —
  mientras, vive en la app (precedente: `GlassPill` es local de Siderita).
- Métricas y movimiento también salen de tokens: `space*`, `radius*`,
  `controlHeight*`, `motion*`, `ease*`. Nada de números mágicos de layout.
- Idiomas: la UI visible en **español**; docs, commits e identificadores en
  inglés; los comentarios siguen el idioma del archivo donde están.

## Verificar UI antes de darla por hecha (obligatorio)

- Principio de la suite: estado veraz — un clic es una petición, no una prueba;
  la UI no muestra resultados no verificados. Lo mismo aplica al agente: "el
  código compila" no es "la UI está bien".
- Tras tocar UI, **lanza la app y mira la superficie tocada**. Sin molestar la
  sesión viva: `XDG_CONFIG_HOME=<scratch> QT_QPA_PLATFORM=offscreen
  QT_ASSUME_STDERR_HAS_CONSOLE=1` + un `Timer` temporal en QML que dispare
  `grabToImage(...)` sobre un Item **declarado en QML** (no
  `window.contentItem`) y guarde un PNG. Los colores y el contraste sí se ven
  offscreen; solo el blur del cristal queda en blanco (necesita display real).
- Cada app tiene un único `scripts/run.sh` que compila en release y la
  **instala** en `~/.local` (el shell: compila y **activa** el panel). Para el
  humo, Siderita tiene `scripts/smoke.sh`: chequeo estático del auto-binding
  `x: x` + arranque offscreen de 8 s que **falla ante
  TypeError/ReferenceError**. Ojo con su límite: solo caza errores de
  *arranque* — un binding que se evalúa al hacer clic exige sesión real. Un
  cambio de color/contraste no se declara correcto sin captura o ventana real.

## README y ROADMAP (la deriva doc↔código es el defecto histórico del repo)

- **Regla del mismo cambio**: si añades/quitas un crate, feature, dependencia,
  consumidor o archivo listado en una tabla de layout, actualiza el README y
  ROADMAP afectados en ese mismo cambio — incluidos los del proyecto vecino
  (p. ej. un crate nuevo en `celestina-rs` toca el README de `celestina-rs`).
- Checkboxes con la regla propia del repo: **"source presence is not runtime
  evidence"** — `[x]` solo con evidencia de ejecución, y al marcarlo se dice
  cómo se verificó. Lo incompleto se nombra, no se esconde (patrón "carried
  past 1.0" de Siderita).
- **Sin números que caducan**: no escribas contadores de tests/LOC en docs; si
  el doc ya los tiene y lo tocas, reverifícalos o elimínalos.
- Estructura estándar: README = rol en 2-3 frases, stack, consumes/consumed-by,
  tabla de layout completa, cómo compilar y ejecutar. ROADMAP = checkpoints
  `CP-n` con objetivo y "Done when".

## Flujo de trabajo

- Antes de dar algo por terminado: `cargo fmt --check`, `cargo clippy` sin
  warnings y `cargo test` en `celestina-rs/` y `siderita/`; las apps compilan y
  arrancan (`smoke.sh` en Siderita). La CI (GitHub Actions) cubre el workspace
  Rust y los guards de estilo/QML; todo lo que exige Qt o un display —
  compilar las apps, capturas, sesión real — lo verifica el agente: **para la
  UI, el agente sigue siendo el CI.**
- Commits: asunto en inglés, imperativo, prefijado por proyecto
  (`Siderita: …`, `Magnetita: …`, `celestina-style: …`; `Docs:`/`Refactor:`/
  `Dedup:` para lo transversal). Commit/push solo cuando el autor lo pida.
- Demonio `magnetitad` (servicio systemd de usuario): para reinstalar,
  `systemctl --user stop magnetitad` **antes** de copiar el binario (si no,
  "text file busy" y un restart relanza el viejo), luego `start`. **No
  reiniciarlo en ráfagas** — deja colgado el KDE Connect del teléfono (se cura
  forzando cierre de la app en el móvil). Estado en `~/.config/magnetita/`;
  borrar `trust.json` fuerza re-emparejado.
- Protocolo KDE Connect — invariantes ya aprendidas (no redescubrirlas):
  nunca auto-solicitar emparejamiento (el teléfono conduce, nosotros
  auto-aceptamos); quien marca el TCP hace de **servidor** TLS; los payloads se
  sirven en el rango **1739–1764** o el teléfono no los descarga;
  portapapeles teléfono→PC es manual por límite de Android, no bug nuestro.
