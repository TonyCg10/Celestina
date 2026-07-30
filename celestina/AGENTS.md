# celestina — delta local del shell Niri

El `AGENTS.md` de la raíz sigue siendo obligatorio. Este archivo concreta la
frontera híbrida Rust/C++/QML del shell y no relaja ninguna regla de la suite.

Para trabajo del plan de reemplazo de Noctalia, lee además `ROADMAP.md` (estado
y fases R0–R9) y `NOCTALIA-REPLACEMENT.md` (órdenes de trabajo por fase, hechos
de la sesión, puertas de salida y decisiones abiertas). Las reglas de este
archivo mandan sobre cualquier detalle de aquellos.

## Fronteras del shell

- `src/niri_adapter.rs` es el único lugar que conoce tipos de `niri-ipc`.
  Reduce el stream del compositor a snapshots estrechos y reconecta el socket
  sin filtrar el vocabulario completo de Niri hacia Qt.
- El protocolo helper→host es JSON delimitado por saltos de línea. Conserva
  `kind: "snapshot"` y `kind: "unavailable"`, es compatible hacia atrás y se
  valida antes de publicar estado. El host limita cada mensaje a 1 MiB y vacía
  estado previo ante caída, salida inválida o indisponibilidad.
- C++ manual queda restringido al ciclo de vida de `QProcess`, marshaling Qt,
  LayerShellQt y KWindowEffects. Cada ampliación nombra la limitación concreta
  de CXX-Qt que la exige; dominio, reducción de eventos y política testeable no
  se desplazan a C++.
- `NiriProtocolDecoder` posee únicamente el framing acotado del pipe y su
  recuperación tras una línea hostil; `NiriClient` valida JSON y publica el
  snapshot. `PanelBlurController` posee únicamente capacidad, reintentos,
  geometría y fallback del efecto KWindowEffects. No devuelvas esas
  responsabilidades a `main.cpp` ni mezcles ambos ciclos de vida.
- El helper reconecta el IPC de Niri dentro del proceso. Si el proceso muere, el
  host puede relanzarlo sólo con espera acotada y backoff; nunca en un bucle
  inmediato sobre el hilo gráfico.
- `qml/` presenta estado ya adaptado. No abre sockets, no lanza procesos y no
  decide protocolo o recuperación. El panel no alcanza ids internos de otros
  componentes: inyecta propiedades estrechas y recibe señales.
- Cada raíz o componente QML nuevo se añade a `QML_FILES` en
  `CMakeLists.txt`; el guard raíz exige paridad exacta con `qml/`.
- Panel y chooser importan el árbol canónico `../celestina-style` directamente:
  CMake crea el alias URI para `qmllint` y el host crea el alias equivalente en
  runtime. No vuelvas a una paleta inline, a una copia local ni afirmes que el
  módulo instalado es el contrato actual.

## Superficies y efectos

- Hay una superficie layer-shell por `QScreen`: borde superior, zona exclusiva
  coherente con su altura y `KeyboardInteractivityNone`. Hotplug crea o retira
  únicamente la superficie del monitor afectado.
- El blur del compositor es best-effort. La región enviada a KWindowEffects se
  mantiene finita y se rearma tras cambios de geometría; QML consume un estado
  explícito y usa una superficie de fallback legible cuando no está disponible.
- `scripts/run.sh` construye y activa el panel real. No lo ejecutes, no ocultes
  otra barra y no cambies la sesión Niri salvo petición explícita del autor.
- `--pick-output` es un contrato con `xdg-desktop-portal-wlr`: al aceptar
  imprime exactamente `Monitor: <output>` en stdout. Logs y diagnósticos van a
  stderr; cancelar no inventa una selección.

## Build reproducible y evidencia

- Cargo se invoca con `--locked`; `Cargo.lock` forma parte de las dependencias
  del target CMake. No actualices el lockfile como efecto lateral de un build.
- Desde la raíz, la matriz mínima para cambios del shell es:

```sh
bash scripts/check-architecture-contract.sh
cargo fmt --manifest-path celestina/Cargo.toml --all --check
cargo clippy --manifest-path celestina/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path celestina/Cargo.toml --all-targets --locked
cmake -S celestina -B celestina/build -DBUILD_TESTING=ON
cmake --build celestina/build
cmake --build celestina/build --target all_qmllint
ctest --test-dir celestina/build --output-on-failure
```

- Tests Rust prueban reducción y serialización; el test C++ de
  `NiriProtocolDecoder` prueba fragmentación, varios frames y descarte/recuperación
  tras exceder 1 MiB. Amplía CTest cuando cambie otra política C++ aislable.
  Build, CTest y `qmllint` no prueban layer-shell, hotplug, blur, foco ni el
  formato real del portal.
- Blur, geometría, foco y selección de monitor sólo se declaran validados tras
  una comprobación en una sesión Wayland/Niri real. Distingue esa evidencia de
  un arranque offscreen o de una compilación correcta.
