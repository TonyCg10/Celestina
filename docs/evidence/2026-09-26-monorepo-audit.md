# Evidence: monorepo audit and hardening program

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): the whole monorepo on `main` at `9d022dd`, clean
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

## Procedure

Seven auditors each took one area from a common brief. The brief was
read-only: no tracked file edited, nothing committed, no production entry
(`build-production.sh`, `complete-production.sh`, `cargo build --release`,
CMake) and no subagent. Each auditor first ran
`python3 scripts/agent-context.py <area>` and read what it printed, then
audited against the standards in that reading order, in this order of
importance: correctness and unsafe behaviour, security, performance,
architecture and reuse, tests, QML and accessibility, documentation truth,
and quick wins. A finding had to be verified by reading the code at a cited
`path:line`; each carries an ID, severity, category, evidence, the reason it
matters, a fix, an effort (S under 1 hour, M under 1 day, L over 1 day) and
an owning prefix. The area records keep every finding unchanged:

- [shared Rust workspace (RS)](2026-09-26-monorepo-audit-shared-crates.md)
- [Siderita (SID)](2026-09-26-monorepo-audit-siderita.md)
- [Hematita and Grafita (HEM, GRA)](2026-09-26-monorepo-audit-hematita-grafita.md)
- [Fluorita (FLU)](2026-09-26-monorepo-audit-fluorita.md)
- [Magnetita and Magnetita Android (MAG, AND)](2026-09-26-monorepo-audit-magnetita.md)
- [shell and shared style (SH, STY)](2026-09-26-monorepo-audit-shell-style.md)
- [repository tooling and governance (TOOL)](2026-09-26-monorepo-audit-tooling.md)

Together the auditors ran:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
python3 scripts/audit-version-commits.py
cargo test -p celestina-core -p celestina-dotfiles-core -p celestina-shell-core --offline --locked
cargo test -p siderita-ops -p siderita-core -p siderita-archive --offline
cargo test -p hematita-core --offline
cargo test -p grafita-core --offline --no-fail-fast
cargo test -p fluorita-core --offline
cargo test -p fluorita-engine --offline
cargo test -p magnetita-proto -p magnetita-link -p magnetita-mobile -p magnetita-net -p magnetita-core --offline
cargo test -p magnetitad --offline
cargo test --manifest-path celestina/Cargo.toml --offline
cargo clippy --all-targets --offline --locked
cargo tree -p magnetitad -e features -i zbus
cargo tree -p magnetita-mobile -e normal
```

plus every `scripts/test-*` fixture suite the tooling record lists with its
timing. Scratch probes ran outside the repository: three Siderita proofs of
concept (SID-1, SID-2, SID-4), a `grafita-core` probe binary (GRA-1, GRA-2),
and a throwaway clone with a scratch bare `origin` for the hook and landing
demonstrations (TOOL-5, TOOL-6, one documentation-only landing), deleted
afterwards.

The seven reports were then consolidated without re-auditing code: duplicates
with the same whole root cause were merged, the Critical and Important
findings ranked, and the findings grouped into single-prefix delivery units.
The only file opened outside the reports and the governance documents was
`docs/projects.toml`, to settle the production-input disagreement (question 1
in the follow-up). After the author asked for the whole program, rulings
R-A1 to R-A8 settled its open questions and were applied to the program;
each is named where it applies.

## Result

- **Exit:** the architecture, language and documentation guards and
  `version_tool.py check` exited 0; `scripts/test-architecture-scanners.sh`
  and `scripts/test-version-contract.py` failed (TOOL-1); every crate test
  run passed except six `hematita-core` `usage_tree` tests and one
  `grafita-core` test that fail only as root (HEM-15, GRA-7);
  `fluorita-engine` did not link (no `libmpv`) and the shell helper crate did
  not resolve `niri-ipc` offline.
- **Observed:** 184 findings after de-duplication, 6 Critical, 78 Important
  and 100 Minor, from 191 raw findings (6 / 82 / 103). The five to fix first
  are pipeline trust (TOOL-1 with TOOL-3/TOOL-4, MAG-9, FLU-21), SID-1,
  AND-1, GRA-1 with GRA-2, and FLU-1. The numbered sections below keep the
  consolidation's own numbering, which its text cross-references; section 5
  is under Limits and section 6 under Follow-up.

### 1. Executive summary

- **Overall health.** The pure Rust layer is strong. `unsafe` is forbidden across the workspace, errors are typed, and the domain tests the auditors ran all pass: shell-core 349, fluorita-core 158, Siderita crates 124, hematita-core 105, magnetitad 81, magnetita-proto 66, celestina-core 34, plus the link, mobile, net and grafita-core suites. The only failures were root-only tests. All three repository guards pass. The lock, polkit and Hematita delete boundaries are carefully built.
- **Where it breaks.** The problems sit at the edges: hostile input from files and the network, the "never lose the source" promise, blocking IO on the Qt thread in every app, and a delivery pipeline that no longer enforces anything.
- **Totals after de-duplication.** 184 findings: **6 Critical, 78 Important, 100 Minor**. The raw count was 191 (6 / 82 / 103). Merging removed 4 Important and 3 Minor; see section 2.
- **Fix these five first:**
  1. **Pipeline trust (TOOL-1 with TOOL-3/TOOL-4, MAG-9, FLU-21).** CI on `main` has been red since 2026-09-25, and every later CI step is skipped. The artifact fingerprints also omit crates the apps really link. Until both are fixed, no check is enforced and a fix to a shared crate may never reach the installed binary.
  2. **SID-1.** A crafted archive escapes the extraction root through chained symlinks (reproduced). One opened download can overwrite `~/.bashrc` or autostart entries.
  3. **AND-1.** An exported `magnetita://pair` deep link pairs the phone with no confirmation. Any app or web link can pin an attacker's desktop, which then receives SMS, 2FA notifications, contacts and files.
  4. **GRA-1 and GRA-2.** A 20 KB PDF aborts the process (stack overflow and slice panics under `panic = "abort"`), and a 1 MiB gzip inflates to 1 GiB. Both run in-process in Grafita and in Siderita's Space preview.
  5. **FLU-1.** A same-format "Replace" overwrites the original instead of trashing it, for edits, metadata writes and batches. This breaks ADR 0009 and loses the original for good.

### 2. Cross-area duplicates merged

**Counting rule.** A finding counts as merged only when its whole root cause is the same as another's. Entries marked "no count change" group related findings under one owner for delivery, but those findings stay separate defects.

| Root cause | Original IDs | Single owner | Delivered by | Count effect |
|---|---|---|---|---|
| `file://` URI to local path is parsed about 7 times with different rules. The shell cover check never percent-decodes. Siderita accepts foreign hosts. Fluorita's two decoders disagree on `localhost`. Grafita refuses non-UTF-8 names. | RS-1, SID-18, FLU-23 | `celestina-core` (lift magnetitad's strict `path_for_file_uri`) | P-6 owner; adoption in P-14 (fluorita), P-15 (siderita), P-17 (celestina), P-19 (grafita) | −2 Minor |
| `atomic_file::replace` has no mode. State files come out 0644 in 0755 directories (clipboard history). User media loses its mode, owner and xattrs. A "Copy" can clobber a name created in the meantime. `magnetita-net` forked the recipe as `write_private`. | RS-2, FLU-2 (related: RS-9, MAG-18) | `celestina-core::atomic_file`, with two explicit entry points: private state (0600/0700) and media landing (keep the source mode, no-replace for copies) | P-6 owner; adoption in P-7, P-10, P-11 | −1 Important |
| No owner for `XDG_RUNTIME_DIR`. There are 7 ad-hoc lookups, and some fall back to world-writable `/tmp` (FUSE mounts, FIFOs, DDC lock, the shell and lock style symlink). | RS-3, SH-2 (the `/tmp` half only) | `celestina-core::xdg::runtime_dir` (absolute, no `/tmp` fallback). The C++ lock mirrors the same rule. | P-6; P-10 (celestina); P-11 (magnetita) | no count change (SH-2 also carries the QML-callable unlock signal) |
| The `.desktop` scan is copied 3 times (Siderita, shell launcher, Hematita) with different shadowing rules. Reads are unbounded, and two run on the Qt thread. Spec gaps in the parser. | RS-4, RS-10 | `celestina-core::desktop_entry::{read, scan}` | P-6; adoption in P-15, P-17, P-18 | no count change |
| The playback session and surface handshake has two owners. Siderita's copy has the wrong close order and no generation guard, so embedded playback stays dead until restart. Fluorita's own close never bumps the generation. | FLU-12, FLU-13, FLU-29 | `fluorita-core` (state machine) and `fluorita-engine` (session loop) | P-14 extracts; P-15 deletes Siderita's copy | −1 Important |
| Siderita embeds other products' cores in-process under `panic = "abort"`. A Grafita importer crash or bomb, blocking recent-list IO, or a Fluorita duration panic takes down the file manager. | GRA-1, GRA-2, GRA-5, FLU-3 | Each core's owning product (grafita, fluorita). Siderita is redeployed through its `production_inputs`, which already list grafita-core and the fluorita crates. | P-5, P-7, P-19 | no count change; see the bump question in section 6 |
| `production_inputs` are hand-listed and omit linked Cargo path dependencies. Magnetita Android declares none. Agent context and the toolchain probe share the blind spot. | TOOL-3, TOOL-4, MAG-9, FLU-21 (related: TOOL-21, TOOL-22) | `suite` registry and guard, derived from `cargo metadata` | P-2 | −2 Important, −1 Minor |
| Hematita was never added to hand-maintained lists: the style guard roots, the version-owner fixture, and the root docs and counts. | TOOL-1, TOOL-2, TOOL-15 | `suite`: derive every list from `docs/projects.toml` | P-1, P-20 | no count change |
| No bounded reader for small state files. The bound is checked after a full read, or a TOCTOU window exists. | RS-6, SH-14 (settings half), FLU-4 (catalogue overwritten after a failed load) | `celestina-core::atomic_file::read_bounded` | P-6; P-7, P-17 | no count change |
| Phone protocol rules have two owners: the clipboard bound (64 vs 256 KiB), wire ids, the path rule, backoff, discovery, and the received-file name sanitiser. | AND-5, MAG-19, AND-9 (sanitiser half) | `magnetita-proto` / `magnetita-mobile`, exported through UniFFI | P-13 | no count change |
| Inert or cosmetic `#[allow]` in `nightlight.rs`. | RS-18, SH-21 (third bullet) | `celestina-shell-core` (inside `celestina` commit roots) | P-17 | no count change (SH-21 has other parts) |
| Related, deliberately not merged: current-uid resolution done ad hoc (SID-16 falls back to uid 0 in Rust; SH-12 reads `$USER` in C++). Stale STATUS "current truth" in every project (SID-30, HEM-13, GRA-9, FLU-25, MAG-24, SH-8, RS-19, TOOL-15). | — | per project | each product's unit | — |

### 3. Critical and Important findings, ranked

Ranking order: data loss and security, then crashes, then correctness, then performance, then architecture, accessibility and docs. Within each group, Critical comes first. Effort: S under 1 hour, M under 1 day, L over 1 day. The "Owner" column gives the base prefix; commit kinds are in section 4.

| # | IDs | Sev | One-line summary | User-visible impact | Eff | Owner | Area |
|---|---|---|---|---|---|---|---|
| 1 | SID-1 | Critical | Chained symlinks escape the archive extraction root; the text-only link check (reproduced) | Opening a downloaded archive writes anywhere in `$HOME` (code runs at next login) | M | siderita | SID |
| 2 | AND-1 | Critical | Exported BROWSABLE `magnetita://pair` pairs with no confirmation, to any address | Any app or link pins an attacker desktop that then gets SMS, 2FA, contacts and files | S | magnetita-android | MAG |
| 3 | FLU-1 | Critical | Same-format Replace renames over the original; the Trash is never used (edit, metadata, batch) | Originals destroyed without recovery, contrary to ADR 0009 | M | fluorita | FLU |
| 4 | SID-2 | Important | Copy rollback deletes a destination a racing writer created (160 of 200 runs) | Copy reports success and the file is gone | S | siderita | SID |
| 5 | SID-3 | Important | Cross-device move removes the source with no fsync; `remove_dir_all` also deletes entries added during the copy | Unplug or power loss, or an active download, loses data | M | siderita | SID |
| 6 | HEM-3 | Important | Mount boundaries matched by lexical path against a non-canonical root | Permanent delete can go through a bind mount and remove files elsewhere | M | hematita | HG |
| 7 | AND-4 | Important | SAF `deleteDocument` is recursive; FUSE `rmdir` maps to it | "Remove empty folders" wipes a phone folder's contents | S | magnetita-android (+magnetita) | MAG |
| 8 | SID-5 | Important | Write workers are detached; quit kills them mid-write | Truncated file left under its final name | M | siderita | SID |
| 9 | SID-4 | Important | Restore ignores relative `Path=` records (reproduced) | Entry restored relative to the CWD; the record is deleted | S | siderita | SID |
| 10 | FLU-4 | Important | Probed tags unbounded; an oversized catalogue fails to load and is overwritten | All learned tags lost at each launch | S | fluorita | FLU |
| 11 | AND-6 | Important | `MirrorKey`/`MirrorGlobal` accepted with no mirror session | A paired desktop types into phone fields silently | S | magnetita-android | MAG |
| 12 | MAG-1 | Important | Session `device_id` comes from the peer's hello, not from the pinned fingerprint | A pinned peer hijacks another device's session or pin | S | magnetita | MAG |
| 13 | MAG-3 | Important | Any unpinned connection burns the one-time QR secret before a proof | Pairing fails; trivially disrupted | S | magnetita | MAG |
| 14 | MAG-2 | Important | TLS handshakes serialised in the accept loop (10 s each) | One LAN host locks the phone out (DoS) | M | magnetita | MAG |
| 15 | SH-2 | Important | Shared style symlink with a `/tmp` fallback; QML can emit the lock's unlock verdict | Local user could inject QML into the lock and unlock it; lock can fail to start | M | celestina | SH |
| 16 | SH-1 | Important | `lock` verb reports `confirmed` before the compositor covers | Script or person believes the session is locked when it is not | S | celestina | SH |
| 17 | SH-3 | Important | Polkit message and PAM text rendered as `AutoText` | pkexec caller restyles the auth dialog and triggers remote image fetches | S | celestina | SH |
| 18 | RS-2, FLU-2 | Important | `atomic_file` has no mode or no-replace | Clipboard history and privately-moded photos readable by other users; Copy clobbers | S–M | celestina-rs → celestina, fluorita, magnetita | RS, FLU |
| 19 | RS-3 | Important | No `XDG_RUNTIME_DIR` owner; `/tmp` fallback for FUSE mounts, FIFOs, locks | Pre-created `/tmp` paths capture mounts and FIFOs | S | celestina-rs → magnetita, celestina | RS |
| 20 | FLU-15 | Important | "Remove location" strips EXIF only; XMP GPS and MPF survive | Photo still leaks the address | M | fluorita | FLU |
| 21 | FLU-16 | Important | Pixelate and blur redaction reversible for text | Redacted text recoverable | S | fluorita | FLU |
| 22 | GRA-1 | Critical | PDF lexer: unbounded recursion and unchecked slices under `panic = "abort"` (reproduced) | 20 KB PDF crashes Grafita, and Siderita on Space | M | grafita | HG |
| 23 | GRA-2 | Critical | Decompression bombs (gzip, ZIP, PDF Flate); the limit applies only to the raw file (reproduced) | Out of memory in Grafita and Siderita | S | grafita | HG |
| 24 | FLU-3 | Important | `Duration::from_secs_f64` on file-controlled doubles | Crafted `.mkv` aborts Fluorita and Siderita's player | S | fluorita | FLU |
| 25 | FLU-20 | Important | Renderer and mpv callback use the raw item pointer outside `synchronize` | Use-after-free at teardown or delegate recycling | M | fluorita (fluorita-qt) | FLU |
| 26 | GRA-3 | Important | Object streams re-inflated per lookup; `/N` sizes a `Vec` | Slow PDF opens; capacity-overflow abort | S | grafita | HG |
| 27 | TOOL-1 | Critical | CI `contracts` red on `main` since 2026-09-25; every later step skipped | Nothing backstops commits that skipped the hooks | S | suite | TOOL |
| 28 | TOOL-3, TOOL-4, MAG-9, FLU-21 | Important | Fingerprints omit linked crates; Android declares no inputs | A fix to e.g. `magnetita-link` or `siderita-ops` never reaches the installed app, while `status` says current | M | suite | TOOL, MAG, FLU |
| 29 | TOOL-8 | Important | Landing deploys before its final guards, commit and push | A failed landing leaves bytes installed that match no revision | M | suite | TOOL |
| 30 | TOOL-5 | Important | Partial staging and automatic merges bypass the documentation contract (demonstrated) | Doc-red revisions reach `main` | M | suite | TOOL |
| 31 | TOOL-6 | Important | Hooks execute the worktree copies of the guards (demonstrated) | An unstaged edit disables all commit enforcement | M | suite | TOOL |
| 32 | MAG-4 | Important | Phone `next()` cancels a read that is not cancel-safe | Control stream desyncs and the session drops | S | magnetita | MAG |
| 33 | MAG-5 | Important | Session loop awaits sends with no deadline; Forget is checked in the same loop | Forget fails; outbox grows without bound | M | magnetita | MAG |
| 34 | AND-2 | Important | File upload runs inline in the phone receive loop | FUSE returns EIO and ring or mirror stop is ignored during uploads | S | magnetita-android | MAG |
| 35 | MAG-6 | Important | Held keys and buttons never released at session end; the governor drops releases | Ctrl or left button stuck on the author's desktop | S | magnetita | MAG |
| 36 | MAG-7 | Important | Mirror FIFO queue unbounded | Daemon memory grows about 1 MB/s | S | magnetita | MAG |
| 37 | MAG-14 | Important | Blocking `wl-copy` and Notify on the 2-worker async runtime | All sessions and timers stall | S | magnetita | MAG |
| 38 | SID-6 | Important | A job never ends if its tab closes | Ring spins forever; refresh off everywhere | S | siderita | SID |
| 39 | SID-7 | Important | Any running job silences folder changes in every tab, never replayed | Tabs stay stale | S | siderita | SID |
| 40 | SID-8 | Important | Folder-change refresh cancels a navigation in flight | The click is silently undone | S | siderita | SID |
| 41 | SID-9 | Important | A cancelled search still publishes | Results reappear after closing | S | siderita | SID |
| 42 | FLU-12, FLU-13 | Important | Handshake duplicated; Siderita order wrong and no generation guard | Siderita embedded playback dead until restart; stale handle | S+M | fluorita → siderita | FLU |
| 43 | SID-10 | Important | Undo, restore, purge and empty Trash run on the Qt thread | App freezes during GB-sized copies | M | siderita | SID |
| 44 | SID-11 | Important | Blocking UDisks and Magnetita D-Bus on the Qt thread, per tab | Freezes of up to 25 s | M | siderita | SID |
| 45 | FLU-5 | Important | Watch resync scan uses a fresh token and is joined on the GUI thread | GUI frozen for up to 120 s | S | fluorita | FLU |
| 46 | FLU-6 | Important | Cancel lost between `submit` and the token swap | GUI join waits for a full job | S | fluorita | FLU |
| 47 | FLU-14 | Important | Portal waits joined on the GUI thread; listener thread leaks | Window close hangs for up to 300 s | M | fluorita | FLU |
| 48 | FLU-19 | Important | Still probe (`stat` plus header read) on the GUI thread | Hangs on network roots | M | fluorita | FLU |
| 49 | FLU-8 | Important | In-place replace leaves a stale duplicate and an untagged new record | Music shows two rows after a tag fix | M | fluorita | FLU |
| 50 | FLU-9 | Important | Directory moves invisible to the watch | Moved folders missing or phantom | S | fluorita | FLU |
| 51 | FLU-10 | Important | Truncation is global and permanent; forgetting disabled | Deleted files keep appearing | M | fluorita | FLU |
| 52 | FLU-11 | Important | Edit recipes are write-only; store unbounded and silently reset | "A copy stays reopenable" is false | M | fluorita | FLU |
| 53 | HEM-5 | Important | `ACTION_TIMEOUT` is inert; the comment claims a bound | Thread and connection parked until exit | S | hematita | HG |
| 54 | GRA-5 | Important | Recent list read plus 2 fsyncs on the GUI thread (Grafita and Siderita) | Stalls on a busy disk; hangs on dead mounts | M | grafita | HG |
| 55 | SH-5 | Important | Blocking D-Bus and process waits on the shell GUI thread | Panels stall up to 5 s at resume | M | celestina | SH |
| 56 | SH-7 | Important | Clipboard re-offer does a blocking write inside Wayland dispatch | Clipboard dead for the session | S | celestina | SH |
| 57 | RS-1, SID-18, FLU-23 | Important | `file://` parsed 7 times with divergent rules | Covers with spaces missing; validation bypassed; foreign hosts treated as local | M | celestina-rs → consumers | RS, SID, FLU |
| 58 | RS-4 | Important | `.desktop` scan copied 3 times, unbounded, partly on the Qt thread | A FIFO named `x.desktop` freezes Siderita | M | celestina-rs → siderita, celestina, hematita | RS |
| 59 | TOOL-10 | Important | `qmllint-cxxqt.sh` picks the newest module in the shared target | QML linted against another app's types | S | suite | TOOL |
| 60 | TOOL-11 | Important | Language exemption markers honoured in any file type | Ratchet emptied without translating | S | suite | TOOL |
| 61 | SID-12 | Important | PE icon reader reads the whole `.exe` | Multi-GB reads per thumbnail, OOM risk | S | siderita | SID |
| 62 | HEM-4 | Important | Scan arena has no entry cap | About 1.5 GB for `/` | L | hematita | HG |
| 63 | HEM-2 | Important | Verdict storm: O(tree) work plus a full republish per group | Window stalls during verification | M | hematita | HG |
| 64 | HEM-1 | Important | Storage rows unbounded; duplicates the core `children_rows` | 100k-row lists rebuilt in JS | M | hematita | HG |
| 65 | GRA-4 | Important | Quadratic UTF-8 to UTF-16 conversion per highlight run | Freeze on minified JS or JSON | M | grafita | HG |
| 66 | FLU-7 | Important | GUI-thread projection stats every item and deep-clones the catalogue | About 55k stats per sidebar click | M | fluorita | FLU |
| 67 | MAG-12 | Important | Messages page polls every 3 s, each poll a full phone resync | Battery and CPU drain; D-Bus churn | S | magnetita | MAG |
| 68 | AND-3 | Important | Conversation list scans every SMS row | Seconds per request | S | magnetita-android | MAG |
| 69 | MAG-11 | Important | `avahi-browse` about 1.4 times per second forever; dead dial loop | Process churn; shutdown can hang 60 s | S | magnetita | MAG |
| 70 | MAG-13 | Important | New session-bus connection per mirror touch; moves never merged | Laggy mirror input | S | magnetita | MAG |
| 71 | MAG-20 | Important | 1 s QUIC keep-alive for the life of the session | Phone battery drain (unmeasured) | S | magnetita | MAG |
| 72 | TOOL-7 | Important | Documentation guard takes 36 s re-verifying immutable inventories | 38 s per commit | M | suite | TOOL |
| 73 | TOOL-9 | Important | Any shared-input change re-verifies 9 projects and redeploys 6 | A ratchet decrease needs every toolchain, including the Android SDK | M | suite | TOOL |
| 74 | SH-4 | Important | Lock never binds `reducedMotion` | Full-screen motion regardless of preference | S | celestina | SH |
| 75 | SH-6 | Important | Lock date and error text about 1.3:1 over a bright wallpaper; contract blind | Unlock error unreadable | M | celestina-style → celestina | SH |
| 76 | FLU-17 | Important | Annotations selectable only by pointer; no accessible role | Keyboard and AT users cannot edit marks | M | fluorita | FLU |
| 77 | MAG-21 | Important | `ConversationRow` is a bare `MouseArea` | No keyboard or AT path to a conversation | S | magnetita | MAG |
| 78 | TOOL-2 | Important | Style guard never scans `hematita/qml` | Two token violations hidden | S | suite (+hematita) | TOOL |
| 79 | MAG-8 | Important | Capability negotiation never performed | Protocol evolution story false | M | magnetita (+android) | MAG |
| 80 | AND-5 | Important | Kotlin re-implements rules `magnetita-mobile` owns | Already diverged (clipboard bound) | M | magnetita-android (+magnetita) | MAG |
| 81 | FLU-18 | Important | Unused libx264 encode path contradicts ADR 0009 | Extra hostile-input and CPU surface | M | fluorita | FLU |
| 82 | RS-5 | Important | magnetitad runs two async executors without the required approval | Supply-chain and runtime surface | S | magnetita | RS |
| 83 | MAG-10 | Important | `verify-production.sh` skips proto, link, mobile and peer | Wire golden vectors not gated | S | magnetita | MAG |
| 84 | SH-8 | Important | README and STATUS say lock and polkit are refused or unbuilt; checkpoint contradicts ROADMAP | Author misconfigures bindings; agents misread state | S | celestina | SH |

#### Minor findings by area

Each Minor is listed with the unit that carries it; "backlog" means not scheduled.

- **RS (14).**
  - RS-6 bounded state reads (P-6/P-17), RS-7 blocking `is_cancelled` (P-6, additive API), RS-9 `Err` after a committed rename (P-6), RS-10 `desktop_entry` spec gaps and a vacuous test (P-6).
  - RS-13 `uniffi/cli` in the library (P-12), RS-18 inert `#[allow]` (P-17), RS-19 docs (P-6).
  - Backlog: RS-8 `pathkey::decode` accepts non-canonical keys (a behaviour change for every consumer, so land it with one), RS-11 `dotfiles-core` has no consumer (author decision), RS-12 three error conventions, RS-14 false MSRV 1.85, RS-15 uncentralised dependencies, RS-16 three text-bounding helpers, RS-17 journal tail read and untested C++ twin.
- **SID (19).**
  - SID-13 small Qt-thread syscalls, SID-19 portal filter delimiters, SID-24 Trash/Recientes without generation, SID-25 scan executor `expect` and stuck worker, SID-26 unbounded thumbnail pool, SID-27 dialogs without role, SID-28 reduced motion, SID-30 docs: all P-15.
  - SID-14 trash/restore bypass `reserve`, SID-15 Replace order, SID-16 volume Trash trust, SID-20 UTC `DeletionDate`, SID-31 dead code: all P-8.
  - SID-17 password on argv: P-4. SID-18 merged into RS-1. SID-29 tests ride with P-4, P-8 and P-15.
  - Backlog: SID-21 three unsafe `localtime_r`, SID-22 calendar and passwd duplicates, SID-23 file-domain logic in the controller (L).
- **HG (15).**
  - HEM-8 duplicates keyed by blocks, HEM-9 blocking FIFO content check, HEM-10 quadratic prune, HEM-15 root-unsafe tests: P-9.
  - HEM-6 D-Bus error typing, HEM-7 stale per-PID facts, HEM-11 unbounded cmdline, HEM-12 `stop()` join, HEM-13 docs, HEM-14 byte formatter ×4: P-18.
  - GRA-7 hostile-input tests: P-5. GRA-8 raw `QQuickTextDocument*`, GRA-9 STATUS: P-19.
  - Backlog: HEM-16 single-instance hand-off copied 3× (`celestina-core`), GRA-6 whole-document round trip per keystroke (L).
- **FLU (10).**
  - FLU-21 merged into P-2, FLU-23 merged into RS-1.
  - FLU-24 MPRIS rate, FLU-25 docs, FLU-26 thumbnail spec keys, FLU-27 dotted-ancestor watch, FLU-28 aborting spawns and uncancellable frame job, FLU-29 tautological tests, FLU-30 `#[allow]`: P-14.
  - Backlog: FLU-22 first-run seed guesses XDG dirs (lift `user-dirs.dirs` into `celestina-core`).
- **MAG (17).**
  - MAG-15 Forget blocks the zbus executor, MAG-17 FUSE caches and `create` truncation, MAG-22 wholesale models, MAG-24 docs, MAG-25 fixes landed as `maintenance`, MAG-26 KDE leftovers, MAG-27 no SIGTERM, MAG-30 cleanups: P-12.
  - MAG-16 share caps and sweep, MAG-18 non-atomic `commands.json`, MAG-29 unescaped Notify body: P-11.
  - MAG-19 clipboard bound, MAG-28 untested FFI, AND-7 polling and unbounded input queue, AND-9 received-file caps: P-13. AND-8 template backup rules: P-3.
  - Backlog: MAG-23 second niri IPC client.
- **SH (17).**
  - SH-11 PAM expired and echo-on handling, SH-12 `$USER`, SH-13 lock sequencing: P-10.
  - SH-9 polkit retry and queue, SH-10 double verdicts and lost connections, SH-14 unbounded niri stream and settings, SH-15 polkit buffer, SH-17 seven reduced-motion readers, SH-19 lock and prompt names and alerts, SH-20 clock ticks every second, SH-21 production `expect`: P-17.
  - STY-1 English accessible names, STY-2 swatch literals and guard gap, STY-3 untested controls: P-16. STY-4 Siderita slider copy: P-15.
  - Backlog: SH-16 drifted backdrop field copy, SH-18 per-app reaper thread, `kitty`, `LD_LIBRARY_PATH`.
- **TOOL (11).**
  - TOOL-13 CI coverage gaps: P-1. TOOL-21 agent context, TOOL-22 toolchain drift: P-2.
  - TOOL-12 doubled landing tests, TOOL-14 errata scope, TOOL-15 stale root docs, TOOL-16 versioning "Agent workflow" contradicts the landing, TOOL-17 unregistered `docs/superpowers/`, TOOL-18 duplicated helpers, TOOL-19 landing robustness, TOOL-20 full-history version audit: P-20.

### 4. Delivery program

**Execution order.** P-1 and P-2 make the pipeline trustworthy, and P-0b lets a dependent unit be prepared on a branch stacked on its dependency before that dependency lands (ruling R-A1). P-3 to P-11 fix security and data loss. P-12 to P-15 fix correctness. P-16 to P-19 cover accessibility, performance and remaining architecture. P-20 fixes tooling integrity and governance documents. Each unit is one commit under one prefix. Only documentation-only suite units can land from the audit container, which has no Qt, libmpv or Android SDK; every other unit is prepared on its own `unit/<project>/<unit>` branch, stacked on its dependency's branch, and landed by the author with `scripts/land-unit.py` in program order (ruling R-A7).

**Commit kinds.**
- A product `bug` bumps that product's PATCH through `land-unit.py`, and needs the product's `complete-production.sh` (or build plus verify for Android and celestina-style) on the author's machine.
- The landing also rebuilds every project whose `production_inputs` the unit touched. After P-2 that set is much larger.
- A fix to a shared crate bumps only the owning product (`<owner>-bug`); the landing rebuilds and redeploys the affected consumers with their versions unchanged (ruling R-A2, settling question 7 of section 6).
- Component work closed by a ledger uses the owning product's primary prefix. Component prefixes such as `siderita-ops:` or `grafita-core:` cannot carry plans, evidence or inventories.
- Where an area report proposed a component prefix or `suite:` for `celestina-core` work, the program substitutes `celestina-rs-maintenance` (dormant owner) followed by product `bug` adoptions.

| ID | Unit | Prefix and kind | Intended change | Closes | Eff | Depends on | Automated exit |
|---|---|---|---|---|---|---|---|
| P-1 | `AUD-1-B` | `suite-maintenance` | Derive the style-guard roots and the version-owner fixture from `docs/projects.toml`. Fix the two Hematita token violations in the same commit, because a guard coverage change and the violations it reveals must land together or the published revision is red. Add the three missing tests to `contracts.yml`. If the Hematita token fix changes what a person sees, the implementer reports it and the unit lands as `suite-bug` bumping Hematita (ruling R-A6). | TOOL-1, TOOL-2, TOOL-13 | S | — | `bash scripts/test-architecture-scanners.sh`, `python3 scripts/test-version-contract.py`, `bash scripts/check-architecture-contract.sh` (style guard now covers `hematita/qml`), `python3 scripts/test-language-contract.py`, `sh scripts/test-production-artifacts.sh`, `sh scripts/test-production-common.sh`; GitHub `contracts` green on the landed commit |
| P-0b | `AUD-1-C` | `suite-maintenance` | Let `scripts/land-unit.py` accept a stacked branch: a ledger row the branch carries that equals `origin/main`'s version of that row is accepted instead of stopping the plan merge, and another unit's evidence file in an add/add conflict takes `main`'s copy. Add fixture tests for both cases. | none (ruling R-A1) | S–M | — | `bash scripts/test-land-unit.sh`, `python3 scripts/test-land-unit.py` with a stacked-branch fixture whose dependency already landed; `sh scripts/check-documentation-contract.sh` |
| P-2 | `AUD-1-D` | `suite-maintenance` | Fingerprint each app's `cargo metadata` path-package closure, or guard that `production_inputs` contains it. Declare Magnetita Android's inputs. Reject buildable projects with empty inputs. Compare the recorded toolchain. Print consumers in agent context. | TOOL-3, TOOL-4, MAG-9, FLU-21, TOOL-21, TOOL-22 | M | P-1 | New positive and negative fixtures in `test-architecture-scanners.sh` / `test-production-artifacts.sh`; `production_artifact.py check` reports magnetita, grafita, fluorita, hematita and android stale against their old manifests. The landing then rebuilds all of them (needs Qt, libmpv and the Android SDK). |
| P-3 | `AND-6-D` | `magnetita-android-bug` | Pair from an intent only after a confirmation screen showing id, fingerprint and address, and refuse non-LAN addresses. Gate `MirrorKey`/`MirrorGlobal` on `Streaming`. Answer ENOTEMPTY for non-empty document-tree deletes. Exclude keys and pins from backup and transfer. | AND-1, AND-6, AND-4 (phone half), AND-8 | S–M | P-2 | New JVM tests (intent without confirmation refused; public address refused; key without stream ignored; non-empty delete refused); `magnetita-android/scripts/verify-production.sh` (`testDebugUnitTest`, `lintRelease`) |
| P-4 | `SID-H1-A` | `siderita-bug` | Never write through a symlinked ancestor. Create links last. Resolve tool-path links canonically. Feed the 7z password on stdin. Bound tool output. | SID-1, SID-17 | M | P-1 | `cargo test -p siderita-archive --offline` including the ported chained-link PoC tar and a tool-path escape test; Siderita `complete-production.sh` |
| P-5 | `GRA-H1-A` | `grafita-bug` | Bound PDF nesting. Replace unchecked slices with typed errors and use `checked_add`. Cap every decoder with `take(limit+1)` and a total across members and filters. Stop trusting `uncompressed_size`/`/N`. Memoise object streams. Add a negative-input table. | GRA-1, GRA-2, GRA-3, GRA-7 | M | P-1 | `cargo test -p grafita-core --offline --no-fail-fast` with fixtures: 20k `[`, non-UTF-8 xref, `startxref` past EOF, `/W` overflow, huge `/N`, 1 GiB gzip → `TooLarge`, lying ZIP header; passes as root. Landing runs Grafita **and** Siderita `complete-production.sh`. |
| P-6 | `RS-H1-A` | `celestina-rs-maintenance` | Add dormant owners: a strict `file_uri` → `PathBuf` parser, `xdg::runtime_dir()` with no `/tmp` fallback, private-state and media-landing writers, `read_bounded`, a truthful post-rename outcome, `desktop_entry::{read, scan}` (regular file, 64 KiB, one shadowing rule), and a non-blocking cancellation query. Correct the workspace README, AGENTS and crate docs. The unit is purely additive (ruling R-A3): no existing function changes behaviour, so RS-10's value unescaping and any change to `is_cancelled` land in a consumer's bug unit. | Owner halves of RS-1, RS-2/FLU-2, RS-3, RS-4, RS-6, RS-9, RS-10; RS-7; RS-19 | M–L | P-2 | `cargo test -p celestina-core --offline --locked` (new tests: localhost vs foreign host, `%XX`, NUL, non-UTF-8 bytes; relative runtime dir → `None`; file 0600, dirs 0700, mode preserved, no-replace refuses; FIFO and oversize refused; fsync-failure outcome; shadowing and cap; hermetic dir order; pause-after-cancel); `cargo clippy --all-targets`. No consumer switches, so there are no product bumps. The landing rebuilds every linking app. |
| P-7 | `FLU-H1-A` | `fluorita-bug` | Replace writes a hidden sibling, fsyncs it, trashes the original through `siderita-ops`, then renames. Land media through the P-6 writer. Add a single `try_from_secs_f64` helper. Sanitise and cap probed tags, and keep a catalogue that fails to load. Create the cancel token in `submit`. Strip XMP GPS and MPF. Make solid fill the default redaction. | FLU-1, FLU-2, FLU-3, FLU-4, FLU-6, FLU-15, FLU-16 | M | P-6 | `cargo test -p fluorita-core -p fluorita-engine --offline` on the author's machine (needs libmpv): original found in the bin (rewrite the `edit.rs:718` test), 0600 preserved, copy refuses to clobber, `f64::MAX` duration, tag cap, cancel-right-after-submit, XMP and MPF fixtures. Fluorita `complete-production.sh`; the landing also rebuilds Siderita (and Magnetita after P-2). |
| P-8 | `SID-H1-B` | `siderita-bug` | Roll back only what this call created. fsync files and directories before removing a source, then remove only the copied entries. Use `rename_without_replacing` for trash and restore. Resolve relative `Path=`. Harden the `.Trash-$uid` choice and get the uid safely. Write local `DeletionDate`. Make Replace place-then-trash. Delete the dead `unwritable`. | SID-2, SID-3, SID-4, SID-14, SID-15, SID-16, SID-20, SID-31 | M | P-2 | `cargo test -p siderita-ops --offline`: 200-round concurrent-create race with no `Ok` and missing file; relative-record restore; newcomer survives a forced copy-move; orphan `files/<name>` not replaced; symlinked `.Trash-$uid` refused; Replace-ordering test in the app crate. The landing redeploys Siderita, Fluorita and Hematita (and Magnetita after P-2). |
| P-9 | `HEM-H1-A` | `hematita-bug` | Compare statx mount ids in walk and delete, or canonicalise the root on the worker. Add an entry ceiling. Key duplicates by `st_size`. Open members `O_NOFOLLOW\|O_NONBLOCK` with a dev/ino check. Add `prune_many`. Make tests root-safe. | HEM-3, HEM-4, HEM-8, HEM-9, HEM-10, HEM-15 | M–L | P-2 | `cargo test -p hematita-core --offline` green as root and as a user; new tests for a bind mount reached through a symlinked root, `TooManyEntries`, a FIFO member, `prune_many`. The landing redeploys Hematita and Siderita. |
| P-10 | `SURF-1-E` | `celestina-bug` | Report `lock` as pending, then confirmed or failed. Use a private 0700 import root with no `/tmp` fallback, and route the unlock through C++ only. Force `PlainText` in the polkit prompt. Take the account from `getuid()`. Handle PAM expiry and echo-on prompts. Fix the sleep-inhibitor edges. Store the clipboard through the private writer. Use `runtime_dir` for the DDC lock and Melibea. | SH-1, SH-2, SH-3, SH-11, SH-12, SH-13; RS-2, RS-3 (shell adoption) | M | P-6 | CTest `shellservice_test` (started but never confirmed → failed), `lockauthenticator_test` (single verdict, uid), `tst_polkitprompt.qml` renders `<b>x</b>` literally, clipboard file mode test; `check-architecture-contract.sh`; Celestina `complete-production.sh` (no activation) |
| P-11 | `MAG-D1-D` | `magnetita-bug` | Take the session id from the pinned fingerprint and refuse duplicate ids. Handshake in the spawned task behind a semaphore. Keep the QR window open until a verified proof. Enforce `MAX_PAYLOAD_SIZE`, expire offers, sweep `.part` files. Write `commands.json` atomically. Escape Notify bodies when markup is supported. Move mounts and FIFOs to `runtime_dir`. Delete `write_private`. Map ENOTEMPTY. Add proto, link, mobile and peer to `verify-production.sh`. | MAG-1, MAG-2, MAG-3, MAG-10, MAG-16, MAG-18, MAG-29; RS-2, RS-3 (magnetita); AND-4 (daemon half) | M | P-2, P-6 | `magnetita/scripts/verify-production.sh` now runs `-p magnetita-proto -p magnetita-link -p magnetita-mobile -p magnetita-peer`; new loopback tests (hello id mismatch closes the session; a stalled Initial does not block a second accept; an unpinned connection does not burn the QR; oversized offer refused); Magnetita `complete-production.sh` |
| P-12 | `MAG-D1-E` | `magnetita-bug` | Give the phone one reader task. Give the daemon a writer task with deadlines, and enforce revocation, stop and supersede outside sends. Release held input and let releases bypass the governor. Bound the mirror FIFO. Move adapters to blocking threads. Make Forget async. Bound FUSE listings and caches. Handle SIGTERM and unmount. Remove the dial loop and the adb polling. Use a 10 s keep-alive. Keep one bus connection per worker and merge moves. Make `ConversationRow` accessible. Use row-level models. Put zbus on tokio or record approval. Gate `uniffi/cli`. Clean up the KDE leftovers, the docs and the STATUS versioning note; the MAG-25 waiver itself is already recorded in the Magnetita plan (ruling R-A5). | MAG-4, MAG-5, MAG-6, MAG-7, MAG-11, MAG-13, MAG-14, MAG-15, MAG-17, MAG-20, MAG-21, MAG-22, MAG-24, MAG-25, MAG-26, MAG-27, MAG-30; RS-5, RS-13 | L | P-11 | Loopback tests (slow sender does not desync; a send past its deadline closes; keys released on drop; bounded FIFO drops to a key frame); `cargo tree -p magnetitad -i async-io` empty or approval recorded; `cargo tree -p magnetita-mobile -e normal` without `uniffi_bindgen`; `qmllint-cxxqt.sh` for the QML; Magnetita `complete-production.sh` |
| P-13 | `AUD-1-E` | `suite-bug` (bumps Magnetita and Magnetita Android PATCH) | Advertise the real capability set, negotiate, and gate on both ends. Export the typed signals, `check_path`, backoff, discovery ranking, bounds and the sanitiser through UniFFI, and delete the Kotlin copies. Use one clipboard bound. Make Messages signal-driven and query threads with a limit. Move the upload to its own coroutine. Suspend on channels. Cap received offers. Add FFI tests. This is genuinely cross-suite: negotiated gating must change daemon, mobile crate and APK atomically, or one side refuses the other. `scripts/land-unit.py` refuses a `suite-bug`, so the author records the two bumps by hand, as the landing contract says. | MAG-8, MAG-12, MAG-19, MAG-28; AND-2, AND-3, AND-5, AND-7, AND-9 | L | P-3, P-12 | `cargo test -p magnetita-proto -p magnetita-mobile -p magnetitad --offline` (a peer that declines a capability; unknown FFI codes refused); JVM tests after the copies are removed; Gradle verify; Magnetita `complete-production.sh` plus Android build and verify |
| P-14 | `FLU-H1-B` | `fluorita-bug` (L; may split into library and player/editor commits) | Cancellable resync. Project on a worker with `Arc`. Forget same-path stale records and probe touched audio. Turn directory events into resyncs. Track completeness per root. Fix the relative dotted-ancestor check. Close portal requests on cancel. Probe stills on a worker. Reopen copies from bounded recipes. Keyboard and accessible annotation selection. Shared render-state object in `fluorita-qt`. Extract the handshake state machine and session loop into `fluorita-core`/`fluorita-engine`, bumping the generation on every close. Real MPRIS rate. Remove the trailer encoder. Spec thumbnail chunks. Typed bridge structs. `Builder::spawn`. Adopt `file_uri`. Correct the docs. | FLU-5, FLU-7, FLU-8, FLU-9, FLU-10, FLU-11, FLU-13, FLU-14, FLU-17, FLU-18, FLU-19, FLU-20, FLU-24, FLU-25, FLU-26, FLU-27, FLU-28, FLU-29, FLU-30; FLU-23 | L | P-6, P-7 | `cargo test -p fluorita-core -p fluorita-engine` (handshake: close before publish, release before closing; per-root completeness; directory move → resync); the app tests; `rg -n libx264 celestina-rs` empty; Fluorita `complete-production.sh` (the landing also rebuilds Siderita and Magnetita) |
| P-15 | `SID-H1-C` | `siderita-bug` (L) | Keep and join worker handles, asking or cancelling on quit. End jobs from the worker. Replay suppressed folder changes. Refresh must not cancel navigation. Add generations to search, Trash and Recientes. Run undo and the Trash verbs as jobs. Keep one process-wide device model on a worker. Move the remaining syscalls off-thread. Bound the PE reader. Use a bounded, cancellable thumbnail pool. Make the scan executor fallible. Adopt the P-14 handshake and delete Siderita's copy. Adopt `file_uri` and `desktop_entry::scan` (`open_with` and `ownicon` off-thread). Portal filter escaping. Dialog roles. Reduced motion. `CelestinaSlider` in `SizeRow`. Docs. | SID-5, SID-6, SID-7, SID-8, SID-9, SID-10, SID-11, SID-12, SID-13, SID-19, SID-24, SID-25, SID-26, SID-27, SID-28, SID-30; FLU-12; SID-18/RS-1, RS-4 (adoption); STY-4 | L | P-6, P-14 (if P-14 slips, land FLU-12's two-line port first) | Siderita app tests and QML tests (Qt on the author's machine); `cargo test -p siderita-core -p siderita-embedded` (a sparse 4 GB `.exe` reads a bounded number of bytes); search-generation test; the `controller.rs` and `FolderView.qml` ratchets do not grow; Siderita `complete-production.sh` |
| P-16 | `STYLE-G7-N` | `celestina-style-bug` | Spanish `qsTr` scrollbar names. Swatch tokens, and a guard for literal `withAlpha`/`multiplyAlpha`. A `lockScrim` token plus lock pairs in `check-contrast-contract.py`. `tst_switch.qml`, `tst_textfield.qml`. | STY-1, STY-2, STY-3; SH-6 (token and contract half) | M | P-1 | `celestina-style/scripts/check-style-contract.sh` with a new fixture in `test-architecture-scanners.sh`; `python3 scripts/check-contrast-contract.py`; the new Qt Quick tests; build and verify (the landing rebuilds Celestina) |
| P-17 | `SURF-1-F` | `celestina-bug` (L) | Async Inhibit and Register, a cached polkit owner, and signal-driven child IO. Non-blocking clipboard send with a deadline. Bounded niri stream and one settings reader. Decoded cover URI. Polkit retry on refusal and a bounded queue. A single verdict, and no `disconnect(this)`. `ProtocolDecoder` for polkit stdout. One reduced-motion owner, also read by the lock. Lock wash and accessible names and alerts. Per-minute clock. `expect` and `#[allow]` cleanup. README and STATUS truth. | SH-4, SH-5, SH-6 (lock half), SH-7, SH-8, SH-9, SH-10, SH-14, SH-15, SH-17, SH-19, SH-20, SH-21; RS-1 (cover), RS-6 (settings), RS-18 | L | P-6, P-10, P-16 | CTest (async paths; `spy.count()==1`; retry); QML tests (lock under reduced motion, `Accessible.name`); helper `cargo test` (a `%20` cover accepted and published decoded; clipboard send deadline; oversized niri line); `check-contrast-contract.py`; `check-documentation-contract.sh`; Celestina `complete-production.sh` |
| P-18 | `HEM-H1-B` | `hematita-bug` | Project rows through `children_rows(MAX_ROWS)` with filters. Coalesce verdicts and mark incrementally. A real action-timeout watchdog. Correct zbus error typing. Live uid and name facts. Bounded cmdline read. A shutdown that cannot stall. One byte formatter. Adopt `desktop_entry::read`. STATUS and README truth. | HEM-1, HEM-2, HEM-5, HEM-6, HEM-7, HEM-11, HEM-12, HEM-13, HEM-14; RS-4 (hematita) | M–L | P-6, P-9 | Hematita tests (batching; row cap; error mapping; timeout); `qmllint-cxxqt.sh`; `check-documentation-contract.sh`; Hematita `complete-production.sh` |
| P-19 | `GRA-H1-B` | `grafita-bug` | Return UTF-16 runs from Rust in one pass and coalesce the palette rehighlight. Recent-list and `existing()` as worker jobs, and debounced preference writes. `QPointer` target. Adopt `file_uri` (non-UTF-8 byte-exact). STATUS. | GRA-4, GRA-5, GRA-8, GRA-9; RS-1 (grafita) | M | P-5, P-6 | `cargo test -p grafita-core` (recent list via a job; no IO in `receive`); Grafita syntax test on a 5 MB single line within a time bound; Grafita **and** Siderita `complete-production.sh` |
| P-20 | `AUD-1-F` | `suite-maintenance` (L; may split into integrity / speed / docs commits) | Run hooks from an extracted `HEAD:scripts`. Run the documentation contract over the index. Add `pre-merge-commit`. Scope errata. Deploy only after the push, and let `--abort` report or restore. Scope shared verification inputs per project. Add a post-build re-check, network timeouts and a safe `close`. Batch and memoise the inventory checks. Audit only the pushed range in CI. Select qmllint modules by URI. Restrict exemption markers by suffix and path through a declared scanner migration. Add a real-guard landing test. One git runner and registry loader. Fix the root STATUS, ROADMAP, README and the versioning "Agent workflow". Register or migrate `docs/superpowers/`. | TOOL-5, TOOL-6, TOOL-7, TOOL-8, TOOL-9, TOOL-10, TOOL-11, TOOL-12, TOOL-14, TOOL-15, TOOL-16, TOOL-17, TOOL-18, TOOL-19, TOOL-20 | L | P-1 | `test-commit-scope.sh` and `test-staged-units.sh` fixtures (partial staging, merge, edited worktree guard ignored); a real-guard case in `test-land-unit.py`; documentation guard hook-mode time recorded under 5 s; `test-qmllint-target.sh` multi-module fixture; `test-language-contract.py` marker fixtures plus the "Resolved language debt" field; `check-documentation-contract.sh`; `audit-version-commits.py` |

**Unscheduled backlog** (Minor only; take when touching the file): RS-8, RS-11, RS-12, RS-14, RS-15, RS-16, RS-17, SID-21, SID-22, SID-23, HEM-16, GRA-6, MAG-23, SH-16, SH-18, FLU-22.

## Limits

Section 5 of the consolidation, "Not audited, and caveats":

A real session on the author's machine is still needed for everything below.

- **Qt, CXX-Qt and QML builds.** No app crate was built: Siderita, Grafita, Fluorita, Hematita, Magnetita, and the Celestina helper, whose `niri-ipc` did not resolve offline. There was no Qt 6 SDK, so no CTest, no `qmllint`, no Qt Quick tests, and no Qt-source cross-check. SH-10's double emission is reasoned from `QProcess` ordering, not observed.
- **Wayland and the compositor.** Nothing was checked on a live session: niri, `ext-session-lock`, lock cover and uncover, portals (FileChooser, SaveFiles), clipboard hand-off, xdg-desktop-portal-wlr `--pick-output`.
- **AT-SPI and perception.** No screen-reader, focus or contrast checks on real output. SH-6's ratios are computed with the contrast contract's method, not measured.
- **libmpv.** `fluorita-engine` tests could not link, so the real-media tests, FLU-20's teardown use-after-free and Siderita's embedded player are unverified.
- **Android and devices.** No SDK, NDK, Gradle, lint or JVM tests. No real phone. No FUSE mount beyond the loopback tests. MAG-20's battery cost is unmeasured.
- **Privileged flows.** No real polkit or PAM runs (SH-9, SH-11). No bind-mount namespace test for HEM-3.
- **Root-only failures.** The container runs as uid 0. Six `hematita-core` `usage_tree` tests and one `grafita-core` test fail only as root (HEM-15, GRA-7). GRA-1's probe used a debug build; release stack depth may differ, but the slice panics do not depend on the profile.
- **Toolchain and pipeline.**
  - MSRV 1.85 was not compiled (RS-14 rests on the dependencies' declared `rust-version`).
  - No production build, deploy or project-path landing ran. TOOL ran one docs-only trial landing in a scratch clone.
  - P-2, P-6 and any unit that touches shared inputs will make the landing rebuild several apps and the APK (TOOL-9). The author's machine therefore needs Qt, CMake, libmpv and the Android SDK available before starting the program.
- **Shallow reads.**
  - Shell: `panelmenucontroller.cpp`, `wallpaper.rs` beyond import, the nightlight adapter, `network.rs`, `ControlCentre.qml`, the tray.
  - Magnetita: `mirror_stream.rs`, `media.rs` (MPRIS polling), `magnetita-core/src/mirror.rs`.
  - Product-crate internals outside each area's scope, and `siderita-ops` trash cancellation as Hematita uses it.

## Follow-up

This unit opens the plans that carry every scheduled finding; each area
record maps its findings to these units.

- `docs/plans/active/2026-09-26-monorepo-hardening.md`: suite checkpoint `AUD-1`: `AUD-1-A` (this record), `AUD-1-B` (P-1), `AUD-1-C` (P-0b), `AUD-1-D` (P-2), `AUD-1-E` (P-13), `AUD-1-F` (P-20).
- `siderita/docs/plans/active/2026-09-26-hardening.md`: `SID-H1`: `SID-H1-A` (P-4), `SID-H1-B` (P-8), `SID-H1-C` (P-15).
- `hematita/docs/plans/active/2026-09-26-hardening.md`: `HEM-H1`: `HEM-H1-A` (P-9), `HEM-H1-B` (P-18).
- `grafita/docs/plans/active/2026-09-26-hardening.md`: `GRA-H1`: `GRA-H1-A` (P-5), `GRA-H1-B` (P-19).
- `fluorita/docs/plans/active/2026-09-26-hardening.md`: `FLU-H1`: `FLU-H1-A` (P-7), `FLU-H1-B` (P-14).
- `celestina-rs/docs/plans/active/2026-09-26-hardening.md`: `RS-H1`: `RS-H1-A` (P-6).
- `celestina/docs/plans/active/2026-08-20-persistent-carriers.md`: new rows of `SURF-1`: `SURF-1-E` (P-10), `SURF-1-F` (P-17).
- `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md`: new row of `STYLE-G7`: `STYLE-G7-N` (P-16).
- `magnetita/docs/plans/active/2026-09-13-app-design.md`: new rows of `MAG-D1`: `MAG-D1-D` (P-11), `MAG-D1-E` (P-12), and the MAG-25 waiver.
- `magnetita-android/docs/plans/active/2026-09-13-app-design.md`: new row of `AND-6`: `AND-6-D` (P-3).

Two spellings are shared by different things. The finding AND-6 (keys and
global actions with no mirror session) and the Magnetita Android checkpoint
`AND-6` are unrelated; ledger units are always written `AND-6-<letter>`. The
suite checkpoint `AUD-1` is not the Celestina shell's closed `AUD-1`
(static-audit hardening); each is owned by its own roadmap.

### 6. Contradictions and doubts

The consolidation's contradictions and the questions it put to the author,
with the ruling that settled each:

| # | Topic | Reports | Resolution or question | Ruling |
|---|---|---|---|---|
| 1 | Which crates the production inputs miss | MAG-9 (proto, link, fluorita-engine, fluorita-qt), FLU-21 (Fluorita: `siderita-ops`), TOOL-3 (a superset) | **Resolved by opening `docs/projects.toml`:** TOOL-3 is the complete list. Magnetita lacks proto, link, fluorita-core, fluorita-engine, fluorita-qt and siderita-ops. Grafita lacks celestina-core. Hematita lacks celestina-core and siderita-ops. Fluorita lacks siderita-ops. Siderita's list already includes grafita-core, the fluorita crates and hematita-core, so P-5, P-7 and P-9 do redeploy Siderita. | Resolved in the audit; no ruling needed. |
| 2 | Grafita's nature | The HG dispatch called it an "image viewer" | Resolved: it is the text editor, and its untrusted surface is the document importer (HG scope note). | Resolved in the audit; no ruling needed. |
| 3 | Proposed prefixes | SID, HG, FLU and RS propose component prefixes (`siderita-archive:`, `grafita-core:`, `fluorita-engine:`, `hematita-core:`, `celestina-core:`) or `suite:` for `celestina-core` work | Resolved by AGENTS.md and `versioning.md`: ledger-closing bugs use the product's primary `-bug` prefix. Dormant `celestina-core` owners are `celestina-rs-maintenance`, followed by product-prefixed adoptions. | Resolved in the audit; no ruling needed. |
| 4 | Fix design for `atomic_file` | RS-2 ("preserve the existing mode or default 0600") vs FLU-2 (a separate media-landing function with no-replace) | Resolved: one module and one owner, with two explicit entry points (private state; media landing that keeps the source mode and refuses to replace). This avoids a silent change of `replace` for its roughly 25 callers. | Resolved in the audit; no ruling needed. |
| 5 | "All guards pass" vs "CI red" | Six area reports vs TOOL-1 | Both true. The three guards pass; the red steps are the registry-coupled `test-*` fixtures, which neither the hooks nor the landing run. | Resolved in the audit; no ruling needed. |
| 6 | Missing cross-references | FLU-12 says it "overlaps the SID audit", but SID does not report it. SID also omits GRA-1/GRA-2/GRA-5's Siderita exposure and RS-4's Qt-thread `open_with` scan. | No contradiction; the SID audit has a coverage gap there. P-15 should re-check Siderita's embedding of other cores. | Resolved in the audit; no ruling needed. |
| 7 | **Question for the author: bumps for shared-crate fixes** | GRA-1/2/5 (grafita-core), FLU-3 (fluorita-engine), SID-2/14 (siderita-ops) change Siderita, Fluorita, Hematita or Magnetita binaries | The program uses the owning product's `-bug`. The landing redeploys the consumers, but their versions do not move. Alternatively use `suite-bug` so every affected product also bumps PATCH, which the contract allows for "one atomic shared change". Choose one policy before P-5. | **Settled by R-A2:** a shared-crate fix bumps only the owning product (`<owner>-bug`); the landing rebuilds and redeploys the consumers with their versions unchanged. |
| 8 | **Question: is P-6 maintenance?** | RS-10 (unescaping desktop values) and RS-7 would change existing behaviour in 3 or more products | P-6 is `maintenance` only if it is purely additive (new `scan`, `read`, `is_cancel_requested`). Otherwise land the behaviour change with a consumer bug, or as `suite-bug`. RS-8 was moved to the backlog for the same reason. | **Settled by R-A3:** P-6 stays purely additive `celestina-rs-maintenance`; behaviour changes land in the consumers' bug units. |
| 9 | **Question: P-1 touching `hematita/qml`** | TOOL-2 proposes `suite:` for the guard and `hematita:` for the fixes | Recommended: one `suite-maintenance` commit, because splitting would publish a red revision. If the token change is visible, it becomes `suite-bug` bumping Hematita (and celestina-style if a token is added). | **Settled by R-A6:** P-1 is one `suite-maintenance` unit; if the Hematita token fix changes what a person sees, the implementer reports it and the unit lands as `suite-bug` bumping Hematita. |
| 10 | **Question: order of TOOL-8 and TOOL-9** | Ranking puts them late (P-20) | Every landing in this program is exposed to them: deploy before guards, and the fan-out to all toolchains. Either pull those two items forward right after P-2, or run `sh scripts/check-documentation-contract.sh` locally before each `land-unit.py` until P-20 lands. | **Not settled by a ruling.** The program order stands (P-20 last); until P-20 lands, run `sh scripts/check-documentation-contract.sh` locally before each `land-unit.py`. |
| 11 | **Question: MAG-25** | 37 Magnetita product fixes landed as `maintenance` | History is immutable. Record a waiver in the Magnetita plan (P-12), or state that installed versions under-report changes since 2026-09-01. Use `-bug` from now on. | **Settled by R-A5:** a waiver is recorded in the Magnetita plan; history is immutable and Magnetita product fixes use `-bug` from now on. |
| 12 | **Question: RS-11** | `dotfiles-core` has no consumer | Remove it with its component scope, or name its consumer and plan. This needs the author's decision. | **Settled by R-A4:** `dotfiles-core` is kept; the missing consumer is recorded as an exclusion of the celestina-rs plan for the author. |

## Landing

- **Base revision:** `9d022dd5d2f4935550194d10a5131b91ef184889`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
