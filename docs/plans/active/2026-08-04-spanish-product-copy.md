# Spanish product copy and its guard migration

- **Opened:** 2026-08-04
- **Plan ID:** spanish-product-copy
- **Status:** active
- **Scope:** suite
- **Implementation checkpoint:** LNG-1
- **Author-validation checkpoint:** none

## Hypothesis

The language contract can hold development truth in English and product copy in
Spanish without weakening anything it currently enforces, because the boundary
between them is mechanical: `qsTr()` in QML, and the string literals of a file
that declares itself product copy.

## Tangible outcome

A Spanish desktop stops being contradicted by its own rules. The guard accepts
Spanish where a person reads it, keeps rejecting it everywhere else, and its
ratchet can record the reduction that acceptance causes.

## Scope

- ADR 0007 and the language standard, root contract and workflow documents.
- The two exemptions in `scripts/check-language-contract.py`, with fixtures.
- The declared-migration escape in `scripts/commit_scope.py`, with fixtures.
- The ratchet movement that escape exists to allow.

## Exclusions

- Translating any surface. This unit changes the rule; the copy each product
  ships is its owner's work under its own prefix.
- Qt translation catalogues. ADR 0007 records why one locale does not need them
  and why `qsTr()` keeps that door open.
- The architecture ratchet and its own resolution field, which are untouched.

## Build order

1. Record the decision and amend the standard, the root contract and the
   workflow documents that repeat it.
2. Teach the scanner the two exemptions, with positive and negative fixtures
   for each.
3. Add the declared-migration escape to the commit guard, with fixtures proving
   that neither the scanner change nor the evidence alone moves a row.
4. Record the reduction the migration earned.

## Implementation exit

```sh
python3 scripts/check-language-contract.py
python3 scripts/test-language-contract.py
bash scripts/test-commit-scope.sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

No product version moves: this unit ships no product behaviour.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LNG-1-A | `suite:` | done | [inventory](../../inventories/2026-08-04-spanish-product-copy/LNG-1-A.numstat.tsv) | 17 files, +568/-44 | Split development truth from product copy in the language contract, teach both guards the boundary, and record the reduction the migration earned | [evidence](../../evidence/2026-08-04-spanish-product-copy.md) | None |

This unit closes as `suite-maintenance`. It is genuinely cross-suite: the rule
it changes governs every project, and the ratchet rows it lowers belong to
three of them.
