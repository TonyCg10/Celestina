# BUBBLE-1 native minimized-window delivery

- **Date:** 2026-08-18
- **Scope:** `BUBBLE-1-A`; Melibea protocol consumption, the aggregate
  provider, panel bubble group, selector lifecycle, combined compositor build,
  production bundle and deployment
- **Environment:** CachyOS; Qt 6.10; Rust 1.85-compatible workspaces; Niri
  26.04 checkout at `/home/toni/CODIGO/NIRI-MELIBEA`; disposable winit Niri on
  `wayland-2`; production prefix `/home/toni/.local`
- **Artifact:** Celestina 0.32.0 plus Melibea and the combined Niri 26.04
  release carrying native minimization and Celestina's per-layer blur-strength
  patch

## Procedure

### Contract and automated evidence

`celestina-shell-core` treats every Melibea line as bounded hostile input,
preserves `u64` identities as decimal strings at the Qt boundary, applies only
sequential revisions atomically and requires a fresh snapshot after loss. The
aggregate helper owns one reconnecting subscription and one-shot action
connections. An accepted action changes no row; only a later authoritative
absence confirms it.

The following passed:

- Melibea: 62 library tests, 12 command tests, strict Clippy, format check and
  release build.
- Combined Niri: 211 compositor tests, 19 configuration tests, the wiki parser,
  four IPC tests and the IPC documentation test. The configuration regression
  parses `passes 2` and `offset 5`, and the release binary validates the real
  `~/.config/niri/config.kdl` containing the dense-glass override.
- Celestina Rust: 342 shell-core tests, 94 aggregate-provider tests, 27 Niri
  adapter tests, and the held-child, MPRIS and notification integration tests.
  The Niri adapter regression proves that the additive
  `MinimizedWindowsChanged` event cannot tear down the ordinary shell stream
  while Celestina remains pinned to upstream `niri-ipc` 26.4.0.
- Celestina QML and C++: the complete QuickTest runner, including compact-group
  overlap, pointer restore and window-level Delete close; QML lint; and all 25
  CTest contracts, including shell service, overlay lifecycle and output
  chooser.
- The architecture, language, colour, contrast, production-artifact, smoke and
  handover guards passed against the release bundle.

### Nested compositor evidence

The disposable session used Niri PID 1599915 and socket
`/run/user/1000/niri.wayland-2.1599915.sock` on an isolated session bus. Two
Kitty windows were natively minimized. The compact group reconstructed from a
fresh subscription after shell restarts, and `celestina msg bubbles-toggle`
returned `confirmed`.

`/tmp/melibea-bubble-selector-open.png` records the selector with both ordered
windows. Pressing Return restored the selected first window and the subscribed
state moved from two entries to one before the chooser retired. For the
remaining window, the reference CLI first reported revision 4 with one entry,
`close 3` returned `close-requested`, and a later list reported revision 5 with
zero entries. That proves close acceptance was not mistaken for destruction.
`/tmp/melibea-selector-before-delete.png` records the final one-row visual and
its explicit close control. The test nest, shell, daemon and stale test socket
were then stopped without touching the host compositor.

## Result

### Production and deployment

`celestina/scripts/complete-production.sh` built Celestina 0.32.0 once and
verified the exact manifest. A different development session started a
build-tree Celestina before deployment; the session interlock correctly
refused to overwrite that live executable. After terminating only the reported
PID and waiting until no `ddcutil` child remained, the verified manifest was
deployed with `celestina/scripts/deploy-production.sh` and
`status-production.sh` reported every installed artifact current and verified.

Melibea and the combined Niri release were copied atomically after byte-for-byte
comparison. Installed SHA-256 values were:

- Melibea: `5f67abc8972157f7c8f5eb8879ef914e23975066e4e7e0868d0dae7405bd1cb2`
- Niri: `db7af1422405cb6865a0fa8fa0820013a94fea60cc9d4c17e58e5de4fa930d49`

## Limits

The screenshots are agent-run nested evidence at scale 1, not author
perceptual acceptance. A real pointer pass, physical keyboard pass, AT-SPI
navigation, icon-theme review and multi-output bubble placement remain
`VAL-BUBBLE-1`. No preview or coordinated window-to-bubble trajectory is
claimed; those remain possible Melibea M7 work.
