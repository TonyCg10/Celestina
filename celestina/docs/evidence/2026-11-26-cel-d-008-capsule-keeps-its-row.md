# Evidence: the capsule that keeps its row, the open piece as the menu's subtitle (CEL-D-008)

- **Date:** 2026-11-26
- **Scope:** `CEL-D-008`: `celestina/src/sheet`, `celestina/src/island`, `celestina/src/bar`, `celestina/src/pieces`, `celestina/src/osd`, `celestina/crates/celestina-ui`, `celestina/crates/celestina-paint`, their tests, the ledger row and `VAL-CEL-D-008`
- **Environment:** worktree `celestina-CEL-D-008` stacked on `unit/celestina/CEL-D-007` at `55095169`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** version 4 of the whole-bar preview, approved by the author on 2026-11-26 (an artifact private to the author)

## Procedure

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
scripts/smoke.sh --binary PATH --only bar,osd,notify
```

The nested look ran through the guarded kit (`env -i`, `--own-bus`, a fake
`wpctl` graph, nested niri, a 2560 by 1440 logical output at scale 1.5): a
probe pressed the pieces with a virtual pointer and took screenshots through
the nest's own niri, in bursts of eight frames about 40 ms apart right after a
press, so that the 350 ms open, the 150 ms switch and the 60 plus 200 ms close
flights are caught mid-motion. Every crop was read.

## Result

- **Capsule keeps its row:** [bar-rest](2026-11-26-cel-d-008-capsule-keeps-its-row/bar-rest.jpg) the bar at rest (workspaces, apps, system load, volume, clipboard, power); [open-volume](2026-11-26-cel-d-008-capsule-keeps-its-row/open-volume.jpg) the volume sheet open at rest: the other pieces of the row (workspaces, apps, system load, clipboard, power) stay in place with no gap where the volume piece was, and the body starts with the subtitle, the icon at 22 px and "40 %"
- **Flight down:** [open-mid](2026-11-26-cel-d-008-capsule-keeps-its-row/open-mid.jpg) six frames from the press: the frame before it, then the volume icon and its "40 %" leaving the slot, growing and turning semibold on their way to the subtitle while the row closes over the slot, and the body fading in once they land
- **Switch:** [switch-to-session](2026-11-26-cel-d-008-capsule-keeps-its-row/switch-to-session.jpg) six frames from the press on the session piece while the volume sheet is open: the volume icon and text fly up to the reopened slot (the piece is not painted in the bar until they land) while the session icon flies down on its own, with no piece drawn twice; [session-rest](2026-11-26-cel-d-008-capsule-keeps-its-row/session-rest.jpg) the session sheet settled, the volume piece back in its row, the subtitle showing the menu's name (the piece has no text)
- **Close:** [close-mid](2026-11-26-cel-d-008-capsule-keeps-its-row/close-mid.jpg) six frames from the press outside: the slot reopens at once, the sheet shrinks to the capsule, and the power icon flies from where the subtitle was over the bar to the slot and lands on it
- **Text-only pieces:** [sysmon-open-mid](2026-11-26-cel-d-008-capsule-keeps-its-row/sysmon-open-mid.jpg) the system load, drawn as one text node, opens: its text flies from the slot to the subtitle (growing, turning semibold) while the page icon fades in with the body; [sysmon-switch-to-session](2026-11-26-cel-d-008-capsule-keeps-its-row/sysmon-switch-to-session.jpg) switching away from it with its subtitle on screen: the text flies back up to its slot and the session icon flies down, the leaving body staying under its own piece; [session-close-mid](2026-11-26-cel-d-008-capsule-keeps-its-row/session-close-mid.jpg) the session sheet closing
- **The subtitle answers its piece:** while a sheet is open the subtitle row (not its overflow button) takes the piece's gestures through the bar's own path: the wheel steps the workspaces or the level, a right or middle click does the piece's own action, a plain click closes the sheet. The subtitle has no hover or press plate of its own (reusing the bar's would need the piece's node, which is out of the row). Tests: `the_wheel_steps_with_the_sheet_open_and_keeps_it_open` (workspaces sheet, wheel over the subtitle), `a_right_click_on_the_volume_subtitle_mutes_and_a_click_closes`. A right or middle click that does a piece action closes the sheet, as it did on the open piece before this unit; one the piece has no action for does nothing. A click there needs its press there, a wheel over the subtitle of a piece without a wheel action scrolls the body
- **Review round 1 fixes:** text-only flights; the leaving body painted under its own slot; scrolled subtitles; the bar glyph in the subtitle (the bell crossed out, the network's link, cloud and unplug glyphs; the Bluetooth subtitle keeps the `TextHi` tint of the flyer, not the bar's accent); flyers resolved before any paint; a resize at rest morphs (the slide latch is time-bound); a placement change during a lift keeps its glide; the bar hits what is painted while the row slides; the weight carries over on a takeover; the flyer fades in over a fading subtitle; the end-of-row slot starts where the piece was
- **Tests:** the flight is covered by pure tests of its timing and interpolation (`sheet::flight`) and by Island tests (`island::tests::flight`): the flight down on the open spring (frames at 0, 50 % and the end: icon 18 to 22, text 13 to 15, weight cross-fade), the close (slot first, flight 60 ms later, 200 ms), the switch (old piece up in 150 ms, the new one down, one flyer each, nothing painted twice), the piece and the subtitle hidden until the flight lands, reduced motion (no flight), a piece that vanishes (its flight is dropped), the live text (the flyer and the subtitle follow a volume that moves during the flight), an icon-only flight for a piece with no text, the only piece of a capsule, and no frame asked at rest
- **Review round 2 fixes (by the controller, after the last round):** a gesture the piece has no action for keeps the sheet open; a release on the subtitle whose press began in the body is no click; a wheel over the subtitle of a piece without a wheel action goes to the body; an up flight whose text node is gone flies its icon; a capsule moved during a lift's glide glides again, its glass and regions on it. Tests: `a_gesture_the_piece_has_no_action_for_keeps_the_sheet_open`, `a_release_on_the_subtitle_after_a_press_elsewhere_is_no_click`, `only_a_piece_with_a_wheel_action_takes_the_wheel_over_its_subtitle`, `a_text_node_gone_mid_flight_leaves_the_icon_to_fly`, `a_capsule_moved_during_a_glide_keeps_its_glass_and_regions_on_it`; each seen failing with its fix reverted. No new nest run for these input and timing fixes
- **Review:** 2 rounds (Opus, high; the implementers were Sonnet). Round 1: 0 critical, 2 important (the open piece's gestures, decided by the author on 2026-11-26: the subtitle takes them; text-only pieces did not fly), 7 minor, 4 nits, all fixed. Round 2 re-read the fixes: 0 critical, 0 important, 4 minor and 2 nits; the controller fixed five of them, and one is a limit below
- **Exit:** every command exits 0; 2617 tests passed, 0 failed (workspace); the guards (architecture, documentation, language) pass; smoke: OK (only: notify,bar,osd), the bar at rest with `rest_frames=0`
- **Progressive blur:** the painter keeps it unused by the sheet; the sheet no longer cuts the row, and the author decided on 2026-11-26 to keep it for later

## Limits

- The open piece is out of the bar while its sheet is open, so pressing it is not possible: pressing the subtitle closes the sheet (the subtitle takes the piece's gestures, see Result). Escape, a press outside and the loss of the keyboard focus close it too; a press on another piece switches.
- A press on the empty row of a capsule whose only piece is open is swallowed (it neither closes the sheet nor reaches anything).
- A change of placement while the lift's glide does not run (an output that changes size, a layout reload) makes the capsule jump to its new place instead of gliding; one that comes during the glide leaves the glide be.
- What still flies nothing or part of it: faces without text fly only the icon (battery, session, the pieces whose page has none; the subtitle shows the menu's name and its text appears with the body); a piece whose glyph is drawn without an icon node flies its text if it matches, else nothing (the Wi-Fi bars of the network piece, the battery body); the workspaces piece (a row of dots) flies nothing; text-only pieces (the system load, the clock) fly the text and the subtitle's icon fades in with the body. The bar's bare numbers (the battery's) are not flown.
- The brightness sheet of several monitors lost its "All" heading: the subtitle shows the level of the bar's own output, as the piece does, above the slider that moves every monitor.
- The weight cross-fade paints the text twice (regular at `1 - k`, semibold at `k`) because femtovg cannot interpolate a weight; the two runs are laid out every frame while a flight runs, which is a few frames of short strings.
- The nested frames are 40 ms apart at best and the frame times are not recorded, so the shots show the shape of the motion, not its timing; the timing is asserted by tests. The look in the author's real session, at the author's scale, is left to `VAL-CEL-D-008`.
- Test-first order: the flight module and its wiring were written before their tests, which were then run and corrected; no test was seen failing before its code existed.
- The bar hits what is painted while its row slides (350 ms after an open, a switch or a close), so a press on a sliding piece released about 100 ms later without moving can land on another node and make no click.

## Follow-up

None.

## Landing

- **Base revision:** `acaa31b1f17994a025e068bb0cf4f06603e10608`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:35998d7d07b127a40efaa4f8dd5876b88890631bfc6041e4a83c9722146d4ef4, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
