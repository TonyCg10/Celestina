# Documentation governance

## One canonical home per kind of truth

| Location | Owns | Must not own |
|---|---|---|
| `AGENTS.md` | durable operational rules and boundaries | milestone state or volatile counts |
| `README.md` | current product role, stack, structure, and use | backlog or agent policy copies |
| `STATUS.md` | current focus, implemented truth, blockers | long-term plan or immutable history |
| `ROADMAP.md` | implementation checkpoints and agent exits | author-only tests |
| `VALIDATION.md` | manual author checks and results | implementation tasks |
| `docs/plans/active/` | settled execution order and change ledger | open design debate |
| `docs/inventories/` | exact stable immutable unit inventories | backlog or execution prose |
| `docs/decisions/` | accepted decisions and consequences | active work lists |
| `docs/discussions/` | dated unresolved/adversarial reasoning | silently settled policy |
| `docs/evidence/` | commands, environment, observations, limits | aspirations |
| `docs/history/` | preserved superseded material | current instruction |

Link to a canonical rule instead of copying it.

## Language

Current rules, documentation, and templates are English under
[the language standard](../standards/language.md). Historical records may retain
original bytes until a dedicated translation. Conversation with the author may
be Spanish and is not copied into repository documents verbatim.

## Roadmap and validation split

Implementation closes on agent-executable evidence. A pending Wayland,
appearance, hardware, physical keyboard, portal, or AT-SPI check never keeps an
implementation checkpoint open. Those checks have independent `VAL-*` entries
in `VALIDATION.md`.

A manual failure records observation/evidence and opens a linked corrective
implementation unit. It does not rewrite the closed checkpoint.

## Plans

Use one active plan for each owner/checkpoint pair. A plan states:

- immutable Plan ID and lifecycle status;
- hypothesis and tangible outcome;
- included scope and exclusions;
- causal build order;
- integrated agent-executable exit;
- durable change/commit ledger;
- related author-validation ID.

Open design questions belong in dated discussions. Once concluded, apply the
decision to canonical documents and active work before marking the discussion
applied.

## Plan lifecycle

- `active`: settled authorized work is being implemented.
- `blocked`: the same plan remains current but a named external condition stops
  progress.
- `completed`: every unit is done and the plan may be archived.
- `archive/`: immutable implementation history; not current instruction.

An active or blocked roadmap and active plan must point to the same checkpoint.
Idle, planned, or done roadmaps point to `none`.

Archiving moves only the plan and preserves basename, Plan ID, checkpoint,
units, links, evidence, and stable inventory roots. A separate archive commit
requires a new administrative unit and inventory.

## Ledger closure

While open, `Files / areas` names stable paths, sections, or symbols. On closure
it becomes one relative link to the unit's stable inventory. `Diffstat` becomes
exact `N files, +X/-Y`, and automated evidence becomes a link to the real dated
record.

The inventory format and commit rules are defined in
[change-policy.md](change-policy.md). The documentation guard verifies:

- unique project IDs, prefixes, Plan IDs, and validation IDs;
- exactly one active plan per active/blocked roadmap;
- no orphan plans;
- valid local links and registered roots;
- complete evidence/inventory links for done units;
- exact inventory paths, hashes, numstat, base, and Pathspec union;
- immutable historical inventory endpoints;
- correct archive transitions.

## Status and snapshots

`STATUS.md` may contain volatile facts only when cheaply reproducible. Prefer
commands and qualitative state over counts that immediately drift. If a touched
count cannot be reverified, remove it or label it with date/source.

`[x]` means evidence exists. Code presence alone is not behavior proof. A build
does not imply startup; offscreen does not imply real compositor behavior.

## Decisions and discussions

Decision records state context, ruling, consequences, alternatives, and revisit
conditions. Discussion records preserve proposal, strongest counter-case,
alternatives, verdict, falsifiers, and exact roadmap instructions. A concluded
discussion is not applied until canonical sources change.

Indexes list each record once and the guard checks lifecycle consistency.

## Evidence

Evidence records exact date, scope, environment, artifact, command/procedure,
exit/result, observed facts, limitations, skipped checks, and follow-up.
Simulation and offscreen results are labelled. Do not infer appearance,
interaction, hardware, or accessibility from compilation.

## Project creation

A new registered project includes, in one coherent unit:

- entry in `docs/projects.toml` with owner, prefix, source/commit roots, and
  production entries;
- local `AGENTS.md`, `README.md`, `STATUS.md`, `ROADMAP.md`, and
  `VALIDATION.md`;
- project-local `docs/plans/{active,archive}`, `docs/evidence`, and
  `docs/inventories` roots;
- canonical production scripts appropriate to deployability;
- CI/guard discovery from the registry rather than hard-coded omissions;
- English repository content and language-contract coverage.

Do not create a second documentation tree when a canonical owner already
exists.
