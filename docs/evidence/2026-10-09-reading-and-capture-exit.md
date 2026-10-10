# Evidence: Calcita in Siderita's «Abrir en» and the reading and capture program closed

- **Date:** 2026-10-09
- **Scope:** EXT-1-C — suite
- **Environment:** CachyOS, rustc 1.98.1, Qt 6.12.0, Python 3.14; debug and
  release builds in the shared session Cargo target
- **Artifact:** not applicable (Siderita's release binary built, not
  installed; the landing runs the production entries)

## Change

- Siderita (`siderita/src/apps.rs`, `siderita/src/suite.rs`): `ItemKind::Pdf`
  — a file named `*.pdf`, in any case, the same rule Calcita's own
  `admit` applies — and `Target::Calcita` (`key` `calcita`, activation name
  `CALCITA`, spawned as `calcita` when nobody owns the name). A selection of
  PDFs gets Grafita, Calcita and the phone; a PDF beside another kind of
  file loses Calcita, as media loses Fluorita. The kind is still decided
  from the folder model and by name on the Qt thread, never by reading the
  file; the generic «Abrir con…» below the section keeps reaching Calcita
  through `xdg-mime` and the `application/pdf` its desktop entry declares.
  This is a deliberate deviation from the plan's wording: the PDF is decided
  by name, not by MIME, because asking `xdg-mime` would put process IO on
  the Qt thread the menu runs on.
  Calcita is offered only when its desktop entry is installed, like the
  others (`installed_in`). `EntryContextMenu.qml` labels the target
  «Calcita»; its icon name is `org.celestina.Calcita`, as the existing rule
  derives it.
- `HOST-HYGIENE.md`: a 2026-10-09 host change record with two findings. The
  PDF role: Calcita is a candidate handler and the «Abrir en» target; the pin
  of `application/pdf` to `org.celestina.Calcita.desktop` is the author's
  hand edit (a step of `VAL-CAL-DARK`), after which the browser no longer
  holds the role; `Decision: pending` until the author records it. The
  capture role: Selenita holds it; `grim` and `slurp` stay as its tools;
  niri's own screenshot UI remains available; recording needs
  `gst-plugins-good` (`mp4mux`), absent on the host, which the author
  installs; `Decision: pending` for the same reason.
- [ADR 0012](../decisions/0012-suite-conventions.md) gains a `Follow-ups`
  section: the two activation names (`org.celestina.Calcita`,
  `org.celestina.Selenita`), Selenita's own `org.celestina.Selenita1` with
  `Capture`, `ToggleRecording` and `StopRecording` (an additive method on an
  application's own interface is compatible), and why there is no ADR 0013:
  the recording spike's child-process choice is a measured rule recorded in
  Selenita's evidence, not a new convention.
- `scripts/activation_contract.py`: the allowlist reason for
  (`selenita/src/activation.rs`, `org.celestina.Selenita1`) names
  `StopRecording` too; text only, the scanner's behaviour is unchanged.
- Root `README.md`: "How the applications work together" names Calcita and
  Selenita among the one-window applications, Calcita among the «Abrir en»
  and drop targets, and Selenita's extra interface; the project table already
  had both rows.
- Root `ROADMAP.md`: the EXT-1 checklist is ticked for `EXT-1-A`, `CAL-1-A`
  to `CAL-1-C`, `EXT-1-B` and `SEL-1-A` to `SEL-1-C`; `EXT-1-C`'s box is left
  for the author's archive commit, as `CONV-1-F` left `CONV-1-F`'s. The
  roadmap stays `active` on `EXT-1`.
- The suite ledger: row `EXT-1-C` is `active` with this record; the landing
  seals its inventory and diffstat.
- Siderita's `README.md` and `STATUS.md` name Calcita among the targets.

## Procedure

```sh
# in siderita/:
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
cargo build --release --locked
# from the root:
sh siderita/scripts/qml-tests.sh
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/activation_contract.py
python3 scripts/test-activation-contract.py
git diff --name-only | python3 scripts/commit_scope.py --check \
  "suite-maintenance: Add Calcita to the open-in targets and close the reading and capture program"
```

## Result

- **Exit:** every command above exits 0.
- **Tests added:** Siderita `apps.rs` 1 unit
  (`a_pdf_goes_to_grafita_calcita_and_the_phone`) and three assertions in
  the mixed-selection, QML-key and installed-entry tests; `suite.rs`
  `media_and_pdfs_are_decided_by_name` (replaces `media_is_decided_by_name`:
  `.pdf`, `.PDF`, a file named `pdf` without extension); QML
  `tst_entry_menu_suite.qml` 1 test
  (`test_a_pdf_lists_calcita_between_grafita_and_the_send_entry`: label,
  icon name, order, and `open("calcita", keys)` on trigger).
- **Observed:** Siderita unit 195 passed, 1 ignored; QML 213 passed, 0
  failed; qmllint Siderita 222 (its baseline row, unchanged); the activation
  scanner's own tests 13 passed; the release binary built in the shared
  target.

## Limits

- Nothing here was run on the real session: opening a real PDF from
  Siderita into Calcita is `VAL-CAL-OPEN`, and the handler pin is a step of
  `VAL-CAL-DARK`, both the author's and pending in `calcita/VALIDATION.md`.
- The target is decided by name, like media: a PDF without the `.pdf`
  extension is offered to Grafita and the phone only, and still reaches
  Calcita through «Abrir con…», which asks `xdg-mime`.
- `SEL-1-C`'s box in the roadmap is ticked on the program's build order:
  this unit lands after Selenita's 1.0 unit. Its own Selenita changes are
  not part of this unit.
- The plan is not archived by this unit; that is the author's hand commit
  after the landing, as `CONV-1-G` was for `CONV-1`.
- The two host-hygiene findings stay `pending`: the pin, the package and the
  key bindings are the author's actions.

## Follow-up

The author's hand commit archiving
`docs/plans/active/2026-10-09-reading-and-capture.md` once `EXT-1-C` lands,
setting the suite roadmap idle and moving EXT-1 to completed work in the
suite status.

## Landing

- **Base revision:** `392a5ea7b7fd3f12c360629ab16e02358a50382c`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:54d973ff0eccb4c233af2d9df856fd4586db8ea02bd491eb633e2b38ea55e0cf, verification_fingerprint sha256:8ba69713fe6c03d8752a130f181abb479727ba67aeda498eb7b24f90d5996006
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
