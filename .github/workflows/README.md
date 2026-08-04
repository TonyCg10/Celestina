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
append-only history fixtures and exact typed-commit SemVer transitions. Its
history audit replays every non-merge commit after adoption so disabling local
hooks cannot publish an untyped or unversioned delivery unnoticed.

`contracts.yml` intentionally has no path filter: a change in one project can
break another project's shared-style or dependency contract. The two Rust
workflows include their associated `celestina-rs/` inputs because omitting a
newly added crate would silently stop testing those Rust consumers.

CXX-Qt applications need Qt and, for real acceptance, a Wayland compositor,
portals, hardware, and AT-SPI. Registered production scripts build and verify
locally; CI/offscreen proves compilation and startup, not real interaction,
appearance, or accessibility. Those checks remain in project `VALIDATION.md`.
