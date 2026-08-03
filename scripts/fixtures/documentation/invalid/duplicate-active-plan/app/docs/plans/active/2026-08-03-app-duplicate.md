# Duplicate app delivery plan

- **Opened:** 2026-08-03
- **Plan ID:** app-duplicate
- **Status:** active
- **Scope:** app
- **Implementation checkpoint:** APP-1
- **Author-validation checkpoint:** None

## Hypothesis

One roadmap checkpoint cannot have two active execution plans.

## Tangible outcome

The one-to-one relation is enforced.

## Scope

- App fixture context.

## Exclusions

- Product code.

## Build order

1. Detect the duplicate plan.

## Implementation exit

The documentation guard rejects the duplicate.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| APP-1C | `app:` | planned | `app/` | Exercise one-to-one plan validation | pending | documentation fixture | None |
