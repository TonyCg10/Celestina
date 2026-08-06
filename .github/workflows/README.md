# CI contract map

Project workflows limit compilation by path. `contracts.yml` always runs
because it verifies relationships across projects.

| Workflow | Triggered by | Evidence |
|---|---|---|
| `contracts.yml` | every push/PR | architecture/style, documentation/language, versions, agent context, commit scope |
| `celestina-rs.yml` | `celestina-rs/` | Rust workspace formatting, lint, tests, backend coverage |
| `celestina.yml` | shell or shared Rust changes | shell Rust-helper formatting, lint and tests; no Qt/C++ host build |

The Qt/C++ shell host, the CXX-Qt applications and `celestina-style` have no
workflow. Building them in CI needs Qt, and their registered
`verify-production.sh` runs the corresponding matrices locally against the
exact release artifact. Until such workflows exist, their compilation evidence
is local only; do not read a green `contracts` or `celestina` run as proof that
the shell host, Siderita, Magnetita, Grafita, Fluorita or the style module
builds.

`scripts/check-architecture-contract.sh` checks dependency direction, QML
registration, shared style, and structural debt ratchets. Its scanners have
positive and negative fixtures.

The documentation guard checks vendor-neutral rules, project registry,
metadata/lifecycles, links, plan/roadmap matching, exact inventories, and archive
transitions. The language guard requires English canonical sources and prevents
legacy language debt from growing.

Commit-scope tests validate prefixes and staged-unit inventory unions. Local
hooks are repeated in CI because `core.hooksPath` does not travel with Git.
The version contract checks registered Cargo/CMake declarations, mirrors,
append-only history fixtures and exact typed-commit SemVer transitions.

## What the published-history replay does and does not cover

Two steps read published history and they cover different properties. Neither
is a full re-run of the local hooks, so a commit that bypassed them — from an
unconfigured clone, from `--no-verify`, or from the web UI — is re-checked only
in part. What follows is the whole of it.

`scripts/test-commit-scope.sh` replays every non-merge commit since the
convention was adopted, in `--history-scope-only` mode. That mode checks one
property: that the subject's prefix covers the paths the commit actually
touched. It deliberately does **not** require an English imperative or a
declared change kind, because commits written before those rules are part of
the same range and are legitimate. It also reads `docs/projects.toml` from
`HEAD`, not from each commit, so widening a project's `commit_roots` today makes
older commits pass that would not have passed against the registry in force when
they were written.

`scripts/audit-version-commits.py` is the stricter replay, and the one that
carries the claim the other cannot. It replays every non-merge commit after the
version policy was adopted, reads the registry from each commit's **own parent**
rather than from `HEAD`, requires the typed subject, rejects a published
`fixup!`/`squash!`/`amend!`, and checks the SemVer transition against the
manifests as they stood. That is what stops an untyped or unversioned delivery
from reaching `main` unnoticed.

Neither step audits a merge commit. A merge declares no prefix of its own, so
it has no scope to replay; the local `commit-msg` hook does check a merge's
index against the debt ratchets and against the inventory rules, and CI does not
repeat that check. Published history is linear today, so nothing has yet passed
through that gap — but a merge pushed from an unconfigured clone would be
audited by neither replay.

Everything else the hooks enforce — the ratchet rules, the staged inventory
batch, the documentation and language contracts — is re-run in CI against the
checked-out tree, not against each commit in the range. A pushed tree that is
red is caught; an intermediate commit that was red is not.

`contracts.yml` intentionally has no path filter: a change in one project can
break another project's shared-style or dependency contract. The two Rust
workflows include their associated `celestina-rs/` inputs because omitting a
newly added crate would silently stop testing those Rust consumers.

CXX-Qt applications need Qt and, for real acceptance, a Wayland compositor,
portals, hardware, and AT-SPI. Registered production scripts build and verify
locally; CI/offscreen proves compilation and startup, not real interaction,
appearance, or accessibility. Those checks remain in project `VALIDATION.md`.
