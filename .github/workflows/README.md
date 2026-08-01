# CI de la suite

Un workflow por proyecto, cada uno con su filtro de rutas, para que el
monorepo se comporte como un repositorio por carpeta: tocar Grafita no
reconstruye el shell, y el historial de checks de un proyecto es suyo.

| Workflow | Corre cuando cambia | Qué prueba |
|---|---|---|
| `contracts.yml` | siempre | Guards de arquitectura y estilo, y el test de los propios scanners |
| `celestina-rs.yml` | `celestina-rs/` | `fmt`, `clippy -D warnings` y los tests del workspace de cores |
| `celestina.yml` | `celestina/`, `celestina-rs/` | `fmt`, `clippy -D warnings` y los tests del package Rust del shell |

## Por qué `contracts.yml` no lleva filtro

Es el único que comprueba relaciones *entre* proyectos — symlinks de estilo
compartido, dirección de dependencias, registro QML, techo de líneas. Un cambio
en un proyecto puede romper la invariante de otro, así que filtrarlo por rutas
sería exactamente el fallo que existe para detectar.

## Por qué los filtros de app incluyen `celestina-rs/`

Las apps consumen los cores por ruta relativa, de modo que un cambio en un core
puede romperlas. Cada filtro nombra el árbol `celestina-rs/` entero en vez de
los crates concretos que ese proyecto importa: una lista de crates caduca en
silencio cuando alguien añade una dependencia, y el modo de fallo sería dejar de
probar sin avisar. Correr de más se nota y se corrige; no correr, no.

## Lo que aquí no se prueba

`siderita/`, `magnetita/`, `grafita/` y `fluorita/` no tienen workflow. Son
aplicaciones CXX-Qt: necesitan Qt 6, y sus superficies necesitan además un
compositor Wayland real para blur, portal, accesibilidad y apariencia. Esa
matriz es local y está descrita en `AGENTS.md`; cada proyecto trae sus propios
`scripts/smoke.sh` y, en Siderita, `scripts/qml-tests.sh`. Un verde aquí no dice
nada sobre ellas.

`fluorita-engine` se compila, enlaza y pasa sus 60 pruebas puras en CI
(`celestina-rs.yml` instala `libmpv-dev`), pero las dos que *arrancan* libmpv
—`an_instance_starts_and_answers_properties` y todo `tests/real_media.rs`— se
quedan fuera.

El motivo no es la falta de pantalla ni de salida de audio: el test ya pide
`vo=null` y `ao=null`. Es la versión. El runner de Ubuntu 24.04 trae mpv 0.37 y
el baseline del motor fija opciones que esa versión no conoce (`load-console`,
`load-select`, `load-positioning` y las demás `load-*` de los scripts Lua
internos), de modo que `set_property` devuelve -8 y no llega a existir ninguna
instancia. En la máquina del autor, con mpv 0.41, arrancan sin queja.

Eso significa que el motor tiene un **mínimo de libmpv sin declarar**, que es
justo lo que el contrato de la raíz prohíbe dejar implícito. Está anotado como
decisión abierta en `fluorita/ROADMAP.md`; hasta que se resuelva, esas dos
pruebas son de la matriz local y un verde aquí no dice nada sobre ellas.
