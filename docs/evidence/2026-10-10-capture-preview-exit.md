# Evidence: the capture preview program closed

- **Date:** 2026-10-10
- **Scope:** PRV-1-E — suite, the exit of the
  [capture preview plan](../plans/active/2026-10-10-capture-preview.md)
- **Environment:** CachyOS, niri 26.04 (the author's running build), main at
  `6a05c7f6` (FLU-P1-B landed)
- **Artifact:** none; documentation only, no build output changes

## What the program delivered

| Unit | Landed | Delivered |
|---|---|---|
| PRV-1-A | `6358440d` | The interfaces between the two applications in [ADR 0012](../decisions/0012-suite-conventions.md#follow-ups): `org.celestina.Fluorita1.Edit(s key)` and `Adopt(s key)` on `org.celestina.Selenita1`; the activation scanner's allowlist learns both literals ([evidence](2026-10-10-capture-preview-interfaces.md)). |
| PRV-1-S | `16dd76e5` | Support: `celestina-rs/crates/selenita-core` among Fluorita's production inputs, because Fluorita appends an edited copy to Selenita's history when Selenita is not running ([evidence](2026-10-10-capture-preview-support.md)). |
| SEL-2-A | `4c03e5ce` | Selenita 1.1.0: the corner preview «Vista previa» after every capture saved to a file and every finished recording (the picture, or the recording's first frame and length), five seconds held by the pointer, a `text/uri-list` drag offered as a copy only, a click that hands the file to Fluorita, `Selenita1.Adopt`; a key-binding launch shows no main window and ends after its preview ([evidence](../../selenita/docs/evidence/2026-10-10-preview.md)). |
| FLU-P1-A | `cf2668ac` | Fluorita 1.6.0: the floating editor on any path («Editar — nombre», one window per file, the library left as it is), `Fluorita1.Edit` and `fluorita --edit`, the save outcomes «Guardar ambas» and «Guardar solo la editada», a copy adopted into Selenita's history, the result dragged out ([evidence](../../fluorita/docs/evidence/2026-10-10-floating-editor.md)). |
| PRV-1-T | `9efca5e8` | Support: ADR 0009's amendment: a video's duration trim is the one editing operation that re-encodes, by an `ffmpeg` child never linked; the result is MP4 with the main video and one audio track; a VA-API failure is retried once with x264 ([evidence](2026-10-10-capture-preview-adr-0009.md)). |
| FLU-P1-B | `6a05c7f6` | Fluorita 1.7.0: the frame-accurate trim, two handles stepping one frame of the film's own rate, play kept to the span, progress and cancel; the fixture's [1 s, 2 s) measures 1.000000 s with both encoders ([evidence](../../fluorita/docs/evidence/2026-10-10-video-trim.md)). |

## Change

- **The niri window rules** (the author's configuration, outside the
  repository). The author approved the two rules of the spec's §7 on
  2026-10-10 and the coordinating session inserted them into
  `~/.config/niri/config.kdl`, after a backup of the file; this unit did not
  touch the configuration. The diff against the configuration before, with
  its three comment lines (one or two above each rule, naming it in the
  configuration's own language) left out here, the language contract
  admitting no Spanish prose in a record:

  ```diff
  393a394,408
  > window-rule {
  >     match app-id=r#"^org\.celestina\.Selenita$"# title="^Vista previa$"
  >     open-floating true
  >     open-focused false
  >     default-floating-position x=24 y=24 relative-to="bottom-right"
  > }
  >
  > window-rule {
  >     match app-id=r#"^org\.celestina\.Fluorita$"# title="^Editar — "
  >     open-floating true
  > }
  >
  ```

  Validation, as the coordinating session recorded it: the author's running
  niri build reports "config is valid" with the rules in place; the
  distribution's `niri validate` (niri 26.04-1.1 from pacman) reports four
  "unexpected node" errors, which the session attributed to nodes only the
  running build knows. This unit read the configuration and
  found both rules (a `grep` of the two titles: lines 397 and 405).
- `HOST-HYGIENE.md`: the parked SEL-1-F addendum to the 2026-10-09 capture
  role (Selenita owns the ScreenCast backend's build through
  `selenita/scripts/build-portal.sh`; the halted shell's September recipe is
  superseded and left as it is; `xdg-desktop-portal-gnome` stays out by the
  author's choice), and a 2026-10-10 host change record with two findings.
  The preview role: Selenita's corner preview, placed by the two window
  rules; `Decision: accepted by the author on 2026-10-10`. The edit role:
  Fluorita's floating editor and its trim through `/usr/bin/ffmpeg`, owned
  by the `ffmpeg` package that `mpv` (libmpv) already depends on
  (`pacman -Qo /usr/bin/ffmpeg`: ffmpeg 2:9.0.2-2.1; `pacman -Qi mpv` lists
  `ffmpeg` among its dependencies); no new dependency; `Decision: nothing to
  install`.
- Root `README.md`: "How the applications work together" names the preview
  and edit flow (the corner preview's drag, the click into Fluorita's
  floating editor through `Fluorita1.Edit` or `--edit`, the two save
  outcomes, `Selenita1.Adopt`) and points at the two applications' READMEs
  for the window rules, which SEL-2-A and FLU-P1-A already document.
- Root `ROADMAP.md`: the PRV-1 checklist is ticked for `PRV-1-A`,
  `SEL-2-A`, `FLU-P1-A` and `FLU-P1-B`, and names the support units
  `PRV-1-S` and `PRV-1-T`; `PRV-1-E`'s box is left for the author's archive
  commit, as `EXT-1-C` left its own. The roadmap stays `active` on `PRV-1`.
- The suite ledger: row `PRV-1-E` is `active` with this record; the landing
  seals its inventory and diffstat.

## Procedure

```sh
git apply --3way host-hygiene-sel1f.patch   # the parked SEL-1-F addendum; applied cleanly
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
git diff --name-only | python3 scripts/commit_scope.py --check \
  "suite-maintenance: Close the capture preview program"
```

## Result

- **Exit:** every command above exits 0.
- **Observed:** no build or test ran; this unit changes documentation only.

## Author's pending live checks

None of the program was seen on the real session by an agent; the three
entries below are the author's:

- `VAL-SEL-PREVIEW` in [Selenita's VALIDATION.md](../../selenita/VALIDATION.md):
  the preview in the corner without taking the focus, the timer and the
  hover, a capture and a recording dragged into WhatsApp and Slack, the
  click into Fluorita.
- `VAL-FLU-EDIT-WINDOW` in [Fluorita's VALIDATION.md](../../fluorita/VALIDATION.md):
  the floating editor on a real session, both save outcomes, the copy in
  Selenita's history, the drag out.
- `VAL-FLU-TRIM` in the same file: trimming a real recording, the frame
  under a handle against the first frame cut.

## Deferred minors

Recorded in the applications' `STATUS.md` known issues unless said
otherwise; none blocks the program's outcome.

Selenita (`selenita/STATUS.md`):

- **A late `Edit` reply.** The hand-off waits `HAND_OFF_TIMEOUT` (3 s) for
  Fluorita; a Fluorita that opens the file but answers later makes the
  preview say «No se ha podido abrir Fluorita.» while the editor is on
  screen.
- **Posters from a previous process.** A recording's first frame stays in
  `runtime_dir/selenita` until the next preview replaces it or the session
  ends; one left by an earlier process is not removed by the next.
- **`Adopt` racing a key-binding exit** (deferred at SEL-2-A's review; not
  in the STATUS). A key-binding launch ends once its preview has gone, and
  a click's hand-off is the usual way it goes; a copy Fluorita saves
  afterwards may find that process on its way out rather than a running
  Selenita or none, so its row can be missed.
- **The `--record` launch's lifetime.** A `--record` launch shows nothing
  while it records and ends only through its final report (the recording's
  preview, or a failure that shows the window); `selenita --stop` or
  Shift+Print is the way to end it.

Fluorita (`fluorita/STATUS.md`):

- **A concurrent `--edit` shows the winner's library.** An `--edit` that
  loses the name to a concurrent launch hands on an `Activate`, which raises
  that launch's library window before the edit reaches it; avoiding it needs
  a claim without a hand-off in `celestina_core::activation`.
- **A history append racing a starting Selenita.** With no Selenita on the
  bus Fluorita appends the copy itself; a Selenita starting at that instant
  can write its list back without the row. One writer for the history file
  across both processes would fix it.
- **The trim's crash leftover.** A Fluorita that crashes or is killed during
  a trim leaves `ffmpeg` running and its hidden file beside the original;
  tying the child to the parent's death needs `unsafe` Fluorita has no
  exception for.
- **The one-frame difference between the preview and the cut.** A handle
  between two frames can show one frame and cut from the next;
  `VAL-FLU-TRIM` checks it on real recordings.

Across both:

- **The adoption folder rule has two copies.** Which files join Selenita's
  history (a `.png` directly in the pictures folder's «Capturas», a `.mp4`
  directly in the videos folder's `Recordings`) is
  `selenita_core::history::adopted`, and again `kind_of` in
  `fluorita/src/adopt.rs`, which also names the «Capturas» folder, Selenita's
  product copy. The follow-up is to give the rule one owner in
  `selenita-core` (the folder names with it) and have Fluorita call it.

## Limits

- The rules' effect (the corner, the focus, the floating editor) is not
  observed here: that is `VAL-SEL-PREVIEW` and `VAL-FLU-EDIT-WINDOW`.
- The validation of the configuration is the coordinating session's record,
  not re-run by this unit.
- Two application STATUS lines predate this landing and are left to each
  application's next unit: Selenita's known issue that the preview opens
  tiled and focused until PRV-1-E adds the rule, and Fluorita's
  "`FLU-P1-B` … awaits landing".
- The plan is not archived by this unit; that is the author's hand commit
  after the landing, as for EXT-1.

## Follow-up

- The author's hand commit archiving
  `docs/plans/active/2026-10-10-capture-preview.md`, Selenita's
  `selenita/docs/plans/active/2026-10-10-sel-2-preview.md` and Fluorita's
  `fluorita/docs/plans/active/2026-10-10-flu-p1-preview.md` once `PRV-1-E`
  lands, ticking `PRV-1-E` and setting the suite roadmap idle.
- One owner for the adoption folder rule in `selenita-core`.

## Landing

- **Base revision:** `6a05c7f632478531d00649b02d1d19c1a5bd4bcc`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
