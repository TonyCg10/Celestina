# Every resource on the Performance page, with per-row availability — H2-B

- **Date:** 2026-09-22
- **Scope:** `H2-B` of
  [`../plans/archive/2026-09-22-h2-resources.md`](../plans/archive/2026-09-22-h2-resources.md):
  the sampler's sections and topology, `hematita/src/publish.rs`,
  `HematitaResources` as index-aligned lists with a `revision` ticket, the
  Performance page weaving rows, kind-driven detail, the per-core grid, the
  per-row failure state, and the H1 follow-ups
- **Environment:** Rust 1.97.1 (pinned), Qt 6 with CXX-Qt 0.9.1, the author's
  checkout
- **Artifact:** `hematita/target/release/hematita`, built by
  `hematita/scripts/build-production.sh` and sealed by
  `hematita/scripts/verify-production.sh`

## Procedure

One build cycle, after every file of the unit was written:

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
    && cargo clippy --release --all-targets --locked -- -D warnings \
    && cargo fmt --all --check)
hematita/scripts/verify-production.sh
QT_QPA_PLATFORM=offscreen timeout 6 hematita/target/release/hematita 2>&1 \
    | grep -E 'Error|Warning|unavailable|Binding loop|Cannot anchor'
```

The first `build-production.sh` failed to compile with one borrow error
(`E0596`: `let state = self.as_mut().rust_mut()` needs `let mut state`); the
line was fixed and the cycle repeated. No other rebuild was spent.

## Result

- **Build:** `>> Hematita release build steps completed (not installed)`;
  `manifest: hematita/target/production-artifact.toml (pending verification)`.
- **Tests:** `hematita` — `test result: ok. 4 passed; 0 failed` (the four
  `publish::tests`: `load_names_the_state_the_page_paints`,
  `only_a_newer_generation_is_applied`, `fractions_and_tickets_stay_in_range`,
  `each_kind_publishes_its_numbers_in_contract_order`). `hematita-core`, run
  again by the verification: `37 passed; 0 failed` plus `5 passed; 0 failed`
  for `tests/captures.rs`.
- **Clippy:** `cargo clippy --release --all-targets --locked -- -D warnings`
  clean for `hematita` and for `hematita-core`.
- **Format:** `cargo fmt --all --check` clean in both workspaces.
- **qmllint:** `qmllint-production: OK — org.celestina.hematita (0 non-fatal
  baseline warning(s))` — the row stayed at `0`, and no suppression comment
  exists anywhere in the QML.
- **Smoke:** `smoke: OK — binary alive for 8 s, no QML errors, no
  auto-bindings`.
- **Verification exit:** `0`;
  `manifest: hematita/target/production-artifact.toml (verified)`.
- **Offscreen run:** the grep printed nothing and `rc=124` — six seconds of
  sampling with no QML error, no warning, no binding loop and no anchor
  complaint from the `ListView` inside the panel's `contentItem`.

## Kind contract

The page reads a row's `numbers` by index, so the order is part of the
interface. `publish.rs` is the only place that writes it and its test locks
every layout:

| Kind | `numbers` |
|---|---|
| `cpu` | `[aggregate percent, frequency MHz (0 = unknown), core count]` |
| `memory` | `[used KiB, total KiB, swap used KiB, swap total KiB]` |
| `gpu` | `[busy percent, memory busy percent, VRAM used B, VRAM total B, GTT used B, GTT total B, core MHz (0 = unknown), memory MHz (0 = unknown)]` |
| `disk` | `[read B/s, write B/s, size B, rotational (1 = mechanical)]` |
| `network` | `[rx B/s, tx B/s, speed Mbit/s (0 = unknown), wireless (1 = yes), up (1 = yes)]` |

Row keys are `cpu`, `memory`, `gpu`, `disk:<name>`, `net:<name>`; states are
`waiting`, `ready`, `unavailable`; reason kinds are `unreadable`, `malformed`
and `no-rate`. Every one of these is a token: the Spanish sentence a person
reads is composed in `PerformancePage.qml` through `qsTr()`, and no Rust file
in the project carries product copy any more (the
`language-contract: product-copy` marker is gone from `resources.rs`).

## The `net:` convention

`/proc/net/dev` is one file for every interface, so when it cannot be read
there is no interface to blame. The sampler then publishes exactly one
`InterfaceSection` whose `info.name` is empty; the adapter keys it `net:` and
the page shows a single unavailable network row named `Red` carrying the
typed reason. A machine whose `/proc/net/dev` reads normally never produces
that row.

## Limits

- No visual check: nothing was opened on the live or nested session.
  Appearance, keyboard, focus, the per-core grid's legibility and the icon
  toggle belong to `VAL-H2`.
- The GPU path is AMD `amdgpu` only, and a machine without such a card
  publishes no GPU row at all — absence, not failure. Sensors (temperature,
  power) stay excluded until H4.
- The unavailable paths (an unreadable `/proc/stat`, a malformed
  `/proc/diskstats`, a missing `/proc/net/dev`) were not provoked on this
  machine: they are covered by the typed construction and by `publish.rs`'s
  tests, not by a live failure.
- Disk and interface rows scale their history by their own peak
  (`Ring::fractions`), so a throughput graph is relative to the busiest
  second in its own minute and two such graphs are not comparable to each
  other.
