# Grafita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md).

## VAL-GRA-FEEDBACK — Tabs, footer and find bar as glyphs

- **Status:** pending
- **Related implementation:** `FEEDBACK-1-GRA` (1.2.3), recorded in
  [the suite evidence](../docs/evidence/2026-09-03-apps-feedback-and-icon-first.md)
- **Requires:** the deployed Grafita on the real session, two documents open
- **Procedure:** hover a tab and cross onto its close glyph; press and hold a
  tab; open find (Ctrl+F), toggle case and whole word, step with the chevrons,
  open and close replace with its glyph and with Ctrl+H; undo, redo, save and
  close from the footer; open the encoding chooser and
  hover a row that is not the current one
- **Pass condition:** the close glyph paints nothing at rest and one circle
  on hover while the tab stays lit; a held tab darkens and sinks; toggles read
  as Selected while on; the bar does not resize when replace opens; every
  glyph is legible without a label and none paints a hover card; the current and hovered
  encoding rows are two different fills
- **Result:** not run by hand
- **Evidence:** any glyph whose meaning was not clear from the glyph alone

## VAL-G7 — Numbered surface and remembered text size

- **Status:** pending
- **Related implementation:** checkpoint G7,
  [plan](docs/plans/archive/2026-08-04-g7-reading-comfort.md)
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

## VAL-G8 — Naming the encoding a file does not declare

- **Status:** pending
- **Related implementation:** checkpoint G8, unit `G8-A` in the
  [plan](docs/plans/archive/2026-08-19-g8-text-already-refused.md);
  [evidence](docs/evidence/2026-08-19-encodings-a-file-cannot-declare.md)
- **Requires:** the author's own compositor and keyboard layout, and a real
  file in an encoding Grafita cannot conclude — a `windows-1252` note, a
  subtitle file, or anything a Windows tool wrote
- **Procedure:** open that file and read what the footer says; press `Ctrl + E`
  and also click the footer's encoding button; move through the list with the
  arrow keys and with the wheel, and choose one with `Enter` and with a click;
  dismiss the list with `Escape` and by clicking outside; choose an encoding
  that is wrong for the file; open a document, type into it, and try `Ctrl + E`
  again; with the document read correctly, save it and compare the bytes with
  a copy made before
- **Pass condition:** the refusal names the file rather than saying only that
  it is not text; the chooser reaches the author from both the key and the
  button, and the current encoding carries its mark; arrow keys, `Enter`,
  `Escape` and the click all do what they look like; a wrong encoding either
  refuses or shows visibly wrong characters and never claims success quietly;
  the button is not offered while there is unsaved work; a file saved after
  being read correctly is byte-identical to the copy
- **Scope:** Grafita's own window. Siderita's embedded editor gained no gesture
  here and reads what the core decides
- **Result:** pending
- **Evidence:** the agent lane checked, headlessly and without a window: that a
  `latin-1` file is refused alone and opens exactly once its encoding is named,
  that saving it untouched reproduces every byte, that a wrong encoding which
  cannot write the file back is refused, and that a dirty document is not
  re-read. Not covered there: whether the key reaches the application from the
  author's physical layout, whether the list reads well at the author's scale,
  and what a wrong-but-valid encoding looks like on screen

## VAL-G9 — The documents Grafita used to refuse

- **Status:** pending
- **Related implementation:** checkpoints G9-G13, unit `G9-A` in the
  [plan](docs/plans/archive/2026-08-19-g9-imported-document.md);
  [evidence](docs/evidence/2026-08-19-documents-grafita-used-to-refuse.md)
- **Requires:** the author's own session, and their own documents — a real
  `.docx` or `.odt` with styles and images, an `.epub`, a `.rtf`, a PDF, and a
  PDF form if they have one
- **Procedure:** open each one and read what the footer says; correct one word
  and save; reopen the file in the application that made it and look at what is
  around the correction; in the PDF, correct a word and then try one that needs
  a letter the document never uses; in a form, change a field and reopen it in
  a viewer; try adding a paragraph and read the refusal; open a document, type,
  and check the encoding button is not offered
- **Pass condition:** the text shown is the document's text, in order, with its
  paragraphs as lines; the original application opens the saved file without
  complaint and every style, image and property is where it was; the PDF shows
  the correction in a viewer and refuses the impossible letter by saying so;
  the form field holds its new value when reopened; the paragraph refusal names
  what it will not do and the file on disk is unchanged
- **Scope:** Grafita's own window. Siderita reads the same core and shows the
  same refusals but gained no gesture here
- **Result:** pending
- **Evidence:** the agent lane checked, headlessly: the byte-for-byte container
  rewrite, the untouched styles of a corrected `.docx` and `.odt`, spine order
  in an `.epub`, rich text's markup after a save, PDF reading over every
  document on the machine with `qpdf` and `pdftotext` confirming the corrected
  files, the glyph refusal on a real document, a form field written and read
  back, and gzip round-tripping its text. Not covered there: whether Word,
  LibreOffice or a reading application is happy with the saved files, whether
  a PDF correction looks right on the page rather than merely reading right,
  and how any of it appears at the author's display scale

## VAL-GRA-SAVEAS — "Guardar como" against the session's file chooser

- **Status:** pending
- **Related implementation:** checkpoint G7, unit `G7-C` in the
  [plan](docs/plans/archive/2026-08-04-g7-reading-comfort.md);
  [evidence](docs/evidence/2026-08-05-loss-free-save-as.md)
- **Requires:** the author's own session, its file-chooser portal and whichever
  backend that portal routes to
- **Procedure:** in a document with no file yet, press `Ctrl+S`; in the chooser
  type a name containing `#` and `%`, accept, and look at the folder; repeat
  and dismiss the chooser instead, then keep editing and save normally later;
  with several tabs open, dirty two of them, quit, and dismiss the chooser the
  sweep raises; in a saved document press `Ctrl+S` twice in quick succession;
  save over an existing symlink
- **Pass condition:** the file created carries the literal name that was typed,
  not its percent-encoded form; a dismissed chooser closes no tab then or
  later, and an abandoned quit sweep leaves every tab where it was; two quick
  saves write once and raise no external-change banner; a
  symlinked destination is still a symlink afterwards and the file it names
  holds the new bytes
- **Scope:** Grafita's own window. Siderita's embedded editor has no "save as"
- **Result:** pending
- **Evidence:** the agent lane covered the rules in `grafita-core` tests only;
  no chooser, portal or compositor was exercised

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
