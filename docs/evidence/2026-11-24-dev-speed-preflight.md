# Evidence: DEV-1-B — preflight in tiers

- **Date:** 2026-11-24
- **Scope:** `scripts/agent-context.py`, `scripts/preflight_budget.py`,
  `scripts/preflight-baseline.tsv`, `scripts/documentation_contract.py` and
  their fixture tests; the `[suite]` table of `docs/projects.toml`; the root
  `AGENTS.md` and the documents that receive its moved text; the `DEV-1-B` row
  of `docs/plans/active/2026-11-24-dev-speed.md`; this record.
- **Environment:** CachyOS, worktree `suite-DEV-1-B`, branch
  `unit/suite/DEV-1-B`, Python 3 standard library
- **Artifact:** not applicable

## Procedure

The unit gives every document of `scripts/agent-context.py` a tier (`now`, or
`when` with a trigger), moves the Qt halt history and the duplicated ledger,
hook and production mechanics out of the root `AGENTS.md`, and adds
`scripts/preflight_budget.py`, which the documentation contract calls to hold
each project's `now` tier under its row of `scripts/preflight-baseline.tsv`.

The "before" column was measured on `36548372`, before any edit, and the
"after" columns on the finished branch, with the same command per target
(`--json` lists the `now` tier, `--full` the flat list an agent read until
now):

```sh
python3 scripts/agent-context.py TARGET | xargs cat | wc -c          # before
python3 scripts/agent-context.py --json TARGET                       # after, now tier; its paths summed with wc -c
python3 scripts/agent-context.py --full TARGET | xargs cat | wc -c   # after, every document
wc -c AGENTS.md
python3 scripts/preflight_budget.py measure
```

Guards and tests, from the worktree root, on the final code:

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
python3 scripts/test-preflight-budget.py
bash scripts/test-commit-scope.sh
bash scripts/test-land-unit.sh
bash scripts/test-staged-units.sh
```

### Text moved out of the root AGENTS.md

| Moved text (first words) | Now lives in (file, heading) | Already there / moved verbatim |
|---|---|---|
| "**History: the Qt/QML shell.** The author halted…" (whole block) | `docs/history/qt-shell-halt-and-salvage.md`, body | moved verbatim |
| "**Salvage record.** Before the removal…" through the CEL-QT-RESCUE bullet | `docs/history/qt-shell-halt-and-salvage.md`, body | moved verbatim |
| "Run `python3 scripts/agent-context.py PATH` … read every document it prints, completely." | `AGENTS.md`, "Mandatory preflight" items 1–2 (reworded by the plan's Task 2 text) | already there |
| "Its output is the reading order, not a summary: root and local `AGENTS.md`…" | `docs/README.md`, "Deterministic agent context" (first paragraph) | already there |
| "Read the active plan ledger completely when one exists; it is the cross-session hand-off…" | `AGENTS.md`, "Mandatory preflight" item 3 (reworded by the plan's Task 2 text) | already there |
| "Every plan has a ledger with ID, prefix, status…" | `docs/governance/change-policy.md`, "Ledger fields" | already there |
| "Line numbers are secondary because they drift; the ledger is the cross-session hand-off and commit source." | `docs/governance/change-policy.md`, "Ledger fields"; `AGENTS.md`, "Mandatory preflight" item 3 and "Git and commits" item 4 | already there |
| "The roadmap-to-plan link is strict. An `active` or `blocked` `ROADMAP.md`…" | `docs/governance/change-policy.md`, "Plan identity and roadmap link"; `docs/governance/documentation.md`, "Plan lifecycle" | already there |
| "For `planned`, `idle`, or `done`, the field is `none`." | `docs/governance/change-policy.md`, "Plan identity and roadmap link" | already there |
| "Every plan declares a unique immutable `Plan ID`…" | `docs/governance/change-policy.md`, "Plan identity and roadmap link" | already there |
| "Archiving never changes it." | `docs/governance/change-policy.md`, "Archive transition"; `docs/governance/documentation.md`, "Plan lifecycle" | already there |
| "A local plan ledger accepts only its owner's primary prefix; `suite:` is for…" | `docs/governance/change-policy.md`, "Plan identity and roadmap link" and "Ledger fields" | already there |
| "Component prefixes are for atomic code or manifest commits…" | `docs/governance/change-policy.md`, "Plan identity and roadmap link" | moved verbatim |
| "A `done` unit records exact `N files, +X/-Y`…" | `docs/governance/change-policy.md`, "Ledger fields"; `docs/governance/documentation.md`, "Ledger closure" | already there |
| "Inventories live at `<owner>/docs/inventories/…`" | `docs/governance/change-policy.md`, "Exact immutable inventories" | already there |
| "They contain the base revision, every path…" | `docs/governance/change-policy.md`, "Exact immutable inventories" | already there |
| "Only `suite:` may use `Pathspec<TAB>.`." | `docs/governance/change-policy.md`, "Exact immutable inventories" | moved verbatim |
| "Before commit, the guard compares exhaustive paths, numstat, and SHA-256 inside those boundaries…" | `docs/governance/change-policy.md`, "Exact immutable inventories" | moved verbatim |
| "The base is the `HEAD` immediately before the unit…" | `docs/governance/change-policy.md`, "Exact immutable inventories" | already there |
| "`scripts/land-unit.py` computes the inventory, the ledger closure, the version transition and the production run…" | `docs/contracts/landing.md`, introduction | already there |
| "Binary rows use `-/-`; mode-only rows use `0/0`." | `docs/governance/change-policy.md`, "Exact immutable inventories" | already there |
| "The main project prefix covers its tree…" | `docs/governance/change-policy.md`, "Commit enforcement" | moved verbatim |
| "A component prefix covers only component code…" | `docs/governance/change-policy.md`, "Commit enforcement" | moved verbatim |
| "Both additionally cover `commit_policy.shared_ratchet_files`…" | `docs/governance/change-policy.md`, "Commit enforcement" | moved verbatim |
| "Python rules committed in HEAD interpret source, baselines and registry TOML from INDEX…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "Only a guard or `scripts/git_hooks.py` that HEAD lacks runs from the worktree…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | moved verbatim |
| "HEAD and INDEX must both authorize every normal commit path and prefix…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "Delivery discovery uses the conservative union of HEAD and INDEX layouts…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "Merge commits cannot change ratchets…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "A semantic rule change becomes authority after landing…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "Hooks are repository-integrity controls, not an adversarial sandbox." | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "A `lines` row may disappear only when the same staged unit changes or deletes that source…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | moved verbatim |
| "A language row may fall without its source only for a declared scanner migration…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "While a migration is staged, `pre-commit` measures the index against HEAD's language rows…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | moved verbatim |
| "This is qualitative architectural closure, not a line threshold." | `docs/governance/change-policy.md`, "Scope and unrelated work" ("line count alone never proves resolution") | already there |
| "Local evidence lives under the project's canonical `docs/evidence/`…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "A component prefix cannot create a nested lookalike evidence directory…" | `docs/governance/change-policy.md`, "Scope and unrelated work" | already there |
| "A tracked inventory is immutable…" / "Later work gets a new unit and inventory." | `docs/governance/change-policy.md`, "Exact immutable inventories" | already there |
| "Archiving moves only the plan from `plans/active/`…" / "Evidence and inventories stay at stable roots." / "If the move needs a separate traceable commit…" | `docs/governance/change-policy.md`, "Archive transition"; `docs/governance/documentation.md`, "Plan lifecycle" | already there |
| "All uncommitted `done` units in one plan form one atomic batch…" / "Do not commit the shared plan while leaving another inventory pending." | `docs/governance/change-policy.md`, "Multiple units and batches" | already there |
| "`.githooks/pre-commit` discovers inventories and requires their union to match…" | `docs/governance/change-policy.md`, "Commit enforcement" | already there |
| "`build-production.sh` builds the canonical release artifact once." | `docs/contracts/production-artifacts.md`, "Objective" (table) and "Single build" | already there |
| "`verify-production.sh` verifies those same bytes while reusing caches." | `docs/contracts/production-artifacts.md`, "Exact verification" | already there |
| "`deploy-production.sh` copies only a current verified artifact…" | `docs/contracts/production-artifacts.md`, "Objective" (table) and "Deploy without rebuilding" | already there |
| "`complete-production.sh` runs build, verify, deploy, and status." | `docs/contracts/production-artifacts.md`, "Objective" (table) | already there |
| "The shell additionally has `activate-production.sh`; completion updates the on-disk bundle…" | `docs/contracts/production-artifacts.md`, "Shell special case" | already there |
| "The landing runs these entries on the tree it commits, for the owner and for every other project…" | `docs/contracts/landing.md`, "Landing", step 7 `build_if_stale` and the deploy after the push | already there |
| "For a product `bug`, `milestone` or `release`, the landing bumps the registered SemVer source…" / "a session never bumps" | `docs/contracts/versioning.md`, "What the landing does" | already there |
| "`maintenance` changes do neither." | `docs/contracts/versioning.md`, "Subject and SemVer mapping" | already there |
| "Do not use a parallel Cargo/CMake build as final evidence…" | `docs/contracts/production-artifacts.md`, "Single build" | moved verbatim |
| "…between a build and its deploy, production targets and caches are reusable monorepo resources." | `docs/contracts/production-artifacts.md`, "Single build" | already there |
| "Once a landing has deployed and checked its artifacts, its last step removes the Cargo debug profiles…" | `docs/contracts/production-artifacts.md`, "Pruning after deploy" | already there |

## Result

- **Exit:** 0 for each of the eight commands; `test-land-unit.sh` ran 116
  tests, `test-preflight-budget.py` 13.
- **Observed:** bytes a session reads before acting, per target:

| Target | Before: documents / bytes | After, `now` tier: documents / bytes | After, `--full`: documents / bytes |
|---|---|---|---|
| `celestina/src` | 22 / 367672 | 4 / 27741 | 22 / 359355 |
| `siderita/src` | 21 / 280517 | 4 / 51266 | 21 / 272200 |
| `celestina-rs/crates/celestina-core` | 85 / 1016545 | 14 / 54491 | 85 / 1008228 |
| `.` | 17 / 182559 | 3 / 28670 | 17 / 174242 |

- **Root `AGENTS.md`:** 24715 bytes before, 14127 after; the limit is 16384.
- **Moved text:** the table above has 52 rows: 40 already there, 12 moved
  verbatim, none dropped.
- **Budget:** measured `now` tier against its row, after the trim:

| Row | Measured bytes | Limit |
|---|---|---|
| `calcita` | 27747 | 40960 |
| `celestina` | 27741 | 40960 |
| `celestina-rs` | 24061 | 40960 |
| `celestina-style` | 36211 | 40960 |
| `cuprita` | 27964 | 40960 |
| `fluorita` | 34072 | 40960 |
| `grafita` | 33903 | 40960 |
| `hematita` | 63475 | 65536 |
| `magnetita` | 31121 | 40960 |
| `magnetita-android` | 26077 | 40960 |
| `root-agents` | 14127 | 16384 |
| `selenita` | 44943 | 45056 |
| `siderita` | 51266 | 53248 |
| `suite` | 28670 | 40960 |

- **First run of the guards:** `test-commit-scope.sh` and `test-land-unit.sh`
  exited 1, because `documentation_contract.py` imported `preflight_budget` at
  load and those fixtures copy the contract module alone. The import moved
  into `check_preflight_budget`; the exits above are from after that fix.
- **Review:** one round of `/code-review` on the branch found five things.
  Fixed: a halted project's baseline row was reported as an unknown row; a
  byte cell with a non-ASCII digit lost its line number; a sentence stated
  twice in `docs/governance/change-policy.md`; the document map described the
  flat output as the default. The fifth is in Follow-up.

## Limits

- No agent session was timed with the new tiers.
- Nothing enforces that a baseline row only goes down: it is a written rule.
  Registering the file in `commit_policy.shared_ratchet_files` was outside
  this unit.
- The `selenita` row leaves 113 bytes of room and `hematita` and `siderita`
  about 2000: the next line added to one of their `STATUS.md` files may stop a
  commit until closed work moves to `docs/history`.
- A shared crate's `now` tier holds every consumer's `AGENTS.md` (14 documents
  for `celestina-core`); the budget measures one project at a time and does
  not bound that sum.
- The tier tests of `agent-context.py` were written with the implementation
  and not seen failing first.
- CI did not run. The session did not run `scripts/land-unit.py`, a production
  entry or a smoke.

## Follow-up

- These files cite the root contract's "Halted projects" section for the
  salvage record, which now lives in
  [the history record](../history/qt-shell-halt-and-salvage.md):
  `celestina-rs/crates/celestina-tokens/src/niri_colours.rs`,
  `celestina/packaging/niri/build-patched-niri.sh`,
  `celestina/packaging/niri/README.md`, `celestina/scripts/dev-session.kdl`,
  `celestina/scripts/dev-session.sh`, and ADR 0002 and ADR 0023 under
  `celestina/docs/decisions/`. They were left as they are: the code files are
  production inputs of `celestina`, and the section's pointer line leads to
  the record.
- A `suite.reading_triggers` entry for a document that is always in the `now`
  tier (a path also in `suite.now_rules`, an owner's `AGENTS.md` or
  `STATUS.md`) is accepted and ignored. The checker could refuse it.

## Landing

- **Base revision:** `3654837201f4b8163ed898a4b544313ee0c7a388`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
