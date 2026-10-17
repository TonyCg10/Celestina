# Celestina decisions

Accepted shell decisions and their falsifiers live here, one record each. ADR
0001 to 0009 are the decisions of the CEL-1 design's §10; ADR 0010 to 0015 are
the decisions of the parity program's §10; ADR 0016 is the one of the
CEL-2-K design's §9. Open questions belong to the
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
| [0010](0010-parity-program-in-thirteen-units.md) | accepted | The parity program is delivered in thirteen units, A to M |
| [0011](0011-duties-switched-on-one-by-one.md) | accepted | Duties are switched on one by one, off while Noctalia holds them |
| [0012](0012-bar-one-surface-per-output.md) | accepted | The bar is one surface per output with the Island in the centre zone |
| [0013](0013-everything-is-a-piece.md) | accepted | Everything the bar holds is a piece behind one contract |
| [0014](0014-lock-and-polkit-carry-over.md) | accepted | Lock and polkit carry ADR 0004 and 0005 over, with their own review |
| [0015](0015-clock-rests-at-minutes.md) | accepted | The clock rests at minutes; seconds run only while the Island is open |
| [0016](0016-weather-is-the-only-network-and-manual.md) | proposed | The weather is the only network use and its location is manual |

New records use the suite [decision template](../../../docs/templates/decision.md).
