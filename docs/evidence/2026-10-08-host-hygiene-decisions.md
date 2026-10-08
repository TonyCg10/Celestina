# Evidence: host-hygiene decisions

- **Date:** 2026-10-08
- **Scope:** AUD-1-O — suite
- **Environment:** CachyOS, Python 3.14
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
```

## Result

- **Exit:** every command above exits 0 (`Documentation contract: OK`,
  `Language contract: OK`, `Architecture contract: OK`); the unit changes
  documentation only.
- **Observed:** `HOST-HYGIENE.md` records the GNOME portal closure, the
  file-manager activation and the image MIME handler as resolved, adds the
  text MIME handler finding and the dead `application/zip` association, and
  appends the 2026-10-08 host change record.

## Limits

- The host facts (package removals, portal routing, `mimeapps.list` pins, the
  activation file and its live cold-activation test) were observed by the
  author's session on 2026-10-08, not re-measured by this unit.
- The Siderita evidence is
  `siderita/docs/evidence/2026-10-08-filemanager1-activation.md`.

## Follow-up

None.

## Landing

- **Base revision:** `bbc3dc6f7e450a99ec9c0e6b8312661887c832d7`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
