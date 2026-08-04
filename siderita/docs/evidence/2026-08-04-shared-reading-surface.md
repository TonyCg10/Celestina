# Evidence — SID-G7 shared reading surface

- **Date:** 2026-08-04
- **Scope:** Siderita `SID-G7-A`; plan
  [shared-reading-surface](../plans/active/2026-08-04-shared-reading-surface.md)
- **Environment:** Linux 7.1.5-1-cachyos, Qt 6, CXX-Qt, offscreen Qt platform
  for every automated surface check; the two units this one depends on,
  CelestinaStyle `STYLE-G7-A` and Grafita `G7-A`, are the two commits that
  precede it and were present in the checkout throughout
- **Artifact:** `siderita/target/release/siderita`, sealed in
  `siderita/target/production-artifact.toml` and deployed to
  `/home/toni/.local/bin/siderita`
- **Related author validation:** `VAL-SID-G7` (pending, not claimed here)

## Procedure

Suite guards over the whole checkout:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
```

Version transition before the canonical build:

```sh
python3 scripts/version_tool.py bump siderita milestone \
  --unit SID-G7-A \
  --summary "Adopt the shared reading controls in both text surfaces"
```

Registered completion, which builds once, verifies those exact bytes and
deploys them without recompiling:

```sh
bash siderita/scripts/complete-production.sh
```

## Result

- **Exit:** 0 for every command above.
- **Observed:**

```text
Contrast contract: OK
QML visual contract: OK
Architecture contract: OK
version-contract: OK (6 owners)
siderita: 1.0.1 -> 1.1.0 (milestone)
manifest: siderita/target/production-artifact.toml (verified)
artifact: siderita current and verified
installed: OK /home/toni/.local/bin/siderita
```

Inside the completion entry: the architecture guard, `cargo fmt --all --check`,
`cargo clippy --all-targets --locked -- -D warnings`, and
`cargo test --all-targets --locked` covering `siderita` itself (69) together
with every crate it consumes — `celestina-core` (26), `siderita-core` (37 + 1),
`siderita-ops` (29 + 22), `siderita-qt` (3), `grafita-core` (80 + 23 + 18) and
the Fluorita crates (73, 75 + 15, 3), all passing; the QML test runner
(47 passed, 0 failed), which constructs both changed surfaces offscreen; and the
smoke (binary live 8 s, no QML error, no auto-binding).

The QML test run is what proves the two shared files are registered through
`build.rs`: an unregistered type fails construction rather than degrading, which
is the failure mode this project's offscreen gate exists to catch.

### Retired debt

- **Resolved architecture debt:** `siderita/qml/dialogs/QuickLookView.qml`

The quick look's text pane no longer instantiates `QtQuick.Controls`
`ScrollView` or `TextArea`; it composes the suite's `CelestinaScrollBar` and
`CelestinaLineGutter` over a plain read-only text item. Both
`scripts/architecture-baseline.tsv` rows are therefore deleted in this same
commit rather than left as ratchets for controls the file no longer contains.

- **Resolved language debt:** `siderita/qml/dialogs/GrafitaEditorDialog.qml`

Dropping the encoding label removed the file's last non-English string, so its
`scripts/language-baseline.tsv` row is deleted here too.

### Boundary record for the preferences adapter

- **Canonical owner:** `grafita_core::preferences` owns the bounds, the clamping
  and the file format. Siderita adds no rule.
- **Equivalent recipes searched:** Grafita's own adapter, `grafita/src/preferences.rs`,
  was read before this one was written. It is not imported, because Qt
  marshalling belongs to each application's `src/` — the same shape by which
  each host already adapts `DocumentSession` — and importing another
  application's `src/` is what the architecture direction forbids.
- **Dependency direction:** Siderita → `grafita-core`. Nothing points back, and
  no Grafita application type is referenced.
- **Deliberate difference:** this adapter re-reads on demand and applies a nudge
  to what is stored at that moment, because Grafita may be running beside it and
  each folder view holds one object. Reloading never writes.

## Limits

- A build, the QML tests and an offscreen smoke prove compilation, registration,
  construction and startup. They do not prove the author's compositor, display
  scale, physical keyboard layout, pointer input or AT-SPI.
- Not covered automatically and left to `VAL-SID-G7`: scroll-bar dragging and
  the `Ctrl` wheel, which the synthetic-input tool cannot produce, and whether
  the size shortcuts reach a modal surface from the author's physical layout.
- Cross-process behaviour — a size changed in Grafita appearing in the next
  Siderita surface — is exercised only through the stored file in unit tests,
  not with both applications running.

## Follow-up

`VAL-SID-G7`, and the sibling units
[`STYLE-G7-A`](../../../celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md)
and
[`G7-A`](../../../grafita/docs/plans/active/2026-08-04-g7-reading-comfort.md).
