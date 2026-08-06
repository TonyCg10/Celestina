# Evidence: 2026-08-06 a label is text, never markup

- **Date:** 2026-08-06
- **Scope:** `STYLE-G7-D`; plan
  [shared-reading-controls](../plans/active/2026-08-04-shared-reading-controls.md);
  finding `H4` of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
- **Environment:** source correction with the module's own contract checks and
  offscreen QML tests. No production deployment; the shell that consumes these
  controls was neither built nor run, since its GPU safety hold stands
- **Artifact:** none; no production build ran

## What was wrong

The 2026-08-05 shell audit raised producer markup rendering live, and
`LVR-3-B` answered it by setting `textFormat: Text.PlainText` on the shell's own
`Text` items. Two label paths are not `Text` items and were therefore missed: a
notification's action labels are `CelestinaButton`, and another application's
tray menu entries are `GlassMenuItem`. Both render `control.text` through a
`Text` in this module that declared no `textFormat`, so both defaulted to
`Text.AutoText`, which renders anything Qt believes to be markup as rich text.

The strings on those two paths are chosen by other processes. A notification
whose action is labelled `<img src=http://host/x>` makes the shell issue that
request on the producer's behalf; `<a href>` and arbitrary styling follow the
same way. The action label bound is 48 characters and the tray label bound is
256, both far more than such a payload needs.

It belongs here rather than in Celestina because the controls are shared: fixing
it in the consumer would have left the same hole for Siderita, Fluorita and
Grafita, and would have been the second copy of a rule with one owner.

## What changed

- `CelestinaButton.qml`, `GlassMenuItem.qml` — the label `Text` declares
  `textFormat: Text.PlainText`, with the reason stated where the next reader of
  either control will meet it.

## Procedure

```sh
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/qmllint-cxxqt.sh siderita
```

## Result

| Command | Result |
|---|---|
| `check-style-contract.sh` | Contrast contract: OK; QML visual contract: OK |
| `check-architecture-contract.sh` | Architecture contract: OK |
| `check-language-contract.py` | OK, 158 legacy files ratcheted |
| `qmllint-cxxqt.sh siderita` | OK, 336 non-fatal baseline warnings |

## Limits

No notification carrying markup in an action label was delivered to a running
shell, and none can be while the GPU hold stands: this is the change that stops
it being rendered, verified by the module's contracts rather than by watching it
not happen. The observation belongs with the shell's own validation once the
hold ends.

Plain text is set on the two label paths this finding names. Any future control
in this module that renders a string its process did not write needs the same
declaration; nothing enforces that automatically today.
