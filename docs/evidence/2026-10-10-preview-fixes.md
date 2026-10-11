# Evidence: the capture preview fixes, suite documents (PRV-2-A)

- **Date:** 2026-10-10
- **Scope:** PRV-2-A of the
  [preview fixes plan](../plans/active/2026-10-10-preview-fixes.md): the
  spec, ADR 0012's PRV-1 follow-up, `HOST-HYGIENE.md`, the roadmap, status
  and active-plans index
- **Environment:** main at 9fc57abf (PRV-1 closed); niri fork 26.04
- **Artifact:** none; documents only

## Finding

The author's screenshot showed Selenita's preview as a tiled column with
focus. The rule `match app-id=r#"^org\.celestina\.Selenita$"# title="^Vista
previa$"` never matched: both applications set an application display name
(`selenita/src/main.rs:80`, `fluorita/src/main.rs:67`), and Qt's Wayland
plugin formats a window title as `<title> — <display name>`, so the preview
is titled «Vista previa — Selenita». Fluorita's rule matches a prefix
(`^Editar — `) and was not affected.

## Change

- The author's configuration: the coordinator changed the title pattern to
  `^Vista previa` on 2026-10-10 at the author's request (backup in the
  session scratchpad); the running niri reports `config is valid`.
- The spec's §7 rule and a note on the title, the host-hygiene addendum,
  ADR 0012's folder name (`Grabaciones`, named `Recordings` until this
  program), and the spec's §1 folder name.
- The checkpoint PRV-2 opened in the roadmap, status and active-plans index.

## Procedure

```sh
grep -n 'set_application_display_name' selenita/src/main.rs fluorita/src/main.rs
~/.local/lib/celestina/niri validate -c ~/.config/niri/config.kdl
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

The display names are set in both applications; the corrected configuration
validates; both contracts pass.

## Limits

The preview's placement is the author's live check (VAL-SEL-PREVIEW).

## Landing

- **Base revision:** `9fc57abfccb8b0717244c2fc34bf4edbdb0dc345`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
