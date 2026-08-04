# Celestina documentation system

This directory gives each kind of project truth one canonical home. It is
vendor-neutral: every agent follows the same repository contract.

## Source-of-truth map

| Question | Canonical home |
|---|---|
| What is the suite and what is deliberately outside it? | [VISION.md](VISION.md) |
| What exists and what is active now? | root/project `STATUS.md`, checked against the checkout |
| What may an agent change? | root and nearest local `AGENTS.md`, plus the author's request |
| How are changes governed? | [governance/](governance/) |
| What architecture and implementation standards apply? | [standards/](standards/) |
| Which cross-project behaviours must remain compatible? | [contracts/](contracts/) |
| What was decided and why? | root or project-local `docs/decisions/`, indexed from [decisions/](decisions/) |
| Which questions are still being argued? | root or project-local `docs/discussions/`, indexed from [discussions/](discussions/) |
| What settled work is being implemented? | root/project `ROADMAP.md` and [plans/](plans/) |
| Which exact paths formed each delivery commit? | [inventories/](inventories/) or the owner-local `docs/inventories/` |
| What version does each product declare and how did it advance? | registered sources in [projects.toml](projects.toml) and [version-history.tsv](version-history.tsv) |
| What must the author test manually? | root/project `VALIDATION.md` |
| What was actually verified? | [evidence/](evidence/) or registered project-local `docs/evidence/` |
| What is retained only as history? | [history/](history/) and [plans/archive/](plans/archive/) |
| Which projects, prefixes and artifact entries exist? | [projects.toml](projects.toml) |

The checkout plus reproducible evidence wins over a stale status claim. A plan,
discussion, decision, README or roadmap never grants authority to modify an
otherwise restricted area.

## Deterministic agent context

Run `python3 scripts/agent-context.py PATH` from the repository root. It prints,
in reading order, the root and local agent rules, the cross-cutting rules
registered as `suite.shared_rules`, the selected owners' explicit
`context_documents`, every applicable general and product owner, its
README/STATUS/ROADMAP/VALIDATION set, relevant contracts and both suite and
project active plans. Shared crates therefore retain the `celestina-rs` context
and also receive the more specific consumer context.

The output is meant to be complete, not merely sufficient: a local `AGENTS.md`
names the workflow, governance and engineering standards it depends on, so the
registry lists them once and the helper prints them rather than leaving an agent
to rediscover them by following links. Adding a standard means registering it.
Every project declares `context_documents`, using an explicit empty list when it
has no additional local document. The documentation guard rejects missing
configuration and registered paths that do not exist.

## Promotion flow

```text
question -> discussion -> accepted decision -> implementation roadmap/plan
         -> agent evidence -> STATUS update

author-only test -> VALIDATION result -> optional new remediation unit
```

Pending author validation never keeps an implementation unit open.
Each closed unit keeps its immutable inventory under the owner's stable
`docs/inventories/<plan-slug>/<unit>.numstat.tsv` root. Archiving moves only the
plan; the inventory path, links and explicit stable `Plan ID` remain unchanged.
The `Plan ID` exists from plan creation and no later than the first commit that
contains one of its inventories.

## Governance

- [Change and authorization policy](governance/change-policy.md)
- [Documentation contracts](governance/documentation.md)
- [Architecture](standards/architecture.md)
- [Rust, C++, Qt and QML engineering standard](standards/rust-cpp-qt-qml.md)
- [Verification standard](standards/verification.md)
- [Repository language standard](standards/language.md)
- [Content activation contract](contracts/content-activation.md)
- [Reusable production artifacts](contracts/production-artifacts.md)
- [Product versions and typed commits](contracts/versioning.md)

## Completed migration

- [Repository governance and delivery-system migration](plans/archive/2026-08-03-repository-governance.md)
- [Guard, artifact and version-contract alignment](plans/archive/2026-08-03-guard-contract-alignment.md)

Reusable skeletons live in [templates/](templates/). Copy one only when the
corresponding canonical document does not already exist; migrate useful history
instead of deleting it.
