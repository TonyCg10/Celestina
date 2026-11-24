# Evidence: CEL-2-C-KEYS — the level widget's node keys

- **Date:** 2026-11-23
- **Scope:** `celestina/src/pieces/level.rs` (the widget the Volume and
  Brightness pieces share), the two pieces' ids, one new test that draws the
  whole piece table, the test fixtures it needs (`pieces::tests_support`,
  eight constructors moved there from six pieces' test modules), the bar's
  two unnamed keys given a name in `celestina/src/island/bar_tree.rs`, the
  "Adding a piece" rule in `celestina/src/pieces/mod.rs`, and this unit's
  ledger row. No behaviour a person sees, no dependency and no version
  changed by the session.
- **Environment:** CachyOS, worktree `celestina-CEL-2-C-KEYS` cut from `main`
  at `974a7490` (celestina 1.13.2); toolchains 1.97.1 (pinned) and 1.99 (the
  runner's stable). No shell was started; nothing touched the live session.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

Reported from the author's session on 1.13.2, the first time the Brightness
piece was ever drawn beside the Volume piece (CEL-2-C-DDC made it appear):
on every rebuild of the bar the journal said

```text
WARN celestina::bar::view: bar: two pieces draw the same node key; the first one gets its input first=volume second=brightness
```

Both pieces build their view with `pieces::level::widget`, which keyed its
icon and its percentage `level-icon` and `level-percent` whatever piece asked.
The bar hands a node's input to the piece that owns its key, the first to draw
it (`bar::view::own`), so those two nodes of the Brightness piece belonged to
Volume. The widget's own node, the only interactive one, was already keyed per
piece (`volume`, `brightness`), which is why clicks and wheel steps reached
the right piece; the defect was the warning on every rebuild and an input
route that would have gone wrong the day an inner node took input.

## Procedure

```sh
cargo test --lib pieces::key_tests            # the new test
# three mutations, each restored byte for byte (sha256) before the next:
#   1. the two inner keys of level.rs put back to the shared names
#   2. the System monitor's text keyed as the bar's end capsule
#   3. the media state left out of the fixture's context
cargo test                                     # in celestina/
cargo clippy --all-targets -- -D warnings      # 1.97.1
cargo +1.99 clippy --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

`widget` takes the piece's id and keys every node from it (`volume`,
`volume-icon`, `volume-percent`; the same for `brightness`).

The new test `no_two_pieces_of_the_table_draw_the_same_node_key` draws every
piece of `PIECES` under `pieces::tests_support::Everything`, a state in which
all sixteen have something to show: a workspace on the bar's output, an unread
notification, a tray item, a system monitor and a weather reading, today's
date, a paused clipboard, a ready connectivity state with a charging battery,
audio and brightness levels, a connected phone, six minimised windows (one
with an icon) and a playing player. It fails, naming them, for

- any piece that drew nothing, so a piece added to the table without its
  state in the fixture fails instead of being skipped;
- any key two pieces draw, as the pair of piece ids;
- any key of a piece that is one of the bar's own (`bar-root`, `bar-measure`,
  the four `bar-zone` capsules), as `(the bar)` and the piece's id. The bar
  finds its capsules by key, so those are taken from the code that draws them
  (`island::bar_tree::own_keys`, test-only), not typed again.

Under that state the sixteen pieces draw 61 nodes, which is every key the
review's table lists for each piece. No clash was found beyond the one this
unit fixes: no other pair of pieces shares a key and no piece draws one of
the bar's.

The context is `tests_support::with_state` with the fixture's slices laid
over it (the `..*base` pattern of `BarTree::cx`), not a second `BarCx`
literal. The constructors the fixture shares with the pieces' own tests
(`workspace`, `sys_reading`, `weather_reading`, `phone_device`,
`bubble_window`, `bubbles_state`, `media_player`, `media_state`) moved to
`tests_support` from the test modules of the Workspaces, System monitor,
Weather, Phone, Bubbles and Media pieces, which now import them; no assertion
of those modules changed. `bar_tree.rs` names its root and measuring keys in
two private functions so the test and the bar read the same ones.

| Command | Exit | Output |
|---|---|---|
| `cargo test --lib pieces::key_tests` | 0 | 1 passed |
| mutation 1 (the shared level keys put back) | 101 | `sharing a node key: [("volume", "brightness"), ("volume", "brightness")]` |
| mutation 2 (a piece draws a capsule's key) | 101 | `sharing a node key: [("(the bar)", "sysmon")]` |
| mutation 3 (no media state in the context) | 101 | `pieces the fixture draws nothing for: ["media"]` |
| `cargo test` | 0 | 1672 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` (1.97.1 and 1.99) | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| the three guards | 0 | OK |

## Review

An independent review (Opus, read-only) found no Critical and no Important
finding, and six Minor ones:

- M1 (the test drew 7 of the 16 pieces), M2 (a second `BarCx` literal), M3
  (piece keys were not checked against the bar's own) and M4 (the "Adding a
  piece" rule did not say how to key nodes) are done in this unit, as the
  Result describes.
- M5 (two short `String`s per level piece per rebuild) is accepted: a rebuild
  happens only when the bar is stale, never per frame.
- M6 (the commit kind): the unit lands as a `bug`, the kind the review
  names for a warning seen in the field; the landing sets it and the version.

## Limits

- The fixture draws one state per piece. It holds every key a piece draws
  today, but a node a later change draws only in another state (an error
  glyph, say) is compared only once the fixture's state shows it; the test
  checks that every piece drew, not that every node of it did.
- The test compares keys across pieces and against the bar. It does not look
  for a key one piece draws twice, and it does not check the prefix itself:
  a key that is neither rooted in the piece's id nor shared passes.
- Nothing ran on a display. Whether the warning is gone from the journal is
  the author's to see with VAL-CEL2-OSD-PIECES.

## Landing

- **Base revision:** `974a74905e2edc182195d35505128f58eccf7387`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d1db88795c1b785e6796f09376dd48c387ce396458844d4c72bc12410f260d20, verification_fingerprint sha256:ecee1404efbeee86461a6e0bb598e2a25f0631a893343f1b9f44df268b0131d4
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
