# Evidence: ACT-1 source-first library navigation contract

- **Date:** 2026-08-04
- **Scope:** ACT-1-A; plan
  [source-first-library-navigation](../plans/archive/2026-08-04-source-first-library-navigation.md)
- **Environment:** documentation only; no build, no artifact, no deployment
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py \
  docs/inventories/2026-08-04-source-first-library-navigation/ACT-1-A.numstat.tsv
```

## Result

- **Exit:** 0 for each command.
- **Observed:** `Architecture contract: OK`, `Language contract: OK`,
  `Documentation contract: OK`, `version-contract: OK (6 owners)`. No registered
  product version moved and no version-history row was appended, which is what
  a `maintenance` delivery must leave untouched.

The activation contract's behavioural-invariant list keeps every other bullet
byte-identical. Only the one naming Gallery and Music as fixed standalone
surfaces changed, and it now names them as catalogue projections selected by a
source's kinds while restating that the embedded Siderita surface stays a
minimal viewer or player. The `Space` versus double-click mapping, the
directory and unsupported-file rows and the browsing-starts-no-decoder rule were
reaffirmed rather than edited.

## Limits

- This unit changes no code and proves no behaviour. It removes the
  contradiction between the author's specified product and the accepted
  contract; the surface that depends on it is Fluorita F5, whose own evidence is
  [here](../../fluorita/docs/evidence/2026-08-04-source-first-library.md).
- The guards check consistency, links, lifecycle and language. They cannot check
  that the ruling is the right product decision; that judgement is the author's
  and is recorded in
  [ADR 0006](../decisions/0006-source-first-library-navigation.md).

## Follow-up

Fluorita F5 and its `VAL-FLU-SOURCES` author validation.
