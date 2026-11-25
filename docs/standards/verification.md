# Verification standard

Evidence runs in the layer that can observe the claim. Compilation, startup,
interaction, appearance, and accessibility are different proofs.

## Agent lane

An agent closes implementation only with reproducible checks and, for a
deployable app, an updated author-test binary:

1. common architecture guard;
2. affected formatting, lint, domain, and app tests;
3. build of the exact production artifact;
4. safe verification and smoke against those same bytes;
5. deployment without recompilation plus matching installed status through
   `complete-production.sh`;
6. scanners for every changed cross-cutting contract;
7. exact commands, results, and limits in evidence or the ledger.

A session runs items 1, 2 and 6 in its worktree and records item 7 there.
Items 3 to 5 happen at landing: on the tree it commits, `scripts/land-unit.py`
builds and verifies every affected project whose artifact is not current,
completes each deployable one, and appends the result to the evidence record;
see
[the landing contract](../contracts/landing.md).

`verify_script` never installs or activates. Registered `complete_script`
chains the full exit and never activates the live shell. See
[production-artifacts.md](../contracts/production-artifacts.md).

## Minimum matrix

| Area | Minimum agent evidence |
|---|---|
| Common architecture | `bash scripts/check-architecture-contract.sh` |
| `celestina-rs` | fmt, clippy with warnings denied, workspace tests |
| Application Rust | fmt, clippy, and affected package/crate tests |
| QML | registration, `qmllint`, build, and safe surface smoke |
| `celestina-style` | visual guard, automated gallery/lint, affected consumers |
| Guard or CI | positive and negative fixtures plus normal execution |
| D-Bus/protocol | producer tests and consumer compatibility |
| Documentation | schema, inventory, links, and lifecycle |
| Language | canonical full scan and non-growth legacy baseline |

Run the narrowest subset that proves the change plus every required
cross-cutting guard. A missing dependency or environment means “not run,” not a
pass.

## Author lane

Real Wayland, appearance, compositor, hardware, portals, physical keyboard, and
AT-SPI live in `VALIDATION.md`. A completed implementation unit does not remain
open for them. A manual failure creates a linked corrective unit.

## Evidence wording

- State the exact command and exit status.
- Name the artifact or manifest under test.
- Distinguish offscreen, simulated, and real-session results.
- Record skipped checks and the missing precondition.
- Do not claim behavior from code presence, a build, or a smoke alone.

## Sensitive paths

A change is sensitive when it touches any of these:

- the session lock: `celestina/crates/celestina-lock`,
  `celestina/crates/celestina-lock-verify`,
  `celestina-rs/crates/celestina-lock-proto`, `celestina/src/lock`,
  `celestina/crates/celestina-wayland/src/lock.rs` and `src/lock/` beside it,
  `celestina-rs/crates/celestina-shell/src/lock.rs`,
  `celestina/scripts/lock-rehearsal.sh`;
- polkit, PAM and secrets: `celestina/src/polkit`,
  `celestina/src/providers/polkit`,
  `celestina-rs/crates/celestina-shell/src/polkit.rs`,
  `celestina/crates/celestina-ui/src/secret.rs`, `celestina/contrib/pam.d`;
- their source guards: `celestina/tests/lock_*.rs` and
  `celestina/tests/polkit_*.rs`;
- what a landing runs: a `*-production.sh`, `smoke*.sh` or `qml-tests.sh`
  script of any project;
- the guards and the landing: `scripts/*.py` and `scripts/*.sh` at the
  repository root, `scripts/fixtures/`, `scripts/documentation-errata.tsv`,
  `.githooks/`, `.github/`;
- the rules themselves: `docs/projects.toml`, any `AGENTS.md`,
  `CONTRIBUTING.md`, `docs/contracts/`, `docs/governance/`, `docs/standards/`.

The ratchet baselines (`scripts/*-baseline.tsv`) are not sensitive.

## Light units

A unit is light when all three hold:

1. its kind is `maintenance` or `bug`;
2. it changes at most 150 lines, added plus deleted, outside documents,
   lockfiles and ratchet baselines (`git diff --numstat origin/main...`,
   leaving out `*.md`, `Cargo.lock` and `scripts/*-baseline.tsv`);
3. it touches no sensitive path.

The session sizes the unit before it starts and writes `light unit` in the
`Scope` line of its evidence. It measures the unit again before the review
round and gives that line count in the evidence. A unit that stops qualifying
becomes an ordinary unit from that point: the session says so and follows the
ordinary rules for what remains.

A light unit:

- has no design and no task-plan document; its ledger row describes it;
- is implemented by the session itself, without subagents;
- records its evidence from
  [the short template](../templates/evidence-light.md): at most 40 lines
  before the section the landing appends, with commands, exits and limits,
  and no narrative or review history;
- takes one review round.

Everything else is ordinary: the ledger row, the inventory, the version rule,
and the landing's build, verification and full smoke.

## Review rounds

A round is one independent review of the unit's diff, followed by its fixes.
Every unit takes at least one. Beyond that these are caps, not targets, and
each later round reads only the fixes of the one before:

| Unit | Rounds at most |
|---|---|
| Light | 1 |
| Ordinary, off the sensitive paths | 2 |
| Touches a sensitive path | 3 |

After the last round, a finding that is a defect in what the unit set out to
do is fixed and checked by the session, with no further reviewer. Any other
finding becomes a backlog entry in the owner's plan or a line under
`Follow-up` in the evidence. More rounds need the author's request.

Evidence records what ran, not the review: one line gives the rounds, the
findings fixed and the findings deferred.
