# H5-D — The polkit interaction flag, the action timeouts and the outcome mapping

- **Opened:** 2026-09-22
- **Plan ID:** h5-privilege-fixes
- **Status:** done
- **Closed:** 2026-09-22
- **Successor:** none
- **Authorization:** the whole-branch review of `H5` came back "With fixes" on
  2026-09-22 and the author's coordinator assigned the wave as its own unit
- **Scope:** hematita
- **Implementation checkpoint:** H5-D
- **Author-validation checkpoint:** `VAL-H5` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A system-unit action reaches polkit with interaction allowed, waits for the
person, and reports what polkit answered.

## Tangible outcome

The installed 0.6.1 asks polkit for a system unit with
`ALLOW_INTERACTIVE_AUTHORIZATION` set, so an agent is actually consulted
instead of the call being refused before anyone is asked; it waits five
minutes rather than twenty-five seconds for the person's answer, and says
`denied` when polkit says the person was not authorised instead of blaming a
missing agent. The listing cannot freeze the sampler, and the documents say
what the code does.

## Scope

- `H5-D` — the eleven findings of the `H5` whole-branch review that the
  controller did not defer, the ADR amendment they rest on, and 0.6.1.

## Exclusions

Deferred by the controller and not attempted here: `revision` is bumped on
every service tick whether or not any unit changed.

## Build order

1. `H5-D` alone, after `H5-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 0.6.1.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H5-D | `hematita:` | done | [inventory](../../inventories/2026-09-22-h5-privilege-fixes/H5-D.numstat.tsv) | 18 files, +463/-74 | `AllowInteractiveAuth` on every unit action; a five-minute action timeout and a two-second listing timeout, both per connection; `NotAuthorized` mapped to `denied`; the listing's stuck bus bounded; the hub as the authority on which unit exists and which manager it belongs to; `actionScope` published; the outcome preferred over an available bus's silence; the process page's own `no-agent` sentence; the foreign kill question naming the prompt window; `Refusal::Foreign` a unit variant; `AGENTS.md` and `VAL-H5` corrected; the deviation from the plan's `terminateForeign`/`killForeign` recorded; 0.6.1 | [privilege fixes 2](../../evidence/2026-09-22-h5-privilege-fixes-2.md) | `VAL-H5` |
