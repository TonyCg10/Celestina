# Evidence: CEL-2-M-LOCK-POLKIT — the lock and the polkit agent on by default

- **Date:** 2026-11-24
- **Scope:** `Duties::default` and `Duties::unclear` in
  `celestina-rs/crates/celestina-shell/src/settings.rs` with their tests and
  the test of the salvaged defaults (`tests/salvage.rs`); the comments that
  stated the old default in `celestina/src/main.rs`,
  `celestina/src/polkit/duty.rs` and `celestina/src/test_support.rs`, with two
  assertions of the quiet fixture; one more duty in the R-10 file check of
  `celestina/tests/lock_scripts.rs`; a comment of `celestina/scripts/smoke.sh`
  and the no-configuration notice of `celestina/scripts/dev-session.sh`;
  [ADR 0026](../decisions/0026-lock-and-polkit-on-by-default.md) and its
  notes in ADRs 0021, 0022 and 0024; the README, the handover, `STATUS.md`,
  `ROADMAP.md` and this unit's ledger row. No dependency changed and no
  behaviour of the lock or of the agent changed.
- **Environment:** CachyOS, worktree `celestina-CEL-2-M-LOCK-POLKIT` cut from
  `main` at `44b8cc2a` (celestina 1.14.0); toolchains 1.97.1 (pinned) and
  1.100.0 (the runner's stable). This host has `/etc/pam.d/celestina-lock`.
  No shell, lock, verifier or `pkexec` ran against the real session: the
  smoke runs inside `dev-session.sh --own-bus` with a system bus that does
  not exist.
- **Artifact:** none sealed by this record; the landing builds its own. The
  smoke ran on the worktree's release build of `celestina`, `celestina-lock`
  and `celestina-lock-verify`, made with the commands of
  `build-production.sh`.

## What the author decided

In the session's chat on 2026-11-24 the author chose: both duties on by
default in one unit; the rows `VAL-CEL2-LOCK-PAM`, `-LIVE`, `-RECOVERY`,
`-SUSPEND` and `VAL-CEL2-POLKIT-LIVE` as the ones he gives as passed (run on
1.13.5 in his own session), with `VAL-CEL2-LOCK-HOTPLUG`, `-NOTICES`,
`-COEXIST`, `-BACKDROP` and `-CLOCK` left `pending` and not gating the
default; and that he writes the row states in `VALIDATION.md` himself. No
agent changed a row state in this unit.

## The change

- `Duties::defaults()`: `lock` and `polkit` are `Duty::On`. Both fields keep
  the `lenient_duty` deserializer, so each is on when its key is absent (the
  struct's default fills the field) and **off** when the key holds a value
  that is neither `"on"` nor `"off"`: they fail closed, like the clipboard.
- `Duties::unclear()`, what a `duties` value that is not an object reads as:
  the defaults with `clipboard`, `lock` and `polkit` off. Before, the last
  two were off there only because the defaults had them off.
- `Duties::all_off()` is unchanged: a protected start (a `shell.json` that
  cannot be read) starts neither duty.
- Nothing in `celestina/src/lock/`, `celestina/src/polkit/`,
  `celestina/src/providers/polkit/` or `celestina/src/energy/` changed. What
  a start with the defaults now does was read from the code, not changed: the
  lock's supervisor pins its binaries and looks for the PAM service once per
  duty start, and with the binaries pinned the idle chain's second step has a
  locker; the agent asks polkitd for the session once, and a refusal (another
  agent holds it) or a bus that cannot be reached is logged once and not
  retried.
- Every script that writes a `shell.json` already named `lock` and `polkit`
  in every file it writes (`smoke.sh` through its `duties` helper and the
  cutover file, `lock-rehearsal.sh`, `wallpaper-backdrop-probe.sh`), so no
  scripted run changed. `dev-session.sh` run by hand with no `shell.json` in
  its configuration home starts the shell on the defaults; its notice said
  "every shipped duty will be on in the nest (the lock excepted)" and now
  names the lock and the polkit agent as included.

## Tests

In `celestina-shell/src/settings.rs`, for both duties: `Settings::default()`
is on; a file with no `duties` key and a `duties` object without the key are
on; `"on"` is on and `"off"` is off; each of twelve unreadable values (one
shared list, `GARBAGE_DUTY_VALUES`, also used by the clipboard's test) is
off and costs the rest of the file nothing; a `duties` value that is not an
object, an array of eleven `"on"` included, leaves the three fail-closed
duties off and the others at their defaults; `all_off()` has both off; the
bytes the store writes for the defaults carry `"lock": "on"` and
`"polkit": "on"` and read back as the defaults; and a file with both written
`"off"` keeps them off through a write for an unrelated change. The tests
whose names stated the old default were renamed. Written first, ten of them
failed against the old `defaults()` and two more against the old
`unclear()`.

## Procedure

```sh
# in celestina-rs/
cargo test -p celestina-shell --locked
cargo fmt --all -- --check
cargo clippy -p celestina-shell --all-targets --locked -- -D warnings
# in celestina/
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings            # 1.97.1
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked --bin celestina
cargo build --release --locked -p celestina-lock -p celestina-lock-verify
cargo build --release --locked --example fake-mpris-player
scripts/smoke.sh --binary <shared target>/release/celestina
# the same smoke with the PAM service hidden from that process tree only
unshare --user --map-user=$(id -u) --map-group=$(id -g) --keep-caps --mount \
    sh -c 'mount --bind <an empty directory> /etc/pam.d && exec scripts/smoke.sh --binary <shared target>/release/celestina'
# at the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

| Command | Exit | Output |
|---|---|---|
| `cargo test -p celestina-shell --locked` | 0 | 538 passed, 0 failed (534 in the library) |
| `cargo fmt --all -- --check` (`celestina-rs/`, `celestina/`) | 0 | clean |
| `cargo clippy -p celestina-shell --all-targets --locked -- -D warnings` | 0 | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` (1.97.1) | 0 | clean |
| `cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings` | 0 | clean |
| `cargo test --workspace --locked` (`celestina/`) | 0 | 45 test binaries, 2272 passed, 0 failed, 2 ignored |
| `scripts/smoke.sh` | 0 | `smoke: OK`, 46 lines |
| `scripts/smoke.sh` with the PAM service hidden | 0 | `smoke: OK`, 46 lines |
| the three guards | 0 | OK |

The cutover run of the smoke starts the shell with every duty on, the lock
and the polkit agent included, over the dead system bus. On this host as it
is: `rest_frames=1, at most 13 over 2 surfaces`, "the PAM service is
installed on this host and the lock did not warn", "polkit on with the agent
unavailable over the dead bus and nothing registered". With `/etc/pam.d`
hidden: `rest_frames=2, at most 15 over 3 surfaces`, "the lock warned once
that the PAM service is missing" (the third surface is the notice's banner,
and the budget's two frames for it were enough). That second run is the
first time the missing-service branch of the cutover run ran on this host;
the mount is private to the smoke's own process tree and nothing under
`/etc` was changed.

The GitHub `celestina` workflow runs `cargo fmt`, Clippy and the tests on
the runner's stable and not the smoke; none of those reads
`/etc/pam.d/celestina-lock`, and no test builds the production wiring of the
lock or of the agent (only `main.rs` does).

## Review

An independent review (Opus, a model other than the implementers', read
only, high reasoning) returned "no critical findings": the code does the
decisions of ADR 0026 and nothing else, no path of the file turns either
duty on against an `"off"` or an unreadable value, no test was weakened,
`VALIDATION.md` is untouched and the documents claim exactly the five
reported rows. Fixed in this unit after it: the lock-out on a machine
without the PAM service was documented through the idle chain only, while
any sleep logind announces starts the lock too, and a comment of `main.rs`
said the same wrong thing (I-1); nothing said that a session that chose
nothing now locks and suspends after 240 s of idle (I-2); polkitd keeps the
first agent, so an agent that starts after the shell is the one refused
(M-2); the ADR's header now names ADRs 0021 and 0022 (M-4); the ledger
row's areas (M-5); the README says what a nest started by hand reaches on
the host's system bus (M-6); the ADR says that only a start is protected and
what a missing file or no configuration home reads as (M-8). Left as they
are: the stale CEL-2-M paragraphs (M-3, below), the older behaviours the
default makes reachable (M-7, below) and a cosmetic assertion of
`tests/duty_files.rs` that names `clipboard` and `lock` but not `polkit`
while taking its key list from `Duties::default()` (M-9).

## Limits

- The author's passed rows are his report in the session's chat; this record
  holds no log of his session, and the states in `VALIDATION.md` are his to
  write.
- No default start ran against a real system bus: that the agent registers
  for the session, or is refused beside another agent, at a start with no
  `shell.json`, was read from the code and is what `VAL-CEL2-POLKIT-LIVE`
  showed with the key written `"on"`, the same path.
- A machine without the PAM service now warns at every start, and its idle
  chain would lock a session the lock cannot unlock (the lock design's
  decision 6, unchanged; ADR 0026, Consequences). Only the warning and its
  frame budget were exercised, in the nest.
- The missing-service notice is a `Notify` call made while the shell's own
  notification server is still claiming its name; the server normally wins
  and did in the run above. Which of the two wins was not forced either way.
  The code is older than this unit; the default makes it reachable on a
  fresh install without the PAM service.
- `dev-session.sh` by hand sets no `DBUS_SYSTEM_BUS_ADDRESS`: a nest started
  with no `shell.json` in its configuration home now starts both duties
  against the host's system bus. The interactive nest's notice and the
  README say so; `--once` by hand prints no notice, and the script was not
  otherwise changed.
- Other behaviours older than this unit that the default now reaches without
  a hand-written `"on"`: with no reachable system bus the lock's sleep worker
  logs `lock: no PrepareForSleep subscription` at `warn` every five seconds;
  a start with the lock on logs its advice about another locker's idle lock
  at `info`; and a session that also runs another locker before sleep has
  two lockers asking at every suspend, one of which the protocol refuses
  (`VAL-CEL2-LOCK-COEXIST`, pending).
- `STATUS.md` and `ROADMAP.md` still call CEL-2-M "implemented on its branch,
  not landed" although it landed as 1.14.0; that paragraph is older than this
  unit and was left as it is.

## Follow-up

The five lock rows left `pending` in `VALIDATION.md`; a failure in one opens
a corrective unit.

## Landing

- **Base revision:** `44b8cc2ad45d18264a15d91ede9d56144f2260a1`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:567037e0208475a3d04b532ae21faa728c07a890eb72209d21b40e779a2aaaaf, verification_fingerprint sha256:c361b35464cd3e8e6c7bc69240fc54bdd0c55671a3f70e7045594c166012a742; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d2a39816b4d11a6d210642dd2ac10e7a1c8a67da21bc79dda187034e72733705, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
