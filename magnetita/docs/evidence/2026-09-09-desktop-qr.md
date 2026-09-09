# The desktop app shows the pairing QR — MAG-P3-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P3-B` of
  [`../plans/active/2026-09-09-android-foundation.md`](../plans/active/2026-09-09-android-foundation.md):
  `src/pairing.rs`, `src/devices.rs`, `src/controller.rs`,
  `src/projection.rs`, `src/main.rs`, `build.rs`, `Cargo.toml`,
  `Cargo.lock`, `qml/components/PairingSheet.qml`,
  `qml/pages/DevicesPage.qml`, this record
- **Environment:** the desktop app built from this tree, driven headlessly
  (`QT_QPA_PLATFORM=offscreen`) against the live daemon's session bus with a
  temporary probe in `Main.qml` that called `startPairing` and grabbed the
  surface; the probe was not committed
- **Artifact:** `target/release/magnetita`, deployed through
  `scripts/complete-production.sh`

## Design

- `devices::start_pairing` calls `Devices1.StartPairing` and returns the
  `magnetita://pair` text. `pairing::matrix` encodes it (`qrcode`, level M)
  as rows of `#` and `.`; `pairing::Watch` remembers the paired ids at the
  moment the window opened and names the first paired device outside that
  set, the phone the QR admitted.
- The controller exposes `pairingMatrix`, `pairingActive`, `pairingWindow`
  (the daemon's 120 s) and `pairingArrived`; `startPairing` runs the call
  off the GUI thread, `dismissPairing` hides the code. Every device
  snapshot passes through the watch, so the arrival needs no new signal.
- `PairingSheet` draws the code as one rectangle per run of dark modules
  on a white card with a quiet zone, whatever the theme, and counts the
  window down; expired, the card dims and a link action generates a new
  code; arrived, the code gives way to the phone's name. `DevicesPage`
  carries the entry, a link icon with the suite's hover circle, above the
  device cards, also when no phone is listed.
- `projection::state_label` now reads a paired, connected phone as
  connected: the mount is the KDE Connect wire's last step and the own
  wire has none, so the own-wire phone no longer shows as connecting.

## Procedure

```sh
cd magnetita
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cargo build --release --locked && ../scripts/qmllint-cxxqt.sh .
XDG_CONFIG_HOME=<scratch> QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 target/debug/magnetita   # with the probe
scripts/complete-production.sh
```

## Result

- **Exit:** 0. 14 app tests (two new: the matrix is square with the finder
  pattern in the corner; the watch names only a device that was not
  paired before). `qmllint`: OK with the 14 baseline warnings, none new.
- **Observed in the grab:** the pairing section label, the QR on its
  sheet, the hint line counting down from 120 s (118 s in the grab), the
  close action; below it the own-wire phone `SM-S938U` in its connected
  state with its battery, and the KDE Connect phone in its mounted state.
- **A first grab showed the defect the fix addresses:** the code read as
  expired at once because the window value arrived after the rows; the
  controller now sets the window first.

## Limits

- The phone's arrival replacing the code is verified by the unit test of
  the watch and not in the grab: the probe ran without a phone scanning.
  `VAL-MAG-11` is the author's scan of this very screen.
- The six-digit code stays off the wire; the QR is the one path.
