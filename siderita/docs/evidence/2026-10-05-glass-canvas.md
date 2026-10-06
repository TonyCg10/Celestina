# Evidence: 2026-10-05 the glass canvas in Siderita

- **Date:** 2026-10-05
- **Scope:** `SID-H1-F` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with a compositor blur window rule for the Celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built as a spike on the author's session and judged by eye at each step:

```sh
cd siderita && cargo build --release --locked --bin siderita
target/release/siderita
```

The author opened the properties dialog of a folder, a context menu over the
grid with the wheel, and watched the sort pill at rest and while the grid
scrolled under it.

## Result

- **Exit:** the release build finished; the window started with no QML
  warning on stderr.
- **Observed:** `Main.qml` sets `color: CelestinaTheme.clear`, so the
  compositor blur shows through and `CelestinaBackdrop` paints the Haze canvas
  (STYLE-G7-Q). `GlassPill` captures at rest as well as while floating, so the
  bottom controls read as the same glass as the location bar; only the shadow
  follows `floating`. The usage crumbs in `FolderUsage.qml` take
  `focusPolicy: Qt.NoFocus`: the dialog's opening focus never lands on them,
  and the ring the author reported is gone, confirmed by the author on the
  session.

## Limits

- The compositor blur depends on the author's Niri rule; without it the
  window is a transparent canvas tinted at 0.70 over whatever the compositor
  paints.
- Keyboard users reach the usage list and treemap, not the crumbs; the crumbs
  stay pointer-only by the author's request.

## Follow-up

None.

## Landing

- **Base revision:** `8021d53bb668a0d4117d6cd75fb664de0e691106`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact is not verified yet; run verify-production.sh
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e6ac4dc4c82ffa02082f211da562e31efbe8bbc18490a0493bd4c9872cc2cac7, verification_fingerprint sha256:6617b13ff442a9a90e209b90a5ce3340d73feed35e9665ee40fae19490df488a
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
