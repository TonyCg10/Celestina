# Evidence — F5 source-first library and direct activation

- **Date:** 2026-08-04
- **Scope:** Fluorita F5; plan
  [source-first-library](../plans/active/2026-08-04-source-first-library.md)
- **Environment:** Linux 7.1.5-1-cachyos, Qt 6, libmpv present, offscreen Qt
  platform for every automated surface check; the author's checkout carried
  unrelated uncommitted Celestina (R3) and Grafita (G7) work throughout and was
  being edited in parallel during this session
- **Artifact:** `fluorita/target/release/fluorita` and
  `siderita/target/release/siderita`, built and sealed in the detached worktree
  described below, deployed to `/home/toni/.local/bin/`
- **Related author validation:** `VAL-FLU-SOURCES` (pending, not claimed here)

## Where the verified bytes were built

`scripts/check-architecture-contract.sh` runs over the whole checkout and is
chained by both completion commands. It failed on the author's uncommitted
Grafita work — `grafita/qml/components/EditorScrollBar.qml` rebuilds a Qt
`ScrollBar` outside the baseline — which is unrelated to this unit and must not
be modified.

The author asked for the deployment regardless. Rather than mutate their
in-flight work, both products were built, verified and deployed from a detached
`git worktree` at `HEAD` carrying **only** this unit's changes. Two files needed
care because the author was editing them concurrently: `docs/version-history.tsv`
and `scripts/language-baseline.tsv` were rebuilt from `HEAD` plus this unit's
lines alone, so no Celestina or Grafita row was carried into the built bytes.

The author then resolved that Grafita violation during the same session, so both
completion commands were run again from the main checkout and passed there. The
manifests, the seals and the deployed binaries all correspond to the author's
own checkout; the temporary worktree was removed and nothing depends on it. Both
runs produced the same result, which is recorded below.

## Procedure

Guards, in the worktree carrying only this unit:

```sh
bash scripts/check-architecture-contract.sh   # Architecture contract: OK
python3 scripts/check-language-contract.py    # OK (172 legacy files ratcheted)
bash scripts/check-documentation-contract.sh  # Documentation contract: OK
python3 scripts/version_tool.py check         # OK (6 owners)
```

Completion, exit 0 for both:

```sh
bash fluorita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

Inside the Fluorita entry: `test-production-artifacts.sh` (27 tests), the
architecture guard, `cargo fmt --all --check`,
`cargo clippy --all-targets --locked -- -D warnings` and
`cargo test --all-targets --locked` for `fluorita` (25 tests) and for
`celestina-core`, `fluorita-core` (73), `fluorita-engine` (75) and
`fluorita-qt`; `qmllint-cxxqt.sh` (OK, 31 non-fatal baseline warnings); and
`fluorita/scripts/smoke.sh`. Inside the Siderita entry: its own guard, lint and
test chain plus 47 passing QML tests and its offscreen smoke.

## Result

- **Exit:** 0 for every command above, including both completion commands.
- **Observed:** manifest and installed state afterwards —

```text
artifact: fluorita current and verified
installed: OK /home/toni/.local/bin/fluorita
artifact: siderita current and verified
installed: OK /home/toni/.local/bin/siderita
```

### New automated coverage

- `fluorita-core`: source scoping of both projections, root removal without
  handle reuse, restore of stored identities, validation of a stored
  configuration as hostile input, and `Catalogue::retain_configured`.
- `fluorita-engine`: the source store's round trip with identities intact, a
  non-UTF-8 root surviving a write and a read, a removed root staying removed
  across a restart, an unusable or oversized file seeding instead of emptying
  the library, and a partly damaged file keeping what it can while counting the
  rest.
- `fluorita`: the portal answer's URI decoding, including a non-UTF-8 name and
  the refusal of non-local URIs.
- `fluorita/scripts/smoke.sh`: a new gate asserting that browsing the library
  writes a recognised folder configuration listing at least one root. Without
  it, a run that resolved no root or kept the set in memory would look
  identical to a healthy one.

### Observed handler state

Fluorita's deploy registers its desktop entry, and the README requires reporting
any effective handler change. After deployment:

```text
video/mp4  -> org.celestina.Fluorita.desktop
audio/flac -> org.celestina.Fluorita.desktop
image/png  -> gmic_qt.desktop
```

Fluorita is the effective handler for its advertised video and audio types on
this unpinned desktop; `image/png` remains with an existing pinned choice. This
is the same stateful behaviour recorded for earlier Fluorita deployments and was
not changed by this unit.

### Pointer defects found by looking at the running application

Three faults survived every automated gate and were only found by opening the
window in the author's session. The first is the important one:

- **Nothing responded to a click.** Both projections and the sidebar drove
  selection from a `TapHandler` on the delegate `Item`, with a
  `CelestinaSurface` — a Qt Quick Controls `Pane` — filling that item above it.
  The Control takes the press, so the handler underneath never saw it, and the
  keyboard path worked while the pointer did nothing at all. Every row and cell
  now uses a `MouseArea` stacked above the surface, which is the idiom the rest
  of the suite already uses. Fluorita was the only place that put a handler
  under a `Pane`, and nothing automated could see the difference: `qmllint` and
  the offscreen smoke both construct the tree successfully either way.
- **Tab reached each folder's unmap button before the content**, so two Tabs and
  a Return unmapped a folder. Observed for real: a keyboard walk during this
  session removed one of the author's configured roots, which had to be restored in
  `~/.config/fluorita/sources.tsv`. Those buttons left the tab chain; the
  keyboard now unmaps through `Delete` on the focused row.
- **The empty-folder sentence was printed twice**, once as the header summary
  and once centred, and the unmap glyph resolved to `list-x` — a list-with-a-
  cross that reads as a filter. The header now yields to the centred state, and
  the glyph is a plain `x`.

This is the concrete reason `VAL-FLU-SOURCES` exists and why a smoke is not
evidence of interaction.

## Limits

- A build proves compilation and a smoke proves startup. Neither proves the real
  compositor, the appearance of the sidebar, pointer behaviour, the desktop's
  own folder dialog, or assistive technology. `VAL-FLU-SOURCES` owns all of
  those and is pending.
- Pointer behaviour still has no automated gate. The click fix above was
  reasoned from the stacking rule and the suite's working idiom, then rebuilt,
  re-linted and re-smoked; it was **not** reproduced by a synthetic click,
  because this session had no pointer injection available. Confirming it is part
  of `VAL-FLU-SOURCES`.
- The portal client was exercised only through its unit tests. No real
  `org.freedesktop.portal.FileChooser` exchange was performed: the smoke runs
  with a deliberately absent session bus.
- The running Fluorita or Siderita processes in the author's session, if any,
  keep their old bytes until restarted.
- No ledger unit is closed here: the exact inventories belong to the author's
  commit request, and shared files were being edited concurrently.

## Follow-up

`VAL-FLU-SOURCES` in [VALIDATION.md](../../VALIDATION.md).
