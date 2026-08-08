# Evidence: UX-1 delivery

- **Date:** 2026-08-08
- **Unit:** UX-1-D
- **Scope:** versioned production delivery, implementation-lane closure and
  pending author-validation hand-off for UX-1
- **Base revision:** `f6f54beb1a086d96e05af7b355e7d9d64db2b018`
- **Environment:** production build and offscreen verification with Noctalia
  retaining ownership of the live session
- **Artifact:** celestina 0.7.0, manifest
  `celestina/build/production-artifact.toml`

## Result

UX-1 is implementation-complete. The registered milestone transition moved
Celestina from 0.6.8 to 0.7.0 and appended its `UX-1-D` history row. The
canonical production exit built one release bundle, verified those exact bytes,
deployed them to the normal `~/.local` author-test prefix and reported every
installed artifact current. It did not activate Celestina or replace Noctalia.

## Procedure

Apply the registered milestone version transition, run the common architecture
and version guards, execute the registered complete production script, then
close status, roadmap, validation and the plan against the verified result.

## Automated evidence

- `bash scripts/check-architecture-contract.sh`: passed.
- `python3 scripts/version_tool.py check`: passed for all six versioned owners.
- `celestina/scripts/complete-production.sh`: passed end to end.
- Production artifact fixtures: 29 passed.
- CelestinaStyle: architecture, visual and contrast guards passed; its modal
  focus CTest passed and its eight-second gallery smoke stayed alive.
- Celestina Rust targets: 12 Niri-adapter unit tests, 45 provider-adapter unit
  tests and six provider integration tests passed.
- Shared Rust crates: 32 `celestina-core`, 247 `celestina-shell-core` and 98
  `magnetita-core` tests passed; formatting and Clippy with warnings denied
  passed.
- Celestina QML lint passed.
- Celestina CTest: 17/17 passed, including the real private-bus tray watcher,
  the durable request ledger and the 16-case indicator-menu suite.
- The eight-second production host/style smoke passed.
- Final deployment status reported the host, both Rust helpers, style library,
  style module, launcher and desktop entry installed and current.
- `git diff --check`: passed.

## Limits

The production smoke mapped no Wayland surface. No network, Bluetooth, monitor,
Niri or live shell state was changed by verification. Menu placement, compositor
input delivery, real focus restoration and safe device actions remain pending
under `VAL-UX-1`. Noctalia remained the session owner throughout.
