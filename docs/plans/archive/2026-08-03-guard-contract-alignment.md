# GOV-2 — Align the guards with the contract they enforce

- **Opened:** 2026-08-03
- **Closed:** 2026-08-03
- **Status:** done
- **Plan ID:** guard-contract-alignment
- **Scope:** suite
- **Implementation checkpoint:** GOV-2
- **Author-validation checkpoint:** none
- **Successor:** none; project roadmaps own subsequent implementation

## Hypothesis

The gaps found by auditing GOV-1 are one cross-cutting contract problem, not
independently deliverable units: guard language, ratchet ownership,
deterministic context, artifact verification, commit intent, version history
and the documents describing them must agree in the same revision.

## Tangible outcome

A green run of every registered guard and fixture suite emits English; no
modified unratcheted guard path hides Spanish diagnostics; a project commit can
lower the matching architecture or language ratchet without gaining authority
over unrelated rows, trusting an unstaged worktree or letting its staged
registry authorize a broader scope; committed rules interpret staged data
without executing Python from INDEX or the worktree, and neither registry
revision can hide a delivery layout; qualitative architecture resolution
remains possible through durable evidence; `agent-context.py` prints
global, lexical-owner and resolved-owner required context and fails closed when
that registry is incomplete; artifact status tracks completion only for
deployable projects and fails closed if an orchestrator disappears; build and
verification seals are written only by a runner that executes the registered
entrypoint successfully over an unchanged interval, instead of trusting
caller-supplied labels; and the documents describe the checkout that enforces
those claims. Product commits use one English change kind that maps to an exact
SemVer increment, while durable version history remains mechanically tied to
the version declarations changed by that commit.

## Scope

- Translate the diagnostics of the guards and production entry points that emit
  Spanish while carrying no language-baseline row, so the guard stops reporting
  a clean result over text it cannot see.
- Register `commit_policy.shared_ratchet_files` and place it inside every
  project and component commit scope, including both architecture and language
  debt. Apply only Python rules committed in HEAD to both HEAD and INDEX registry
  TOML and to INDEX source/baseline bytes, intersect both authorities, and never
  execute staged or unstaged interpretation modules. Discover delivery roots
  from the conservative HEAD/INDEX union and reject ownership conflicts. Accept
  resolution evidence only from the primary owner's canonical directory before
  any `lines` row disappears. Merge commits cannot change ratchets and must keep
  every staged guarded source equal to the INDEX baseline. A future semantic
  scanner migration must first land dormant/backward-compatible code and only
  then activate it with its measurement change. Require a recognized English
  imperative plus conservative language screening for new normal, revert and
  fixup subjects while replaying inherited history scope-only. Treat hooks as
  integrity controls rather than an adversarial sandbox.
- Register mandatory suite rules and owner-local context documents, print them
  from `scripts/agent-context.py`, preserve both sides of cross-owner symlinks,
  and reject absent or escaping context configuration.
- Include all required lifecycle entries in artifact verification fingerprints:
  verify and status for every project, plus deploy and both completion layers
  for deployable projects and activation when declared. Prove that changing,
  deleting or unregistering them invalidates verification without allowing a
  reseal, while a nondeployable library remains independent of completion.
- Make the artifact runner capture production or verification state, execute
  exactly the registered entrypoint in a reserved internal mode, and seal only
  after a zero exit over an unchanged interval. Remove the public start/record
  operations, and make all seven build/verify scripts delegate sealing while
  their internal mode performs real work without recursion or self-sealing.
- Correct ADR 0001, the CI contract map, every stale project exit command, the
  shell plan's deployment wording, and the volatile claims in the root
  `STATUS.md`.
- Define and enforce typed product commit subjects for bug fixes, milestones,
  major releases and version-neutral maintenance; register authoritative
  version declarations and require the matching SemVer increment and history
  entry in the same staged unit.
- Cover each mechanism with positive and negative fixtures.

## Exclusions

- Hardening the language detector itself. A stricter heuristic adds 14 paths and
  raises 45 rows of `scripts/language-baseline.tsv`, which the ratchet forbids;
  re-basing the ratchet changes the unit of measurement and is the author's
  decision, not a guard fix.
- Translating unrelated files that already carry a language-baseline row. The
  touched guard sections still follow the normal editing rule and lower or
  remove their own debt.
- Requiring an inventory for project-prefixed commits that touch source roots,
  proportionality rules for `complete-production.sh` fan-out, and collapsing the
  ledger rules duplicated across five documents. Each changes how the author
  works and needs an accepted decision first.
- Suite-wide locking or transactional rollback for multi-file deployments, and
  status coverage for generated desktop/icon/portal/service outputs. The current
  helpers atomically replace one file or tree at a time and status covers only
  registered artifact mappings; those are explicit hardening fronts, not claims
  of this interval-sealing unit.
- Any release rebuild, deployment or activation. The runner
  changes registered build scripts and therefore makes existing manifests
  intentionally stale; this unit records that state instead of hiding a release
  rebuild inside governance verification.
- Retrospective product version changes. Existing declarations are recorded as
  the adoption baseline; the first later typed product delivery increments from
  that baseline.

## Build order

1. Translate the guard chain and the production entry points; realign the
   fixtures that assert on those strings; lower the language baseline.
2. Add both shared ratchets to commit scope, anchor interpretation in HEAD,
   make removed architecture rows and hidden inventory layouts fail closed, and
   prove legitimate, illegitimate and language-invalid commit cases.
3. Add mandatory suite and owner context entries, print them, update golden
   context files and add negative fixtures for omissions.
4. Fingerprint every lifecycle entry, place build/verification execution under
   the artifact runner, remove caller-driven sealing, and cover success, child
   failure and every changed-interval class with fixtures.
5. Register product version sources, add the typed subject and SemVer/history
   guard, and prove each accepted and rejected increment with fixtures.
6. Correct the disagreeing documents, run every registered contract fixture,
   record the earlier installed-state audit and leave the newly stale manifests
   for the next real release build.

## Implementation exit

- `bash scripts/check-architecture-contract.sh`,
  `bash scripts/check-documentation-contract.sh` and
  `python3 scripts/check-language-contract.py` pass and print English.
- `scripts/test-architecture-scanners.sh`, `scripts/test-documentation-contract.sh`,
  `scripts/test-commit-scope.sh`, `scripts/test-staged-units.sh`,
  `python3 scripts/test-version-contract.py`,
  `python3 scripts/version_tool.py check` and
  `python3 scripts/audit-version-commits.py`,
  `scripts/test-production-artifacts.sh` and
  `scripts/test-production-common.sh` pass.
- `LANGUAGE_COMPARE_REF=HEAD python3 scripts/check-language-contract.py` passes,
  proving the baseline only fell.
- Every no-argument build/verify entry delegates to the runner; legacy direct
  sealing commands are absent; and artifact fixtures prove that only a
  successful registered child over an unchanged interval creates a seal. No
  application binary is rebuilt or deployed. Because build scripts are
  production inputs, existing manifests remain explicitly stale until the next
  real release build.

GOV-2 carries no author-validation case: every claim is reproducible by command
and none of it depends on a real session, hardware or perception.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and stable
symbols are authoritative; line counts drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| GOV-2-A | `suite:` | done | [Exact inventory](../../inventories/2026-08-03-guard-contract-alignment/GOV-2-A.numstat.tsv) | `95 files, +7186/-1309` | One fail-closed governance contract: complete context, row-safe ratchets, truthful English output, typed product changes with exact SemVer history and runner-supervised artifact execution | [GOV-2 evidence](../../evidence/2026-08-03-guard-contract-alignment.md) | None |

## Closing note

The corrected implementation and evidence pass. Runner supervision proves that
the registered entrypoint exited successfully while its inputs stayed stable;
it does not certify the semantic completeness of commands inside that script.
The author requested publication, so the unit closes with one exact inventory
against the direct parent of its landing commit. That commit uses legacy
`suite:` because the current HEAD owns subject interpretation; typed subjects
are mandatory immediately after adoption.

## Decisions and rollback

Nothing here changes an accepted decision except ADR 0001, whose wording is
brought in line with the guard that has always been stricter than the sentence
describing it; the ruling itself is untouched. Every tracked change is a
document, registry entry, guard or fixture. Reverting restores the previous
behaviour. The runner edit invalidates ignored manifests at the production
layer, so their next update requires the normal release build; no data migration
is involved.
