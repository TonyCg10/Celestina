# Evidence: the recording that never received a frame (SEL-1-E)

- **Date:** 2026-10-10
- **Scope:** SEL-1-E of
  [SEL-1's plan](../plans/archive/2026-10-09-sel-1-foundation.md):
  `selenita/src/record.rs` (the recording worker), STATUS, VALIDATION
- **Environment:** as the
  [SEL-1-D record](2026-10-10-recording-fixes.md): niri 26.04,
  xdg-desktop-portal-wlr 0.8.4, PipeWire 1.6.9, GStreamer 1.28.7,
  radeonsi VA on `/dev/dri/renderD128`
- **Artifact:** the `selenita` release binary in the shared Cargo target,
  not installed

## What happened live

Selenita 1.0.1 (the SEL-1-D landing, `09403a4b`) recorded nothing on the
session where 1.0.0 had recorded twice at 14:55. Four attempts from 15:53:
two with the shipped argv ended with `exited None` (the child killed at
the 10 s stop deadline, nothing saved), two under a logging wrapper for
`gst-launch-1.0` that dropped `-q` and the CQP tokens and ran `GST_DEBUG=3`:
«Estableciendo el conducto a PAUSA …», no buffer, no error, SIGINT, exit
130. `path=57` in every argv. No portal session was opened here; the
findings below come from the diff, the logs and offline runs over
`videotestsrc`, `audiotestsrc` and a PipeWire node fed by a test pattern.

## Findings

1. **The start order did not move.** `git diff 79e62e53 09403a4b` touches
   neither `selenita/src/portal.rs`, nor `celestina-rs/crates/selenita-core/
   src/runner.rs`, nor `tools.rs`; in `selenita/src/record.rs` the SEL-1-D
   change in `Session::start` only swapped the two reports after
   `prepare_and_start` returned (`Started` now before `State(Recording)`),
   and `prepare_and_start` is unchanged: `recorder.prepare()` (the whole
   portal exchange: `CreateSession`, `SelectSources`, `Start` with its
   `streams` answer, `OpenPipeWireRemote`) returns before `recorder.start()`
   spawns the child on `prepared.source.node` with the remote on fd 0.
   `the_child_is_spawned_after_the_portal_answered_and_on_its_node` now pins
   it: the fake records `["prepare", "start"]` in that order, the pipeline's
   source is the prepared node and `path=<node>` is in the argv. The node id
   extraction (`streams` → first `(u, a{sv})` → `u`) is the 1.0.0 code.

2. **The two wrapped runs measured the wrapper, not Selenita.** The wrapper
   ran `/usr/bin/gst-launch-1.0 "${args[@]}" … &` and waited. A bash
   background job in a non-interactive script gets `/dev/null` on fd 0
   (measured: a script started with a socket on fd 0 shows
   `/proc/self/fd/0 -> /dev/null` in its background child and
   `socket:[…]` when run directly). `pipewiresrc fd=0` therefore had no
   remote: it waits for a node that will never appear on `/dev/null`,
   which is «PAUSA …» and nothing more. The CQP bisect inside that wrapper
   is void. A stub or wrapper in `SELENITA_TOOLS_DIR` must `exec` the
   launcher.

3. **The CQP settings are not the cause.** Over `videotestsrc` in BGRx at
   1920×1080/60 (the live format after `pipewiresrc`), `vah264enc
   rate-control=cqp qpi=20 qpp=22 key-int-max=60` prerolls, writes 239
   frames in 4 s and stops in 0.1 s, as the CBR default does; over a real
   PipeWire node (`videotestsrc ! pipewiresink` as a `Video/Source`, read
   with `pipewiresrc path=<node> do-timestamp=true`, memfd buffers) CQP
   writes 183 frames in 4 s and stops in 0.1 s. (`pipewiresink` ends when
   its first reader leaves, so each run needs its own feed; a run against
   a dying feed showed 24 frames, which is the feed, not the encoder.)

4. **The unwrapped failures match a node that exists and never streams.**
   With the feed `SIGSTOP`ped (its node present, no buffer ever), the
   video-only shipped pipeline stays in preroll; SIGINT after 4 s does not
   end it (killed at 12 s, a 0-byte file). In Selenita that is exactly
   `exited None` after «Detener» plus 10 s, the hidden file removed, no
   notice until then. 1.0.0 would behave the same on such a node; what
   made the portal's node stop streaming between 14:55 and 15:53 is not
   known from here (xdpw's instance per output is created anew per
   session — `pw-dump` showed no leftover node and the id 57 recycled to
   the next client — so it is not a stuck instance; niri's capture side,
   or the chooser's choice of output, are the open candidates and need the
   child's own log).

5. **`path=57` four times is id reuse.** PipeWire hands a freed global id
   to the next object: after the session `pw-dump` itself was client 57.
   Not a stale node; the node line on stderr now shows it per recording.

## Change

- `Session::watch_signal` (`selenita/src/record.rs`): while recording, once
  `SIGNAL_DEADLINE` (8 s) has passed since the start and the hidden file is
  still empty (the muxer writes its header with the first frames), the child
  is killed, the file removed and the recording fails with
  `RecordError::NoSignal`, whose `message_es` names the portal as the side
  that sent no picture; the worker polls it every tick beside the exit
  check. A source that streams late within 8 s is unaffected; the fake
  writes at start, so the smoke and the tests never see it.
- `Real::prepare` prints `selenita: recording: portal stream node N, remote
  fd yes|no` on stderr, the one line a live diagnosis needs.
- `Fake` records its calls (`prepare`, `start`, `stop`, `abandon`) and
  exposes `FAKE_NODE`.
- STATUS: the finding, the recipe (`GST_DEBUG=3 GST_DEBUG_FILE=/tmp/
  selenita-gst.log selenita` from a terminal; the child inherits it) and
  the wrapper hazard. VALIDATION: VAL-SEL-REC started that way, the
  no-signal notice as a pass condition.

## Procedure

```sh
cd selenita && cargo fmt --all --check
cd selenita && cargo clippy --all-targets --locked -- -D warnings
cd selenita && cargo test
cd selenita && cargo build --release --locked
cd celestina-rs && cargo test -p selenita-core
sh selenita/scripts/qml-tests.sh          # three consecutive runs
sh selenita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/selenita
bash scripts/qmllint-cxxqt.sh selenita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
python3 scripts/commit_scope.py --check "selenita-maintenance: Fix the recording that never receives a frame and pin the start order"
```

## Result

- **selenita:** 35 tests (33 before): the start order and node, the
  no-signal watchdog (a silent source fails at once with the deadline at
  zero, the states Preparing, Recording, Failed, Idle, no file; the fake
  never trips it; idle has nothing to watch), `NoSignal` among the errors
  with Spanish words.
- **selenita-core:** 46 tests, unchanged.
- **QML:** 46 passed, 0 failed, three runs; nothing in `qml/` changed.
- **Smoke:** both runs as before (`cards=3 history=2 recording=idle
  lastRecording=true …`), the MP4 under `videos/Recordings`.
- **qmllint:** 1 warning, the baseline row. **Guards:** all exit 0.

## Limits

- Nothing here opened a portal session: whether the portal's node streams
  again on the author's session, and why it stopped, is for the live
  re-run with the child's log kept. The watchdog turns the failure into a
  notice within 8 s; it does not make the node stream.
- The 8 s deadline is an estimate: a portal that takes longer than that to
  send its first frame would be cut off.

## Follow-up

- VAL-SEL-REC re-run as its entry now says; if the notice shows, the
  `GST_DEBUG_FILE` log and the `portal stream node` line go to the next
  unit.

## Landing

- **Base revision:** `09403a4bd1bfa4a69a6e27beffbf133e95243565`
- **Check:** `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c6e0c207bd6aa89f72f1e431b47e900c820eea7ab28e610e654b45c908cf195c, verification_fingerprint sha256:b05b24bacf711b3a9e1599a0bafea512e6e44dbf5a9cf95bc539081b3e6e7cc0
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh
