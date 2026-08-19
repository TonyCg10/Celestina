# Siderita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md). Each failed row keeps its result and opens a new
corrective implementation unit.

## VAL-SID-07 — Compressing and extracting on the live session

- **Status:** pending
- **Related implementation:** `SID-A1`
- **Requires:** a verified Siderita artifact on the real Niri/Wayland session, a
  disposable tree and archives made elsewhere (`zip`, `tar czf`, and one whose
  members carry accents and spaces)
- **Procedure:** extract each archive from the entry menu, extract the same one
  twice into the same folder, compress a folder and a multi-selection in both
  containers, and cancel a long run part-way
- **Pass condition:** the extracted tree matches the original byte for byte, the
  second extraction lands beside the first instead of over it, the archives open
  in another manager, and a cancelled run leaves neither a partial archive nor a
  staging folder behind
- **Result:** not run by hand
- **Evidence:** the archives used, the resulting listings and any failure text

## VAL-SID-01 — Drag comfort and live menu glass

- **Status:** pending
- **Related implementation:** completed CP4
- **Requires:** verified Siderita artifact on the real Niri/Wayland session and
  a disposable source/destination tree
- **Procedure:** spring-open a folder during drag, edge-scroll while dragging,
  reorder sidebar rows, then open/scroll several live-capture menus while
  recording frame timing
- **Pass condition:** every drag reaches only its intended target, reorder
  persists, no covered row receives input and menu blur frame-time p95 is at or
  below the recorded 16.7 ms budget
- **Result:** not run against the current surfaces
- **Evidence:** record fixture, output scale, gestures and timing samples

## VAL-SID-02 — File chooser in daily portal use

- **Status:** pending
- **Related implementation:** completed CP5
- **Requires:** explicitly opted-in portal routing plus one GTK application,
  one Qt application and one browser named in the evidence
- **Procedure:** in each of those three clients exercise open, multi-open, save,
  save-multiple, directory, filters, cancellation and application exit; repeat
  one open request with the opt-in route disabled as the control
- **Pass condition:** requests map once, return the selected URIs or cancellation
  correctly, never strand a backend, and the disabled control remains with its
  pre-existing chooser without changing the other two clients' configuration
- **Result:** not run as the author's sustained default chooser
- **Evidence:** dated application/request list and any failure logs

## VAL-SID-03 — Reduced motion, focus and assistive technology

- **Status:** deferred
- **Related implementation:** completed accessibility foundation plus future
  local follow-up after STYLE-M1
- **Requires:** verified Siderita/CelestinaStyle artifacts, real keyboard and
  AT-SPI stack
- **Procedure:** traverse main view, menus, operation dialogs, picker and both
  embedded surfaces with reduced motion off/on; inspect focus containment,
  restoration, roles, selected/progress/error state and actions
- **Pass condition:** every action remains operable and announced, focus never
  escapes a modal and spatial/scale motion disappears in reduced mode
- **Result:** deferred until STYLE-M1 and the required AT-SPI stack are available
- **Evidence:** dated surface matrix and AT observations

## VAL-SID-04 — Portal transient parenting

- **Status:** deferred
- **Related implementation:** SID-M1
- **Requires:** SID-M1 verified artifact, an opted-in portal requester and real
  Wayland compositor support
- **Procedure:** open pickers from two distinct applications, inspect their
  parent/stacking/minimise lifecycle, then cancel one requester while both exist
- **Pass condition:** each picker belongs to the correct requester, never steals
  another request, closes with its parent and still degrades safely when given
  no usable parent handle
- **Result:** deferred until SID-M1 produces its verified artifact
- **Evidence:** window tree plus dated requester outcomes

## VAL-SID-05 — Destructive verbs and portal answers with a real requester

- **Status:** pending
- **Related implementation:** SID-G7-C
- **Requires:** the deployed Siderita binary and portal backend, a real Wayland
  session, one sandboxed or portal-using application that opens and saves files,
  a folder holding a symlink to another directory, and a scratch folder that can
  be safely written into during the test
- **Procedure:** copy a file and paste it into the folder it already lives in;
  start a large copy and, while it runs, try "Enviar a la papelera" from the
  context menu; watch a folder while another process writes into it; activate a
  symlink that points at a directory; permanently delete one entry from the
  trash while the list holds several; drag files from another manager into
  Siderita, including one whose name is not valid UTF-8; then, from the other
  application, open a file read-only and save over an existing name
- **Pass condition:** the paste duplicates instead of removing the original; the
  trash verb is refused while the copy runs and the copy stays cancellable; the
  watched folder never flashes a read error; the symlink opens as a folder; the
  purge removes the entry that was chosen; the drop pastes every file it can
  and reports the ones it cannot; and the requester receives no write access it
  did not ask for and is asked before overwriting
- **Result:** not run
- **Evidence:** none

The unit's automated lane is compilation and unit tests, recorded in the
[destructive-operation guards evidence](docs/evidence/2026-08-05-destructive-operation-guards.md).
It cannot serve a real portal request, and this project has already seen a
picker look broken for a reason that lived entirely in the requester.

## VAL-SID-06 — A file whose name is not valid UTF-8, by hand

- **Status:** pending
- **Related implementation:** `SID-G7-D` and `SID-G7-E`,
  [ADR 0008](../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md),
  [seam evidence](docs/evidence/2026-08-06-byte-exact-path-seam.md) and
  [thumbnail and clipboard evidence](docs/evidence/2026-08-06-thumbnail-and-clipboard-bytes.md)
- **Requires:** the deployed Siderita binary on the author's real Wayland
  session, a scratch folder, and one file created there whose name is not valid
  UTF-8 — for example
  `python3 -c "open(b'/tmp/scratch/na\xffme.txt','w').write('hola')"` — plus one
  other file manager or portal-using application to drag to and from
- **Procedure:** browse to that folder; read the name the row shows; press
  `Space` on the entry, then `Enter`; rename it; star it and use the sidebar
  star to reveal it; give it a custom icon and an accent; open its properties;
  copy it and paste it into another folder; cut it and paste it back; drag it
  into the other application and drag it back; send it to the Trash and restore
  it from there; restart Siderita and check the star, the icon and the tab; then
  walk the breadcrumbs of a folder whose own name is not valid UTF-8, and use
  the save picker of the other application to write a new file into it
- **Pass condition:** the row shows the name with a replacement character where
  the byte is, and **every** verb above acts on that exact file rather than
  reporting that it no longer exists; the breadcrumbs navigate; the star, the
  icon and the reopened tab survive the restart; the drag out hands the other
  application a URI it can resolve; and the picker writes where it said it would
- **Result:** not run
- **Evidence:** the exact bytes of the name, the compositor, and the other
  application named

The two limits recorded in advance are no longer limits: `SID-G7-E` gave the
thumbnail provider the file's bytes and made the system clipboard exchange
percent-encoded URIs, so both are now part of what this run checks. Add to the
procedure: look at the entry when it is an image — it should show a real
thumbnail, not a glyph — and, in the copy to the other application, paste it
there and copy something back. What remains outside this row is a filename
containing `;`, which Qt and GLib percent-encode differently and which therefore
lands on two different thumbnail cache entries; that interop limit is in the
[evidence](docs/evidence/2026-08-06-thumbnail-and-clipboard-bytes.md)'s `Limits`
and is not a Siderita defect.

## VAL-SID-G7 — Numbered text panes and the shared text size

- **Status:** pending
- **Related implementation:** checkpoint SID-G7,
  [plan](docs/plans/active/2026-08-04-shared-reading-surface.md)
- **Requires:** the author's own compositor, keyboard layout and display scale,
  and a text file long enough to scroll with at least one line far wider than
  the dialog
- **Procedure:** press `Space` on that file to open the quick look, then open it
  in the embedded editor; scroll both to the end and back; drag each scroll bar
  and click its empty track; press `Ctrl +` and `Ctrl −` in each surface; move
  the caret onto a line containing non-ASCII characters; change the size in
  Grafita's own window, then open a Siderita surface again
- **Pass condition:** every visible line carries exactly one number, level with
  the row it starts on and unchanged by wrapping; the numbers stay pinned when
  the text scrolls sideways; the footer's line and column match the caret,
  including on the non-ASCII line; no encoding label remains; the size shortcuts
  reach both surfaces from the physical layout through the modal and stop at
  their limits; and a size changed in Grafita is the size the next Siderita
  surface opens at, in both directions
- **Result:** pending
- **Evidence:** dated observations naming the file, the output scale and the
  compositor

## Closed historical observations

`VAL-SID-BASE`, `VAL-SID-GRAFITA` and `VAL-SID-FLUORITA` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).
