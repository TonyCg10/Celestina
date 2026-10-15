# Evidence: 2026-08-08 workspace groups survive their monitor

- **Date:** 2026-08-08
- **Scope:** Celestina `WSG-1`, units `WSG-1-A` through `WSG-1-D`
- **Artifact:** celestina 0.8.0, canonical production bundle
- **Environment:** Linux 7.1.6 (CachyOS), Rust 2021 workspace, Qt 6.9, CMake
  Release, offscreen Qt platform. No live Niri session was used and Noctalia
  continued to own the session throughout.

## What was implemented

A workspace's **home** — the monitor it belongs to rather than the one it is
currently on — is remembered from a frame that could see it, or declared by the
author, and the panel strip folds a displaced session into one open group and
one capsule per absent monitor.

| Unit | Result |
|---|---|
| `WSG-1-A` | Pure grouping policy, the learned/declared memory and the refusal rules in `celestina-shell-core::workspace_groups` |
| `WSG-1-B` | The home published additively on the Niri snapshot, persisted durably, seeded from the author's declarations, and the group decision published with it |
| `WSG-1-C` | The collapsible strip, its extracted pill and capsule components, and their assistive routes |
| `WSG-1-D` | Version, history, guards and the canonical production exit |

## Procedure

All commands were run from the repository root unless stated otherwise.

```sh
bash scripts/check-architecture-contract.sh
```

Exit 0 — `Sealed colour contract: OK (4 colour(s))`, `Contrast contract: OK`,
`QML visual contract: OK`, `Architecture contract: OK`.

```sh
python3 scripts/check-language-contract.py
```

Exit 0 — `Language contract: OK (157 legacy file(s) ratcheted)`. The baseline
did not move.

```sh
bash scripts/check-documentation-contract.sh
```

Exit 0 after the ledger statuses were closed. The pre-existing `SID-G7-D`
errata rows are unchanged and unrelated to this unit.

```sh
cd celestina-rs && cargo fmt --all --check && \
  cargo clippy --locked -p celestina-shell-core -p celestina-core -p magnetita-core --all-targets -- -D warnings && \
  cargo test --locked -p celestina-shell-core -p celestina-core -p magnetita-core
```

Exit 0 — 273 `celestina-shell-core` tests pass, including the eight new
persistence, schema, bounding and declaration cases.

```sh
cd celestina && cargo fmt --all --check && \
  cargo clippy --all-targets --locked -- -D warnings && \
  cargo test --all-targets --locked
```

Exit 0 — 20 `celestina-niri-adapter` unit tests pass, including the four new
grouping cases, plus the existing helper and integration binaries.

```sh
cd celestina && ./scripts/qmllint-production.sh
ctest --test-dir celestina/build --output-on-failure
```

Exit 0 — qmllint reused the generated module without invoking CMake or Cargo.
CTest 17/17, including the five new `WorkspaceStrip` grouping cases.

```sh
python3 scripts/version_tool.py check
celestina/scripts/complete-production.sh
celestina/scripts/status-production.sh
```

Recorded at the end of this file.

## What the clippy refusal changed in the contract

`clippy::struct_excessive_bools` refused the first shape of the wire row, which
carried `group_expanded` and `group_focus` as two booleans. That was the right
refusal for a reason beyond counting: the two facts are not independent — being
the workspace a capsule asks for only means anything while that capsule exists.
The published field is now one `group` value of `expanded`, `collapsed` or
`collapsed-target`, and the host splits it into the two flags a surface
consumes. No `#[allow]` was added.

## Deliberate boundaries

- **Correlation is not causality.** Nothing here investigates or bears on the
  GPU device-loss question. This unit touched no DDC, no brightness path and no
  process lifecycle.
- **The compositor is not asked to move anything.** Niri's own `open-on-output`
  is not available over IPC and this shell does not read another program's
  configuration files. A home is remembered or declared, never read.
- **A single-output frame teaches nothing.** It cannot distinguish a workspace
  that belongs to the survivor from one displaced onto it, which is the whole
  distinction the feature rests on.
- **An observation never overwrites a known home.** The frame that would rewrite
  it is precisely the frame that is wrong about it. Only a declaration changes a
  known home.
- **The declaration takes effect at the next helper start.** It is read from the
  shell's settings file at helper start and on each reconnection, not watched.
  The settings file is written by the aggregate provider helper alone; the Niri
  adapter only reads the single field it can act on, through the same
  `celestina_shell_core::settings` schema, so there is no second idea of what
  that file contains.
- **No motion was added.** The strip changes shape when the focus moves between
  monitors, instantly, like every other panel state. There is no animation for
  `CelestinaTheme.reducedMotion` to undo.

## Limits

- **Offscreen and unit evidence only.** No panel was mapped on a real
  compositor. Geometry, hotplug, the appearance of a capsule at either output
  scale, focus behaviour and AT-SPI remain `VAL-WSG-1` and are the author's to
  run.
- **The host's default for an absent `group` field is not directly unit-tested.**
  `NiriClient::applySnapshot` is private and the host has no snapshot test target
  today; adding one was out of this unit's boundary. The equivalent default is
  covered at the surface, where the QML fixture omits `home`, `groupExpanded` and
  `groupFocus` and the strip still renders one flat group. The host's own two
  lines are covered only by review and by the offscreen smoke loading the panel.
- **The learned memory has never been taught on real hardware.** Every test
  teaches it from a synthetic multi-output frame. Whether the author's session
  produces such a frame before two monitors go off is exactly what `VAL-WSG-1`
  exists to find out.
- **`--pick-output` is what the smoke exercises**, so the smoke proves the host
  and the compiled style module load without QML errors. It starts no helper,
  runs no `ddcutil`, opens no real Wayland connection and does not map a panel.

## Result

### Canonical production exit

`python3 scripts/version_tool.py bump celestina milestone` moved the registered
declaration `0.7.0 -> 0.8.0` and appended the matching `docs/version-history.tsv`
row. `python3 scripts/version_tool.py check` then reported
`version-contract: OK (6 owners)`.

`celestina/scripts/complete-production.sh` built the 0.8.0 bundle once, verified
those exact bytes and deployed them. Exit 0. Observed:

```text
100% tests passed out of 17
smoke-production: OK — host release + CelestinaStyle compilado vivos 8 s
manifest: celestina/build/production-artifact.toml (verified)
>> Celestina bundle deployed to /home/toni/.local; the session was not activated
artifact: celestina current and verified
installed: OK /home/toni/.local/libexec/celestina/celestina
installed: OK /home/toni/.local/libexec/celestina/celestina-niri-adapter
installed: OK /home/toni/.local/libexec/celestina/celestina-provider-adapter
installed: OK /home/toni/.local/libexec/celestina/libcelestina-style.so
installed: OK /home/toni/.local/libexec/celestina/CelestinaStyle
installed: OK /home/toni/.local/bin/celestina
installed: OK /home/toni/.local/share/applications/celestina.desktop
```

`activate-production.sh` was not called and no live surface was replaced.
Noctalia still owns the session. The author's on-disk test bundle under
`~/.local` now carries 0.8.0; a running shell would have to be restarted to load
it, and restarting it is not part of this unit.
