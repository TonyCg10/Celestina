# Evidence: top bar and segments

- **Date:** 2026-09-30
- **Scope:** STYLE-G7-P — celestina-style
- **Environment:** CachyOS / Qt 6.11.2 / offscreen QPA for tests
- **Artifact:** not applicable

## Procedure

```sh
$ bash celestina-style/scripts/check-style-contract.sh
Contrast contract: OK
QML visual contract: OK

$ bash scripts/check-architecture-contract.sh
Sealed colour contract: OK (4 colour(s))
Contrast contract: OK
QML visual contract: OK
Radius contract: OK
Architecture contract: OK

$ python3 scripts/check-language-contract.py
Language contract: OK (144 legacy file(s) ratcheted)
```

Negative fixture for the trailing-slot guard (`celestina-style/tests/ProbeTrailing.qml`,
scratch, deleted after the check):

```qml
import QtQuick
import CelestinaStyle
CelestinaTopBar { trailingData: [ CelestinaButton { text: "Guardar" } ] }
```

```sh
$ bash celestina-style/scripts/check-style-contract.sh
celestina-style/tests/ProbeTrailing.qml:3:CelestinaButton inside a top bar's trailing slot
ERROR: a top bar carries icon-only actions; move the words to a dialog

Contrast contract: OK
exit=1
```

Deleting the scratch file restores `exit=0` (`QML visual contract: OK`).

### CelestinaSegmentedControl — ctest (offscreen)

```
PASS   : celestina_style_modal::CelestinaSegmentedControl::initTestCase()
QWARN  : celestina_style_modal::CelestinaSegmentedControl::test_an_echoing_host_walks_the_segments() QQuickItem: Cannot set activeFocusOnTab to false once item is the active focus item.
QWARN  : celestina_style_modal::CelestinaSegmentedControl::test_an_echoing_host_walks_the_segments() QQuickItem: Cannot set activeFocusOnTab to false once item is the active focus item.
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_an_echoing_host_walks_the_segments()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_arrows_emit_and_never_move_the_index_themselves()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_click_activates()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_geometry_is_the_bar_slot()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_icon_only_hides_the_labels_and_keeps_the_names()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_one_tab_stop_lands_on_the_current_segment()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_reduced_motion_lands_the_fill_at_once()
PASS   : celestina_style_modal::CelestinaSegmentedControl::test_roles_and_names()
PASS   : celestina_style_modal::CelestinaSegmentedControl::cleanupTestCase()
```

The stateless rewrite (fix round 1) replaced the brief's hidden left/right
cursors with `move(delta)` as `control.activated((control.currentIndex +
delta + count) % count)` — the control never writes its own `currentIndex`,
only emits `activated(index)`; the host echoes it back
(`onActivated: function(index) { currentIndex = index }`), exactly the
pattern the gallery snippets use. The two `QWARN` lines above are the
echoing-host test's one-tick transition where a segment's `focusPolicy`
binding flips to `Qt.NoFocus` for one event-loop tick while `currentIndex`
moves away from it before the new segment takes focus; harmless, and
`tryCompare(segment(2), "activeFocus", true)` confirms focus lands correctly
immediately after.

### CelestinaTopBar — ctest (offscreen)

```
PASS   : celestina_style_modal::CelestinaTopBar::initTestCase()
PASS   : celestina_style_modal::CelestinaTopBar::test_height_and_fill()
PASS   : celestina_style_modal::CelestinaTopBar::test_role_and_focus()
PASS   : celestina_style_modal::CelestinaTopBar::test_title_sits_left_after_the_leading_slot_and_elides()
PASS   : celestina_style_modal::CelestinaTopBar::test_trailing_sits_right()
PASS   : celestina_style_modal::CelestinaTopBar::cleanupTestCase()
```

Focus order follows the bar's `RowLayout`: leading slot (the segmented
control) before trailing slot (the icon buttons), with the bar itself never
taking focus (`activeFocusOnTab: false`, `Accessible.role: ToolBar`) — the
offscreen focus pattern copied from `tst_switch.qml` into both `tst_topbar.qml`
and `tst_segmented.qml`'s `init()` (`testWindow.requestActivate()` +
`tryCompare(testWindow, "active", true)`, then
`forceActiveFocus()` + `tryCompare(item, "activeFocus", true)`) is what makes
the Tab/activeFocus assertions settle under `QT_QPA_PLATFORM=offscreen`,
where window activation does not happen synchronously.

### Gallery smoke (this task)

```sh
$ QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen; echo "gallery exit $?"
gallery exit 124
```

No QML warning on stderr (an unresolved icon name would print one; the
gallery's new section uses only `gauge`, `view-list`, `cpu`, `search`,
`view-grid`, `view-refresh`, `go-previous`, `x`, `view-details`, every one of
which exists in `celestina-style/icons/`).

## Result

- **Exit:** every guard `OK`/exit 0; ctest `1/1` passed; gallery smoke exit
  124 (the expected `timeout` kill of a long-running offscreen window, with
  no QML warning on stderr)
- **Observed:** `CelestinaTopBar` and `CelestinaSegmentedControl` render in
  the gallery's new "TOP BAR AND SEGMENTS — THE DESKTOP FRAME" section: a
  full bar with leading segments and three trailing icon actions, a bar with
  only a back action, a bar with a title long enough to force elision and one
  trailing close action, and a bare icon-only segmented control
- **Trailing-slot guard, list form:** the scratch
  `trailingData: [ CelestinaButton { text: "Guardar" } ]` fixture printed
  `ProbeTrailing.qml:3:CelestinaButton inside a top bar's trailing slot` and
  the `ERROR` line, `exit=1`
- **Trailing-slot guard, single-child form:** the scratch
  `trailingData: CelestinaButton { text: "Guardar" }` fixture printed the
  same hit line and `ERROR` line, `exit=1`
- **Trailing-slot guard, positive run:** with the scratch file deleted,
  `QML visual contract: OK`, `exit=0`

## Limits

- Offscreen proves construction and the asserted geometry/focus only;
  perceptual checks (blur, concentricity at scale, real Tab order read by eye)
  are `VAL-STYLE-08`.
- A `QWARN` about `activeFocusOnTab` appears for one tick in the
  echoing-host test (`test_an_echoing_host_walks_the_segments`); it is a
  harmless mid-transition warning, not a defect — see above.

## Follow-up

`VAL-STYLE-08`

## Landing

- **Base revision:** `6cdc2f97ed4cc8f4f3c96dc9475a433854ba2f18`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8cc19602e3c1c8da2c48da3bc7af6c82e1dadc438a33fcbef4429a1d1e4a7e21, verification_fingerprint sha256:059e299969c40e316efbd1dedd2d5d68576c2062b1ab003d049ac3c63c504582; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:3b304f586f6100a053b1496a37e69ea0ece8f0783977303ad156831bb349d2b8, verification_fingerprint sha256:10d751d8a09bbbd5d9e346e9620f3b68af743db3fabec75054bfd35de5ca3a9b; siderita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:50d43327de39cff92d2a7f6a22d069a8c013a28b6d692f542119cf93985ecd2e, verification_fingerprint sha256:6617b13ff442a9a90e209b90a5ce3340d73feed35e9665ee40fae19490df488a; magnetita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:92a656b40e303121a6640beaa86c05ebb7cb0baccc4c5a97a0dddf0522dc9114, verification_fingerprint sha256:8720ad62ca33607d2e057fe746b943a731d5f28984b94ee841deb2385ed3f9d0; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:42f29b417078a7b613fccab7027fd99c2f7d925f542fe988c92e538cf31963bf; grafita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:dc706bafeceed3cab7df2ef2d5af87bfc61669778fcfa5d78579969c65bf5d0f, verification_fingerprint sha256:6f5b6e144d760b2bad7a8e51653b6f7dd3188b73019901d722fdcffd04f9251f; fluorita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:cfe5c7329d896461e447f74ff6e00b3288ff4f3e82e9318327b88c861f45956d, verification_fingerprint sha256:cca39cab1421055650130391f01bac54c7f787cc09b562e516c69a5472445527; hematita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:593eeabe6453eb6af82856638958f2e40d3a0c68e60d7f964362ea30bc124d7d, verification_fingerprint sha256:50864c95bc9fde43e1c8f14e3a50cfcdf13858e6b07ba55457ab2c4bbab8f98c
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
