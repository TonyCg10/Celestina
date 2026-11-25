# VIS-1 — Preview visible changes before building them

- **Opened:** 2026-11-25
- **Plan ID:** visible-change-preview
- **Closed:** 2026-11-25
- **Successor:** none; the author named no next suite checkpoint. Its one row (VIS-1-A) is done
- **Status:** done
- **Authorization:** on 2026-11-25 the author asked that any visible change
  be shown first as previews as faithful as possible to what can be built, and then to add that rule to the repository. Opened and closed in one
  commit; TRK-1, the active suite plan, excludes anything but the tracks'
  documents
- **Scope:** suite
- **Implementation checkpoint:** VIS-1
- **Author-validation checkpoint:** none

## Hypothesis

Describing how a screen will look and seeing it are different things, and
iterating on a built screen costs a build, a landing and a smoke per round.
A rule in the root contract, which every session reads first, makes a
faithful preview and the author's approval the first step of any visible
change.

## Tangible outcome

The section "Visible changes: preview before building" in the root
`AGENTS.md`, and the light-unit rule of `docs/standards/verification.md` that
says where a light unit links its preview.

## Scope

- `VIS-1-A` — that section, the light-unit line, and the suite's roadmap and status lines.

## Exclusions

- Any guard: no script checks that a preview was shown.
- `celestina-style/DESIGN.md`, which keeps the values the previews use.

## Build order

1. `VIS-1-A`.

## Implementation exit

The checkpoint closes when `VIS-1-A` is `done`: the documentation guard and the
staged-units check pass on the sealed commit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| VIS-1-A | `suite:` | done | [inventory](../../inventories/2026-11-25-visible-change-preview/VIS-1-A.numstat.tsv) | 7 files, +147/-1 | Require a faithful preview, approved by the author, before planning or building a visible change | [evidence](../../evidence/2026-11-25-visible-change-preview.md) | None |

This plan records intent; it grants no authority.
