# Celestina decisions

Accepted shell decisions and their falsifiers live here. They are the nine
decisions of the design's §10, one record each; open questions belong to the
[discussion queue](../discussions/README.md).

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-pure-rust-shell-on-wayland.md) | accepted | A pure Rust shell on Wayland, for niri only |
| [0002](0002-halted-qt-shell-is-history.md) | accepted | The halted Qt shell is history at `celestina-qt/`; salvage is by copy |
| [0003](0003-no-dock.md) | accepted | No dock |
| [0004](0004-femtovg-behind-a-painter.md) | accepted | femtovg behind a Painter contract |
| [0005](0005-cosmic-text-only-text-stack.md) | accepted | cosmic-text is the only text stack |
| [0006](0006-tokens-are-data.md) | accepted | Tokens are data |
| [0007](0007-focus-policy.md) | accepted | No keyboard at rest, on demand for a page, exclusive for the launcher |
| [0008](0008-claim-free-names-only.md) | accepted | Claim well-known names only when free |
| [0009](0009-one-process-one-loop.md) | accepted | One process, one loop, worker threads |

New records use the suite [decision template](../../../docs/templates/decision.md).
