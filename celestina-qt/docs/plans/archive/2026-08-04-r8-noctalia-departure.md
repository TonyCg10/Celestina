# R8 — reversible Noctalia departure

- **Opened:** 2026-08-04
- **Plan ID:** r8-noctalia-departure
- **Closed:** 2026-08-04
- **Successor:** none; the Polkit and dock slices wait on SHELL-D3 and SHELL-D4 rather than on a plan
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** R8
- **Author-validation checkpoint:** `VAL-R8` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The handover can be described precisely enough that a tool refuses to remove
Noctalia while any responsibility is still uncovered or unvalidated — so the
decision to depend on this shell alone is made once, deliberately, on evidence
rather than on the feeling that enough has been built.

## Tangible outcome

`celestina/scripts/handover-status.sh` reports, read-only, which
responsibilities this shell has taken over, which Noctalia still supplies, and
what each still needs. The removal path exists, writes a rollback first, and
refuses while the report is not complete — so today, with every author
validation still deferred, it refuses.

## Scope

In scope: the pure handover model — the responsibilities, what covers each and
what "covered" requires; the read-only status report; a removal path that
writes a rollback before touching anything and refuses on an incomplete report;
and the documented way back.

## Exclusions

Out of scope: Polkit, which at plan time waited on SHELL-D3 and has since been
authorized as its own R8 slice
([ADR 0005](../../decisions/0005-first-party-polkit-agent.md)); the dock,
which at plan time waited on SHELL-D4 and has since been decided against
([ADR 0003](../../decisions/0003-no-running-app-dock.md)); uninstalling
packages or touching a package manager; editing the author's Niri
configuration; and running the removal at all — this plan builds the tool and
never uses it.

## Build order

1. Add the pure handover model to `celestina-shell-core`: the
   responsibilities, what covers each, and when removal may be offered.
2. Add the read-only status report over that model.
3. Add the removal path: rollback first, refusal on an incomplete report,
   explicit confirmation required.
4. Document the handover and the way back.

## Implementation exit

- The model names every responsibility Noctalia supplies today, and a test
  fails if one is added without saying what covers it.
- Removal is refused while any responsibility is uncovered or its author
  validation is unrecorded, proved by a test rather than by inspection.
- The status report runs read-only and changes nothing, proved by running it in
  verification.
- The removal path is never invoked by any build, verification or completion
  script.
- Rust format, Clippy and package tests pass.
- The architecture and documentation contracts pass.
- `scripts/complete-production.sh` builds once, verifies those exact bytes and
  updates the on-disk bundle; the live session is never replaced.

R8 implementation closes on this evidence. Actually removing Noctalia is
`VAL-R8`, and it is the author's decision on their own session.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R8-A | `celestina:` | done | [inventory](../../inventories/2026-08-04-r8-noctalia-departure/R8-A.numstat.tsv) | 19 files, +649/-11 | The handover model, the read-only report, and a removal that writes its rollback first and refuses on an incomplete report | [R8 Noctalia departure](../../evidence/2026-08-04-r8-noctalia-departure.md) | `VAL-R8` |

Both build-order steps closed as one unit, as every checkpoint since R3 has:
each `done` unit needs one exclusive inventory *and* one exclusive evidence
record, and one verification run does not honestly produce two.

## Decisions and rollback

Removal is **refused by default**, and the refusal is the feature. Every author
validation from R3 onwards is still deferred, so the report is incomplete and
the tool says so: the shell will not help remove the thing it has not yet been
proven to replace.

Nothing here uninstalls a package. The reversible step is disabling Noctalia's
autostart and stopping it for this session; the package stays exactly where it
is, so the way back is turning the autostart on again. A tool that removed
software would be making a decision about the machine rather than about the
session.

The rollback file is written *before* anything changes, and names what was
disabled and how to restore it. A removal that could not write it does not
proceed.
