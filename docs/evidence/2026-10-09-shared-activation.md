# Evidence: shared activation

- **Date:** 2026-10-09
- **Scope:** CONV-1-A — suite
- **Environment:** CachyOS, rustc 1.98.1, Qt 6.11.2, Python 3.14.7,
  dbus-run-session 1.16.2; debug and release builds in the shared session
  Cargo target
- **Artifact:** not applicable

## Change

- `celestina_core::activation` (feature `activation`, default off; zbus 5
  blocking API) owns the claim-first hand-off that was HEM-H1-F's: the
  object `/org/celestina/<App>` with interface `org.celestina.Application1`
  (`Activate()`, `Open(as paths)`, each element a `pathkey` key) is served
  before the name is requested with `DoNotQueue`; on `NameTaken` the launch
  calls the owner with a 3 s method timeout and exits; a bus failure is said
  once and answers `Unsettled`. When the owner exits between `NameTaken` and
  the call (`ServiceUnknown`/`NameHasNoOwner`), the name is requested once
  more instead of opening a window that owns nothing. An owner that predates
  the shared interface (`UnknownMethod`/`UnknownInterface`/`UnknownObject`)
  is named as such on stderr: close it once. Requests wait in a bounded inbox
  (16, oldest dropped and counted) until `Owner::attach`. `claim_or_exit`
  wraps `claim` for `main` (exit 0 on hand-off, `None` when unsettled), so
  each adapter keeps only its cxx-qt sink. `open_in` is the client half. The
  five names (`SIDERITA`, `GRAFITA`, `HEMATITA`, `FLUORITA`, `CUPRITA`) live
  in the crate; each application's `APP_ID` and Siderita's targets use them.
- The three private copies' hand-off code is gone: Grafita's `OpenDocument`
  server and `hand_off`, Hematita's claim and inbox, Cuprita's claim and
  wake-up hook. Each application keeps a thin `activation.rs` adapter (a
  QObject that implements `Activatable`, holds the `Owner` and queues to Qt);
  Cuprita's stays under `src/controller/`, because cxx-qt-build accepts the
  bridges of one directory per QML module. Siderita and Fluorita each feed
  one long-lived worker through a channel, in arrival order: it asks the
  filesystem (and Fluorita the stored sources), never the bus thread or the
  Qt thread, and an `Activate` after an `Open` raises the window after its
  tabs exist.
- `Open` per application: Grafita one tab per document; Hematita the storage
  view at the first folder, carried as its path key byte-exact (the window's
  `startPath` too, and `openPath` now takes a key); Cuprita raises; Siderita (new name
  `org.celestina.Siderita`, never claimed by a `--portal` or
  `--file-manager` process) one tab per folder, a file shown in a tab on its
  folder with the file selected; Fluorita (new single instance, its own
  connection beside MPRIS) plays the first playable file through the same
  path as `requestedKey` and ignores the rest with a stderr note, a folder
  that is a configured source becomes the selected source, any other folder
  and a bare launch raise.
- Siderita's activator (`editor.rs`, `media.rs`, `usage.rs`) calls
  `apps::open_in` on a worker: `open_in` first; the spawn (with the path)
  when nobody owns the name or the bus cannot settle it (no session bus, a
  timeout), so the launch carries on standalone; only an owner that answers
  and refuses is a failure. A failure reaches QML as `standaloneFailed` /
  `handoffFailed`, and the desktop-handler fallback is kept.
- `scripts/activation_contract.py`, run by the architecture contract, scans
  each Rust file of every registered application as one text, its comments
  removed by a small tokenizer that skips string, raw-string and char
  literals (so `"file://"` or `"image/*"` hide nothing), and refuses a bus-name claim (`request_name`,
  `request_name_with_flags`, or `.name(...)` on a zbus connection builder,
  also split across lines) and any `"org.celestina.<Capital>` literal
  outside the crate. The allowlists are keyed by (file, argument) and
  (file, literal), each with its reason: Siderita's `FileManager1` and portal
  names, Fluorita's MPRIS name, the Magnetita daemon's client names, and a
  desktop-file name in a Grafita test. Its fixture tests show a reintroduced
  call in `grafita/src/activation.rs` refused, a call split across lines, a
  builder `.name(...)`, a second claim in an allowlisted file, a spelled
  suite name, a claim after `"file://"` on the same line and a claim after an
  `"image/*"` string each refused.

## Procedure

```sh
cd celestina-rs && cargo test -p celestina-core --features activation -- --nocapture
# in each of siderita grafita hematita fluorita cuprita:
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
cargo build --release --locked
# from the root:
sh siderita/scripts/qml-tests.sh; sh cuprita/scripts/qml-tests.sh
sh <app>/scripts/smoke.sh --binary .cargo-target/release/<app>   # each app
bash scripts/qmllint-cxxqt.sh <app>                                # each app
bash scripts/check-architecture-contract.sh
python3 scripts/test-activation-contract.py
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-land-unit.py
```

The bus-level test re-runs itself under `dbus-run-session`, on a private bus;
no application window was started on the author's session. Its transcript:

```text
private bus: one owner, one hand-off, Open delivered [Open(["/tmp/a b.txt", "/tmp/mal-\xFF"]), Open(["/tmp/a b.txt", "/tmp/mal-\xFF"])]
test two_claims_make_one_owner_and_one_hand_off ... ok
```

Two claims from two threads: one `Owner`, one `HandedOff`; the loser's
`Open` (with a non-UTF-8 name) waited in the inbox, replayed on `attach`,
and a later `open_in` reached the owner directly, byte-exact.

## Result

- **Exit:** every command above exits 0. Rust tests added: crate 5
  (`tests/activation.rs`) and 1 unit test (an owner that left is claimed
  again once), the scanner 13, adapters in Grafita, Hematita, Cuprita, Siderita
  (route and per-path open) and Fluorita (first media file, source folder),
  Siderita's `settle` decision with fake `open_in` answers; QML tests added:
  `cuprita/tests/qml/tst_activation.qml`,
  `siderita/tests/qml/tst_activation_route.qml`. Hematita's smoke race gate
  (two launches on a private bus, one window) passes over the shared code.
- **Observed:** qmllint rows unchanged (Siderita 241, Cuprita 0); the
  language baseline loses Grafita's `activation.rs` row (its debt is gone).

## Limits

- Fluorita has no play queue: files after the first are ignored with a note
  on stderr. A queue is a later Fluorita unit (ROADMAP, "Conditions for
  opening the next checkpoint").
- Upgrade note: there is no fallback to the old per-application interfaces
  (`org.celestina.Grafita.OpenDocument`, `org.celestina.Hematita.Open(s)`,
  `org.celestina.Cuprita.Activate`). Instances started before this change
  must be closed once; until then a new launch fails to hand off and opens
  its own window.
- Grafita: a bare second launch now raises the running window instead of
  opening a second one (the spec's single instance).
- A hung owner costs Siderita about 6 s and a duplicate window: `open_in`
  times out (3 s), Siderita spawns the program, and the new launch's own
  hand-off times out too (3 s) and opens a window.
- Cuprita's `claim` passes no arguments, so a second launch with arguments
  sends `Activate`, not `Open`; Cuprita opens no files either way.
- Hematita's `openPath` still asks `is_dir` on the Qt thread, as before this
  unit.
- Live checks on a real session (second launch of each application,
  Siderita opening Grafita, Fluorita and Hematita) are the author's.

## Follow-up

CONV-1-B.

## qmllint baseline re-measured under Qt 6.12.0

The host's `qt6-declarative` moved from 6.11.1 to 6.12.0 between the landing
of CUP-1-H and this unit. The new `qmllint` counts differently in every
application, on an untouched main (dd13c81e) as well as on this branch:

| Application | Row before | Row now (measured on this unit's tree, release build of each application) |
|---|---|---|
| siderita | 241 | 222 |
| grafita | 45 | 38 |
| fluorita | 17 | 56 |
| hematita | 0 | 26 |
| magnetita | 4 | 38 |
| cuprita | 0 | 9 |

Magnetita's and Hematita's sources are not touched by this unit, so the
rises are the linter's new rules (for example `Member "length" not found on
type "QStringList"`), not new debt; the rows record the measurement so the
ratchet keeps refusing growth from here. Lowering them again is ordinary
per-application work.

## Landing

- **Base revision:** `dd13c81e3052870cc306f6454f439600e5fe139c`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 0: artifact: celestina-style current; `production_artifact.py check celestina-rs --require-verified` exit 0: artifact: celestina-rs current; `production_artifact.py check siderita --require-verified` exit 0: artifact: siderita current; `production_artifact.py check magnetita --require-verified` exit 0: artifact: magnetita current; `production_artifact.py check magnetita-android --require-verified` exit 0: artifact: magnetita-android current; `production_artifact.py check grafita --require-verified` exit 0: artifact: grafita current; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** fluorita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:86ed5926b5d20047d295d9ff85d899aebe3d6729332d31fc41e7cf509da644d6, verification_fingerprint sha256:50d235306b47413c3180d545eb66bdfe4dda651ad37cebbc0cb0520c603c1e1d; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8eab64cfeb6bfc6975347748fd7e1a13c2cad41d371564a40928e7eda80b7237, verification_fingerprint sha256:d65e1f0f655c98209aae1e17d7b7b22966367c1fbd99fae6fb98ee7ede767a7b; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d18b3735081f33b7384fd3dc5076e0ad7f0a53d7caaa1276d6eaf95e52b1badd, verification_fingerprint sha256:d373f1e5e40451bc3ad75ca9b42956356f2d7da27f55ae3d2d324d5caa83e641
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh
