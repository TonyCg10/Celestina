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
