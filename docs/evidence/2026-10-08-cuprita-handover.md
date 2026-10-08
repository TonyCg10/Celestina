# Evidence: Cuprita handover record

- **Date:** 2026-10-08
- **Scope:** AUD-1-R — suite
- **Environment:** CachyOS; Cuprita 1.0 landed through CUP-1-F
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
```

## Result

- **Exit:** the three contracts exit 0.
- **Change:** `HOST-HYGIENE.md` "Transitional shell authorities" records the
  2026-10-08 decision — Noctalia stays pending (shell halted); Blueman,
  `nm-applet` and Pavucontrol are superseded by Cuprita 1.0 — and the GNOME
  closure paragraph notes that Pavucontrol remains until the author retires
  it. The roles table is `cuprita/docs/evidence/2026-10-08-exit.md`.

## Limits

- Cuprita's live checks VAL-C, VAL-D and VAL-E (join a Wi-Fi with a
  password, pair the author's headphones, switch output to HDMI and change a
  profile) are the author's and were pending when this record was written;
  the packages are not removed by this unit.

## Landing

- **Base revision:** `2e885fe223c446a4bff2049cf5d8f015d58cd9ad`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
