# CI contract map

Project workflows limit compilation by path. `contracts.yml` always runs
because it verifies relationships across projects.

| Workflow | Triggered by | Evidence |
|---|---|---|
| `contracts.yml` | every push/PR | architecture/style, documentation/language, agent context, commit scope |
| `celestina-rs.yml` | `celestina-rs/` | Rust workspace formatting, lint, tests, backend coverage |
| `celestina.yml` | shell or shared Rust changes | shell build and tests |
| app workflows | app, associated crates, or shared style | app-specific build/tests |

`scripts/check-architecture-contract.sh` checks dependency direction, QML
registration, shared style, and structural debt ratchets. Its scanners have
positive and negative fixtures.

The documentation guard checks vendor-neutral rules, project registry,
metadata/lifecycles, links, plan/roadmap matching, exact inventories, and archive
transitions. The language guard requires English canonical sources and prevents
legacy language debt from growing.

Commit-scope tests validate prefixes and staged-unit inventory unions. Local
hooks are repeated in CI because `core.hooksPath` does not travel with Git.

`contracts.yml` intentionally has no path filter: a change in one project can
break another project's shared-style or dependency contract. App workflows
include associated `celestina-rs/` inputs because omitting a newly added crate
would silently stop testing consumers.

CXX-Qt applications need Qt and, for real acceptance, a Wayland compositor,
portals, hardware, and AT-SPI. Registered production scripts build and verify
locally; CI/offscreen proves compilation and startup, not real interaction,
appearance, or accessibility. Those checks remain in project `VALIDATION.md`.
