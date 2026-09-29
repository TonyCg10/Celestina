# Celestina Rust workspace status

- **Updated:** 2026-09-26
- **Implementation:** the registered workspace crates are present;
  `dotfiles-core` has no consumer (see the RS-H1 plan's exclusions); `RS-H1`,
  the shared owners that follow the 2026-09-26 monorepo audit, is the active
  checkpoint, and its unit `RS-H1-A` adds those owners to `celestina-core`
  without switching any consumer
  ([plan](docs/plans/active/2026-09-26-hardening.md))
- **Author validation:** routed to the owning applications; see
  [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- The workspace contains pure domain cores for the shell, Siderita, Grafita,
  Fluorita and Magnetita, plus the explicitly bounded transport, daemon, media
  engine and Qt render-seam exceptions described in [AGENTS.md](AGENTS.md).
- Siderita consumes the file, Grafita and Fluorita contracts. Standalone
  Grafita and Fluorita consume the same cores through separate adapters.
- Magnetita's Qt app is a client of the `magnetitad` D-Bus service; protocol and
  network ownership remain in this workspace.
- `celestina-core` carries dormant owners that no application calls yet: the
  strict `file_uri` parser, `xdg::runtime_dir` (no `/tmp` fallback) and
  `xdg::ensure_private_dir` and `xdg::effective_uid`,
  `atomic_file::replace_private`, `land_media`, `stage_media`,
  `publish_without_replacing` and `read_bounded` with the `Published` outcome,
  `desktop_entry::{read, scan, find}` and
  `CancellationToken::is_cancel_requested`. Until each product's own unit
  adopts them, that product's copies keep their current behaviour, including
  the `/tmp` fallbacks and the unbounded `.desktop` reads the audit found.
- Development manifests use sibling `path` dependencies. A compatibility and
  release-versioning promise has not been accepted yet, so old claims that a
  release already consumes pinned crate versions are not current truth.
- `dotfiles-core` produces plans only. Applying changes remains outside its
  current public contract.

## Planned implementation debt

- `siderita-core` still needs the settled executor correction that cancels a
  running scan when a newer scan supersedes it, with cancellation and shutdown
  tests.
- Public API stability and family versioning require an accepted decision
  before implementation work can be planned.
- Config, activation, MIME and handler helpers are extracted only when concrete
  consumers demonstrate the same contract; they are not an active feature
  bucket.

## Evidence boundary

The detailed implementation and earlier evidence are preserved in the
[historical roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the canonical workspace release passed format, workspace Clippy with
`-D warnings` and the complete locked workspace tests; the final manifest seals
the exact artifact. The initial sandbox could not open loopback sockets, so the
same matrix was rerun with approved unrestricted execution and passed. See the
suite [evidence](../docs/evidence/2026-08-03-repository-governance.md).

## Records

- [Implementation roadmap](ROADMAP.md)
- [Manual-validation routing](VALIDATION.md)
- [Workspace registry entry](../docs/projects.toml)
