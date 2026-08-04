# Grafita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md).

## VAL-G7 — Numbered surface and remembered text size

- **Status:** pending
- **Related implementation:** checkpoint G7,
  [plan](docs/plans/active/2026-08-04-g7-reading-comfort.md)
- **Requires:** the author's own compositor, keyboard layout and display scale
- **Procedure:** open a document long enough to scroll and containing a line
  far wider than the window; scroll to the end and back; drag both scroll bars
  and click their empty track; press `Ctrl +` and `Ctrl −` several times,
  including holding one down; hold `Ctrl` and turn the wheel, then turn it
  without `Ctrl`; press `F10` and `Alt + Z`; close Grafita and open it again
- **Pass condition:** every visible line carries exactly one number, level with
  the row it starts on and unchanged by wrapping; the numbers keep up while
  scrolling and stay pinned when the text scrolls sideways; the first column is
  clear of the frame; the footer's line and column match the caret, including
  on a line with non-ASCII characters; dragging a bar moves the text and a bar
  appears only when its axis can scroll; `Ctrl` and the wheel resize while a
  bare wheel still scrolls; the size shortcuts reach the editor from the
  physical layout and stop at their limits; at least one of the wrap bindings
  survives the compositor; the relaunched window uses the last size and wrap
  mode
- **Scope:** Grafita's own window only. Siderita shows the same surface and has
  its own row, `VAL-SID-G7`
- **Result:** pending
- **Evidence:** the agent lane checked, in a nested compositor at a display
  scale and window size that are not the author's: the gutter against a wrapped
  line and against sideways scrolling, single-press size changes with their
  stored file, `F10` with its stored file, a caret column of 455 matching a
  line of exactly 454 characters, and both bars appearing only on the axis that
  can scroll. Not covered there: the `Ctrl` wheel gesture and bar dragging,
  which the synthetic-input tool cannot produce, and `Alt + Z`, which the
  author's compositor claims for itself

## Closed historical observations

`VAL-GRA-EMBEDDED`, `VAL-GRA-STANDALONE` and `VAL-GRA-COMFORT` are preserved in
the [migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).

## VAL-GRA-METADATA — Cross-owner and extended-attribute save

- **Status:** deferred
- **Related implementation:** current loss-free save contract
- **Requires:** a disposable real file owned by another user or a non-temporary
  filesystem carrying representative ACLs and extended attributes
- **Procedure:** edit and save through a consuming Grafita surface, then compare
  owner, group, mode, ACL and extended attributes with the original fixture
- **Pass condition:** every declared metadata field survives unchanged, or the
  save refuses before replacing the original
- **Result:** deferred until a relevant filesystem fixture exists
- **Evidence:** none

## Coverage intentionally outside the current plan

IME, AT-SPI and reduced-motion were explicitly set aside in the version-1
record. They are not pending milestones. If the author requests one, add a
bounded `VAL-GRA-*` case here; a failure then opens a separate corrective
implementation unit.
