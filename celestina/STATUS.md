# Celestina status

- **Updated:** 2026-08-03
- **Implementation:** R0-R2 complete; R3 active
- **Author validation:** mixed; see [VALIDATION.md](VALIDATION.md)
- **Live migration:** Noctalia still supplies every responsibility not yet
  explicitly handed over by the author

## Current checkout truth

- A C++20/Qt 6.9+ host maps one top layer-shell panel per output and owns the
  `org.celestina.Shell1` session interface.
- Rust helpers reduce Niri state and carry the aggregate providers through the
  pure `celestina-shell-core` contracts.
- The panel contains real workspace/window, system, media, audio, DDC and tray
  state. It can become the StatusNotifierWatcher when no other process owns it.
- The launcher and clipboard-history overlays are implemented and use the same
  surface and command contracts.
- R3 session verbs, OSD, night light, caffeine/idle and composed lock are not
  implemented yet.
- No task document authorizes changing Niri configuration, installing a locker,
  activating the shell or stopping Noctalia.

## Durable boundaries

`celestina-shell-core` owns pure protocol and policy. Rust helpers own bounded
non-Qt IO. C++ owns Qt, D-Bus and layer-surface adaptation that CXX-Qt cannot
express cleanly. QML owns presentation only. See [AGENTS.md](AGENTS.md) and the
suite [architecture standard](../docs/standards/architecture.md).

## Evidence boundary

The canonical release bundle was built and verified on 2026-08-03: Rust tests,
direct QML lint, CTest 11/11, an eight-second offscreen smoke of the release host
with the compiled style module, and dynamic-library checks passed. Exact
artifacts, commands and limits are recorded in the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). This does not
replace any real Niri check in [VALIDATION.md](VALIDATION.md).

## Records

- Current implementation plan: [R3 session verbs](docs/plans/active/2026-08-03-r3-session-verbs.md)
- Open product questions: [discussion queue](docs/discussions/README.md)
- Accepted product decisions: [decision index](docs/decisions/README.md)
- Completed detailed roadmap: [history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
- Original phase work orders: [Noctalia replacement history](docs/history/noctalia-replacement-through-2026-08-03.md)
