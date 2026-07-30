# AGENTS.md — contrato de trabajo para agentes en Celestina

Este archivo es la fuente canónica de instrucciones del monorepo. Sus reglas
son obligatorias para cualquier agente que analice, modifique o revise el
checkout. Codex da precedencia técnica al `AGENTS.md` más cercano al directorio
de trabajo; como política del proyecto, los archivos anidados sólo añaden
requisitos locales y nunca relajan las invariantes de esta raíz.

## Antes de actuar

1. Lee este archivo completo y el `AGENTS.md`, `README.md`, `ROADMAP.md` o
   `DESIGN.md` más cercano al código que vayas a tocar. Si la sesión comenzó en
   la raíz, abre manualmente el `AGENTS.md` del subproyecto afectado: Codex sólo
   autodetecta instrucciones entre la raíz Git y el directorio de inicio.
2. Inspecciona el checkout real (`git status`, archivos, consumidores y guards)
   antes de proponer o implementar. Un documento puede estar atrasado.
3. Busca antes de crear (`rg` / `rg --files`): reutiliza contratos existentes y
   no introduzcas una segunda forma de resolver el mismo problema.
4. Conserva cambios ajenos del worktree. No hagas commit, push, instalación,
   activación de servicios ni cambios fuera del repositorio sin petición del
   autor.
5. Mantén el alcance: una auditoría no autoriza una corrección; una corrección
   concreta no autoriza una reescritura vecina.

## Límites de autorización de la suite

- El trabajo de implementación ordinario está autorizado en `siderita/` y
  `magnetita/` dentro del alcance pedido.
- No ampliar `celestina/` salvo petición explícita del autor.
- No añadir código a `fluorita/` ni `grafita/` salvo petición explícita; sus
  contratos escritos no autorizan comenzar esos proyectos.
- El estado y los hitos viven en los ROADMAP, no en este archivo. No copies aquí
  versiones, contadores de tests o afirmaciones temporales.

## Mapa arquitectónico y dirección de dependencias

| Responsabilidad | Destino | No debe contener |
|---|---|---|
| Dominio puro, operaciones, protocolo, IO testeable | `celestina-rs/` | Qt, QML o composición visual |
| Estado de UI, adaptación y marshaling Qt/D-Bus/XDG | `src/` de la app | Reglas de dominio que puedan probarse sin UI |
| Presentación y composición de una pantalla | `qml/` de la app | IO, protocolo o decisiones de dominio |
| Tokens y controles visuales reutilizables | `celestina-style/` | Estado de una app, D-Bus, Niri o workflows |
| Hueco que CXX-Qt no cubre | `cpp/` de la app | Lógica que Rust/CXX-Qt sí pueda expresar |
| Contrato entre procesos | D-Bus compatible hacia atrás | Acoplamiento a ids o tipos internos de QML |

Reglas de dirección:

- Los crates puros pueden ser usados por adaptadores de UI; nunca dependen de
  estos.
- `magnetita/` es un cliente QML fino. Protocolo, red y daemon de Magnetita
  pertenecen a los crates correspondientes de `celestina-rs/`.
- Siderita puede tener dominio propio en `celestina-rs` aunque aún tenga un solo
  consumidor, si es puro y testeable.
- `celestina-style` puede depender de Qt Quick, pero nunca de módulos de una
  aplicación.
- C++ manual requiere un comentario que nombre la limitación concreta de
  CXX-Qt que lo hace necesario.

Si una pieza no encaja claramente en una fila, detente y documenta la decisión
antes de implementarla. No la coloques por conveniencia en el archivo que ya
está abierto.

## Reutilización sin duplicación

Antes de crear una función, control, token o componente, busca nombres y
recetas equivalentes en todo el monorepo.

- Comportamiento puro y testeable: módulo/crate de `celestina-rs`, incluso con
  un solo consumidor inicial.
- Presentación repetida dentro de una app: componente/helper local con API
  estrecha.
- Componente ya especificado en `celestina-style/DESIGN.md`: puede entrar en el
  estilo con su primer consumidor real.
- Componente visual no especificado: sólo entra en `celestina-style` cuando al
  menos dos aplicaciones demuestran la misma semántica. Hasta entonces es
  local.
- Compartir QML de estilo significa symlink relativo y registro explícito;
  nunca una copia.
- Una abstracción compartida conserva sólo la intersección real. No recibe
  flags de aplicación para fingir reutilización.

Cuando una receta aparece por segunda vez, el agente debe comparar ambas y
decidir explícitamente entre extraer o documentar por qué sus semánticas son
distintas.

## Modularidad: coordinadores, no monolitos

El techo general para archivos fuente nuevos Rust/QML/C++ es 800 líneas. Los
archivos heredados por encima del techo están congelados por
`scripts/architecture-baseline.tsv`: pueden reducirse, nunca crecer. Elevar un
límite o añadir una excepción requiere aprobación explícita del autor y una
justificación arquitectónica; no se actualiza el baseline para silenciar CI.

El número de líneas es una alarma, no la definición completa. Extrae antes si
aparece cualquiera de estas señales:

- el archivo mezcla coordinación, presentación detallada y dominio;
- una región posee estado, acciones y ciclo de vida propios;
- la API necesita pasar objetos genéricos para alcanzar ids externos;
- una segunda feature añade otra razón independiente para cambiar el archivo;
- tests o verificación sólo pueden ejecutarse cargando una superficie enorme.

Reglas QML:

- Un host coordina; cada región coherente vive en un componente.
- Componentes se comunican mediante propiedades tipadas, `required property`,
  señales y funciones pequeñas. Evita `property var` cuando existe un tipo o
  un contrato más estrecho; si es inevitable, comenta qué interfaz espera.
- Un componente no alcanza ids del archivo padre. El padre inyecta datos y
  recibe señales.
- Delegates no contienen IO ni decisiones de dominio.
- Todo QML nuevo se registra en `build.rs`, CMake o `qmldir`, según el proyecto.
- No uses `x: x` al inyectar propiedades: renombra la propiedad o expón un
  alias inequívoco.

Reglas Rust/CXX-Qt:

- Un módulo tiene una responsabilidad nombrable y testeable.
- Un QObject no acumula dominios independientes; crea otro objeto cuando el
  estado y ciclo de vida no sean compartidos.
- El bloque `#[cxx_qt::bridge]` puede concentrar el contrato exigido por la
  herramienta, pero sus invocables delegan; la lógica no vive en el bridge.
- Preferir composición y funciones libres pequeñas a controladores universales.

## Estilo QML y componentes

- No escribir colores QML fuera de `CelestinaTheme`: ni hex, nombres, `Qt.rgba`,
  `Qt.darker`, `Qt.lighter` ni mezclas locales. La regla es obligatoria para
  todo QML nuevo o modificado; el guard visual cubre Siderita, Magnetita, el
  shell y el propio módulo compartido.
- No hardcodear anatomía compartida: tipografía, radios, bordes, paddings de
  control, opacidades de estado, duraciones y easing salen de tokens semánticos.
- Coordenadas, anchos responsivos y geometría propia de una pantalla permanecen
  locales; no crear tokens sin semántica reutilizable.
- Los colores se consumen en pares superficie/tinta. No declares un par válido
  por nombre: verifica contraste en los estados reales donde se pinta.
- Usa el control compartido existente antes de reconstruir uno Qt. Las
  excepciones locales existentes son un baseline descendente, no precedentes
  para crear más.
- Antes de modificar tema o componente compartido, busca todos sus consumidores
  y valida el conjunto, no sólo la app que motivó el cambio.
- El mínimo de Qt declarado debe cubrir la API más nueva usada. Introducir una
  API posterior obliga a actualizar el contrato de toolchain o aportar fallback.

### Accesibilidad forma parte del componente

Un control no está terminado sólo porque responda al ratón.

- Toda acción debe ser operable por teclado y tecnología asistiva.
- Usa controles Qt cuando aporten semántica; si construyes sobre `Item` o
  `MouseArea`, declara rol, nombre, estado y acción `Accessible` equivalentes.
- El foco visible de controles nuevos o modificados se basa en `visualFocus`;
  no muestres el anillo por clic salvo que el patrón lo requiera expresamente.
- Diálogos contienen el foco, desactivan las acciones de la superficie inferior
  y restauran el foco al cerrar.
- Listas, pestañas, selección, progreso, errores y toggles exponen estado
  accesible, no sólo apariencia.
- `CelestinaTheme.reducedMotion` es la entrada compartida y cada host la inyecta
  desde `CELESTINA_REDUCED_MOTION`. Todo movimiento nuevo o modificado la honra:
  las transformaciones espaciales/escala quedan instantáneas o desactivadas y
  ningún `Behavior` o `Transition` nuevo queda sin ruta reducida. Su existencia
  no demuestra que toda animación heredada haya sido auditada ni validada en una
  sesión real.
- Texto normal cumple al menos 4.5:1 y texto grande 3:1 en cada estado y esquema.

## Rust, IO y contratos externos

- `unsafe` está prohibido salvo una excepción previamente aprobada y aislada.
- No añadir `#[allow]` para ocultar deuda. No introducir TODO/FIXME/HACK como
  sustituto de terminar o registrar trabajo.
- Cero `unwrap`, `expect` o `panic!` en rutas de producción, excepto el patrón
  de mutex ya documentado o una invariancia demostrada junto al `expect`.
- Errores tipados con contexto y fuente. D-Bus best-effort: su caída degrada el
  servicio, no tumba ni bloquea la app.
- Una API D-Bus/IO bloqueante nunca corre en el hilo Qt. Ejecuta el trabajo fuera
  del GUI thread, limita/coalesce ráfagas, aplica sólo snapshots confirmados al
  volver y conserva un ciclo de vida que pueda cerrarse de forma determinista.
- Escrituras con pérdida cero: no borrar origen antes de verificar destino;
  limpiar destinos parciales tras fallo o cancelación.
- Entrada de red es hostil: límites de bytes, timeouts y saneado de nombres/rutas.
- Métodos D-Bus publicados se conservan. Extiende `a{sv}` con claves nuevas en
  vez de romper consumidores existentes.
- Toda feature de dominio aterriza con tests en el mismo cambio.
- Cada dependencia nueva lleva justificación en `Cargo.toml`; no añadir runtimes
  o frameworks pesados sin necesidad medida y aprobación del autor.

## Evidencia y definición de terminado

Ejecuta primero el guard común:

```sh
bash scripts/check-architecture-contract.sh
```

Después aplica la matriz mínima según el área tocada:

| Área | Evidencia mínima |
|---|---|
| `celestina-rs` | `cargo fmt --all --check`, clippy con `-D warnings`, tests del workspace |
| Rust de una app | fmt, clippy y tests de su package |
| QML de Siderita | registro, `qmllint`, build y `siderita/scripts/smoke.sh` |
| QML de Magnetita | registro, build y arranque offscreen de la superficie afectada |
| `celestina-style` | guard, `all_qmllint`, galería y consumidores afectados |
| Guards/CI de arquitectura | `bash scripts/test-architecture-scanners.sh`, guard normal y fixture negativa relevante |
| D-Bus/protocolo | tests del productor y compatibilidad de consumidores |
| UI visual | captura/inspección de la superficie; Wayland real para blur/compositor |
| Accesibilidad | teclado/foco automatizado cuando sea posible y AT-SPI real antes de declararla validada |

Un build prueba que compila. Un smoke prueba que arranca. Ninguno prueba por sí
solo interacción, apariencia, portal, hardware, compositor o accesibilidad.
Describe exactamente qué evidencia obtuviste y qué quedó sin prueba real.

## README, ROADMAP y DESIGN

- README describe rol, stack, estructura y uso actuales.
- ROADMAP registra estado, decisiones abiertas y evidencia de checkpoints.
- `celestina-style/DESIGN.md` es el contrato visual; no se reinterpreta desde
  comentarios locales.
- Añadir/quitar crate, feature, dependencia, consumidor o archivo inventariado
  actualiza los documentos afectados en el mismo cambio.
- `[x]` requiere evidencia de ejecución: presencia de código no es prueba de
  comportamiento.
- Evita contadores y snapshots que caducan. Si tocas uno, reverifícalo o bórralo.
- UI visible en español; identificadores, documentación de producto y commits
  en inglés. Estas instrucciones operativas y los comentarios siguen el idioma
  del archivo existente.

## Flujo del agente y cambios delicados

- Haz cambios pequeños y revisables. No combines refactor mecánico con cambio de
  comportamiento si pueden verificarse por separado.
- No borres ni reviertas trabajo ajeno. Si una regla nueva descubre deuda
  previa, congélala con un baseline explícito y propón su reducción.
- No eleves un baseline, amplíes una allowlist ni desactives un guard para hacer
  pasar una entrega.
- No marques validación real si sólo ejecutaste una simulación/offscreen.
- Commits en inglés, imperativos y prefijados por proyecto. Commit/push sólo a
  petición del autor.
- Para reinstalar `magnetitad`, detener primero el servicio de usuario y
  arrancarlo después de copiar; no reiniciarlo repetidamente.
- Invariantes KDE Connect: el teléfono conduce el pairing; quien inicia TCP es
  servidor TLS; payloads usan 1739–1764; clipboard teléfono→PC es manual por
  Android. No “arreglar” estos límites sin nueva evidencia de protocolo.
