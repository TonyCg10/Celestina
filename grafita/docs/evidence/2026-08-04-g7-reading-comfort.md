# Evidence — G7 reading comfort

- **Date:** 2026-08-04
- **Scope:** Grafita `G7-A`; plan
  [g7-reading-comfort](../plans/active/2026-08-04-g7-reading-comfort.md)
- **Environment:** Linux 7.1.5-1-cachyos, Qt 6, CXX-Qt, offscreen Qt platform
  for every automated surface check; the author's checkout carried the Siderita
  `SID-G7-A` unit uncommitted while this ran, since it is delivered by the
  commit that follows this one
- **Artifact:** `grafita/target/release/grafita`, sealed in
  `grafita/target/production-artifact.toml` and deployed to
  `/home/toni/.local/bin/grafita`
- **Related author validation:** `VAL-G7` (pending, not claimed here)

## Procedure

Suite guards over the whole checkout:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
```

Version transition before the canonical build, so the deployed binary carries
the delivered number rather than a placeholder:

```sh
python3 scripts/version_tool.py bump grafita milestone \
  --unit G7-A --summary "Add the reading comfort checkpoint"
```

Registered completion, which builds once, verifies those exact bytes and
deploys them without recompiling:

```sh
bash grafita/scripts/complete-production.sh
```

## Result

- **Exit:** 0 for every command above.
- **Observed:**

```text
Contrast contract: OK
QML visual contract: OK
Architecture contract: OK
version-contract: OK (6 owners)
grafita: 1.0.0 -> 1.1.0 (milestone)
manifest: grafita/target/production-artifact.toml (verified)
artifact: grafita current and verified
installed: OK /home/toni/.local/bin/grafita
```

Inside the completion entry: the architecture guard, `cargo fmt --all --check`,
`cargo clippy --all-targets --locked -- -D warnings`, and
`cargo test --all-targets --locked` for `grafita` (7) and `grafita-core`
(80 + 23 + 18 = 121, all passing); `qmllint-production.sh` (OK, 59 non-fatal
baseline warnings) over `org.celestina.grafita`, which is what proves both
shared QML files are registered through `build.rs`; and the offscreen smoke
(binary live 8 s, no QML error, no auto-binding).

### New automated coverage

`grafita-core` gained tests for the stored preferences and the caret mapping:
parsing a preferences file and ignoring it when absent, clamping a stored value
that is out of bounds or not a number, a size nudge stopping at each limit
without reporting a change, and the mapping from a widget UTF-16 offset to a
line and a **character** column — including an offset that falls inside a
surrogate pair, which is the case a column counted in UTF-16 units would get
wrong.

### Retired debt

- **Resolved language debt:** `grafita/qml/components/DocumentHeader.qml`

Removing the encoding label removed the file's last non-English string, so its
`scripts/language-baseline.tsv` row is deleted in this same commit rather than
left as a ratchet the file no longer needs.

## Limits

- A build, a lint and an offscreen smoke prove compilation, registration and
  startup. They do not prove Wayland, the author's compositor, display scale,
  physical keyboard layout, pointer input or AT-SPI.
- Not covered automatically and left to `VAL-G7`: the `Ctrl` wheel gesture and
  scroll-bar dragging, which the synthetic-input tool cannot produce, and
  `Alt + Z`, which the author's compositor claims for itself — which is why the
  same command is also bound to `F10`.
- The shared components' own registration is proven by their owner's evidence,
  not here.

## Follow-up

`VAL-G7`, and the sibling units
[`STYLE-G7-A`](../../../celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md)
and
[`SID-G7-A`](../../../siderita/docs/plans/archive/2026-08-04-shared-reading-surface.md).
