# Evidence: 2026-10-08 claim-first single instance in Hematita

- **Date:** 2026-10-08
- **Scope:** `HEM-H1-F` — `hematita/src/activation.rs`, `hematita/src/main.rs`, `hematita/scripts/smoke.sh`
- **Environment:** the author's CachyOS, Qt 6.11.2, zbus 5.19.0, cxx-qt 0.9.1, `dbus-run-session` from dbus 1.16; offscreen runs only
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session worktree:

```sh
(cd hematita && cargo fmt --check)
(cd hematita && cargo clippy --all-targets)
(cd hematita && cargo test)
(cd hematita && cargo build --release)
sh hematita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/hematita
bash scripts/qmllint-cxxqt.sh hematita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

And, by hand, the race the report describes, with the second launch handing a folder:

```sh
XDG_RUNTIME_DIR=$S/run QT_QPA_PLATFORM=offscreen HEMATITA_SMOKE_SHAPE=1 dbus-run-session -- sh -c '
timeout 8 "$1" >"$2/a.log" 2>&1 & a=$!
timeout 8 "$1" "$2/folder" >"$2/b.log" 2>&1 & b=$!
wait $a; ra=$?; wait $b; rb=$?; echo "exit codes: a=$ra b=$rb"' x "$BIN" "$S"
```

## Result

- **Exit:** fmt, clippy (no Rust diagnostics; the C++ `-Wsfinae-incomplete` notes from the Qt headers are the usual ones), `cargo test` (82 passed, among them the two new `activation::tests`), the release build, the smoke, qmllint (0 non-fatal baseline warnings), the architecture contract, the language contract and the documentation contract all exited 0.
- **Observed:** the smoke's new race gate reports `two launches on one bus ended as one window`. The hand-run race ended with `a=124 b=0`: the launch that won the name was still alive when `timeout` ended it, and the other handed off and left on its own with 0 and an empty log. The winner's log carries `This plugin does not support raise()` once, which is the offscreen platform answering the `window.raise()` that the replayed request triggered: the loser's `Open` arrived at the served object before the QML had attached, waited in the inbox and reached the Qt thread on `start()`. Neither log says `cannot claim`, `cannot hand off` or `no session bus`.
- **Order of the launch now:** `main` calls `activation::claim` before `QGuiApplication` exists. The connection is built already serving `/org/celestina/Hematita` and with a three-second method timeout, then `RequestName` is asked with `DoNotQueue`. Primary owner keeps the connection for the process in a `OnceLock` and opens the window; `NameTaken` asks the owner `Open(path)` (falling back to `Activate` on `UnknownMethod`) or `Activate` and exits 0 on an answer; any other error, and a hand-off that fails or times out, is said once on stderr and the launch opens its own window. The served methods never touch Qt: they hand a `Request` to a mutex-held inbox that delivers straight through once the QML's `activation.start()` has attached the `CxxQtThread`, and keeps at most eight requests, oldest dropped, until then.
- **Equivalent recipes searched:** `grafita/src/activation.rs` and `siderita/src/dbus.rs` keep the serve-then-claim shape on a worker thread; Cuprita's claim-first design exists only in `docs/superpowers/plans/2026-10-08-cuprita-foundation.md` (no `cuprita/src` yet). The shared helper of audit finding HEM-16 stays unscheduled; this unit changes Hematita alone.

## Limits

- Offscreen only: that the raised window actually comes to the front on Wayland, and that the browse lands in the storage section, are not shown here (the smoke's start gate keys on the command-line `startPath`, not on `Open`). `VAL-S3` in `VALIDATION.md` still covers the real session.
- The hand-run race is a sample of one scheduling; the smoke's gate repeats it on every run.
- An owner that vanishes between the claim's `NameTaken` and the hand-off's call makes this launch open its own window without re-claiming the name; a third launch in that instant would open as well. Not retried, by the unit's scope.
- Production build, verification and deployment were not run in the session; they happen at the landing.

## Follow-up

None.

## Landing

- **Base revision:** `55fd83cda9b82fff3d9ebf61e13228e8e105c4ae`
- **Check:** `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:44af33b90c0d2ad39a7b985c2afa22371772aab17db4d32195b0bc04636917a1, verification_fingerprint sha256:407fd05c063d6b49fbc73bab14a4be30bdca38a014e2c6a32133aa8ab1991c01
- **Deploy:** after the push: hematita: deploy-production.sh, status-production.sh
