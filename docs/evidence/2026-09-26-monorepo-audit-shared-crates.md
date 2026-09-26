# Evidence: the shared Rust workspace audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): the `celestina-rs/` workspace manifests, `celestina-core`, `dotfiles-core`, `celestina-shell-core`, dependency hygiene across all crates, and recipes duplicated across crates and apps, on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Audit RS: shared Rust workspace `celestina-rs/`".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
python3 scripts/agent-context.py celestina-rs
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
cargo test -p celestina-core -p celestina-dotfiles-core -p celestina-shell-core --offline --locked
cargo clippy --all-targets --offline --locked
cargo tree -p magnetitad -e features -i zbus
cargo tree -p magnetita-mobile -e normal
```

Every consumer claim was checked with `rg` over the whole monorepo
(each copy of a recipe is named in its finding). `cargo tree` needed a
one-time read-only registry index fetch with `--locked`; tests and
clippy then ran offline.

## Result

- **Exit:** every command exited 0: 34 + 4 + 349 tests passed, clippy reported nothing, and the three guards printed OK.
- **Observed:** 19 findings: 0 Critical, 5 Important, 14 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Base: `main` at `9d022dd`. Scope: the workspace manifests, `celestina-core`, `dotfiles-core`, `celestina-shell-core`, dependency hygiene across all crates, and recipes duplicated across crates and apps.

### Executive summary

1. **Healthy:** `unsafe_code = "forbid"` applies to the whole workspace. There is no production `unwrap`/`panic!` in the three shared crates. `cargo test --offline --locked` for `celestina-core`, `celestina-dotfiles-core` and `celestina-shell-core` passes (34 + 4 + 349 tests), and so does `cargo clippy --all-targets`. `check-architecture-contract.sh`, `check-documentation-contract.sh` and `check-language-contract.py` all report OK.
2. **Healthy:** `Generation`/`GenerationClock`, `percent`, `pathkey`, `xdg::{config,cache,data,state}_home` and `atomic_file::replace` are real single owners that 25+ consumers use. `celestina-shell-core` is pure except for its declared `journal` sink, and its framing and bounds are careful.
3. **Top risk (bug):** the `file://` URI → local path recipe exists in at least 7 places with different rules. The shell's media cover check (`celestina/src/provider_adapter/media.rs`) never percent-decodes, so any cover whose path has a space or non-ASCII byte is dropped. It also validates the literal file while QML opens the decoded one.
4. **Top risk (security):** `atomic_file::replace` cannot set a mode. The clipboard history is therefore persisted at the process umask (typically 0644), inside directories created 0755, which breaks the XDG 0700 rule. Meanwhile the journal, which deliberately excludes clipboard content, is 0600/0700. `magnetita-net` already had to fork the recipe to protect its private key.
5. **Top risk (security/reuse):** no owner resolves `XDG_RUNTIME_DIR`. Seven ad-hoc copies exist, and several fall back to world-writable `/tmp`, including for magnetitad's FUSE mountpoints and video FIFOs.
6. **Reuse:** the `.desktop` directory scan (shadow-by-id, read, parse, listability) is copied in Siderita, the shell launcher and Hematita. Each copy uses an unbounded `read_to_string`, and two of them run on the Qt thread.
7. **Dependency hygiene:** magnetitad runs tokio and also zbus's default async-io executor (18 async crates), and no approval is recorded. `magnetita-mobile` enables `uniffi/cli` in its library dependency, so 58 of its 124 dependencies come from the bindgen/CLI stack.
8. **Toolchain truth:** the declared MSRV 1.85 is false. The locked graph needs 1.87 (zbus/zvariant) and 1.88 (`time` via rcgen), and nothing checks it.
9. **Conventions:** error types follow three incompatible conventions: flatten `io::Error` into strings, store sources without `source()`, or chain properly. Text bounding exists three times with different units, and there is no bounded reader for small state files.
10. **Documentation truth:** README, STATUS, the local AGENTS.md and the crate docs do not match the checkout. Five crates are undocumented, `dotfiles-core` has zero consumers, and the shell-core "no IO" claim is contradicted by `journal`.

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| RS-1 | Important | 4 Architecture (+1 bug) | `celestina/src/provider_adapter/media.rs:170` | `file://` → path parsed 7× with divergent rules; the shell's cover check skips percent-decoding | M | `celestina-core` (+ consumers) |
| RS-2 | Important | 2 Security | `celestina-rs/crates/celestina-core/src/atomic_file.rs:15,40-44` | `replace` has no mode; clipboard history lands 0644 in 0755 dirs; key-writing recipe forked | S | `celestina-core` (+ `celestina`) |
| RS-3 | Important | 2 Security / 4 | `celestina-rs/crates/magnetitad/src/mount.rs:19-23` | No `xdg::runtime_dir` owner; 7 copies, `/tmp` fallback for FUSE mounts, FIFOs, locks | S | `celestina-core` (+ consumers) |
| RS-4 | Important | 4 Architecture (+1) | `siderita/src/apps.rs:32-70` | `.desktop` scan copied 3×; unbounded reads, two on the Qt thread | M | `celestina-core` (+ consumers) |
| RS-5 | Important | 4 Dependency hygiene | `celestina-rs/crates/magnetitad/Cargo.toml` (`zbus = "5"`) | Second async executor (async-io) next to tokio without recorded approval | S | `magnetita` |
| RS-6 | Minor | 1 / 4 | `celestina/src/provider_adapter/settings.rs:46` | No owner for bounded state-file reads; bound checked after a full read, TOCTOU or absent | M | `celestina-core` |
| RS-7 | Minor | 1 Correctness | `celestina-rs/crates/celestina-core/src/lib.rs:113-118` | `is_cancelled()` blocks while paused (60 ms sleep poll) behind a query-shaped name | S | `celestina-core` |
| RS-8 | Minor | 1 Correctness | `celestina-rs/crates/celestina-core/src/pathkey.rs:75-91` | `pathkey::decode` accepts non-canonical keys despite promising refusal | S | `celestina-core` |
| RS-9 | Minor | 1 / 5 | `celestina-rs/crates/celestina-core/src/atomic_file.rs:21-28` | Returns `Err` after a committed rename; callers then diverge from disk; no failure-path test | S | `celestina-core` |
| RS-10 | Minor | 1 / 5 | `celestina-rs/crates/celestina-core/src/desktop_entry.rs:105,242-247,456-460` | Spec gaps (no value unescape, `\;`, relative/empty `XDG_DATA_DIRS`); an order test that asserts no order | S | `celestina-core` |
| RS-11 | Minor | 4 Architecture | `celestina-rs/crates/dotfiles-core/Cargo.toml:2` | `celestina-dotfiles-core` has no consumer anywhere; speculative crate | S | `celestina-rs` |
| RS-12 | Minor | 4 Architecture | `celestina-rs/crates/magnetita-link/src/error.rs:46` | Three incompatible error conventions; `LinkError` drops sources | M | `celestina-rs` / `magnetita` |
| RS-13 | Minor | 4 Dependency hygiene | `celestina-rs/crates/magnetita-mobile/Cargo.toml` (`uniffi … features = ["cli"]`) | Bindgen/CLI stack in the library's normal dependencies (58/124 deps) | S | `magnetita` |
| RS-14 | Minor | 7 Documentation / toolchain | `celestina-rs/Cargo.toml` (`rust-version = "1.85"`) | Declared MSRV contradicted by zbus (1.87) and `time` (1.88); unchecked | S | `celestina-rs` |
| RS-15 | Minor | 4 / 8 | `celestina-rs/crates/siderita-archive/Cargo.toml` (dev-deps), `magnetita-*/Cargo.toml` | Shared third-party crates declared per crate; redundant dev-dep; unjustified deps | S | `celestina-rs` |
| RS-16 | Minor | 4 Architecture | `celestina-rs/crates/magnetita-core/src/text.rs:18` | Text bounding implemented 3× (UTF-16 units, scalars, scalars + control filter) | S | `celestina-shell-core` |
| RS-17 | Minor | 3 Performance / 4 | `celestina-rs/crates/celestina-shell-core/src/journal.rs:555-560` | Reads up to 4 MiB to test the last byte; busy-poll close; C++ twin without parity test | S | `celestina-shell-core` |
| RS-18 | Minor | 8 Quick win | `celestina-rs/crates/celestina-shell-core/src/nightlight.rs:70-71` | Dead or noise `#[allow]` attributes (lints not enabled; `TEMPERATURE` alias) | S | `celestina-shell-core` |
| RS-19 | Minor | 7 Documentation truth | `celestina-rs/README.md` (Architecture table) | README, STATUS, local AGENTS, shell-core `lib.rs` and `image.rs` contradict the checkout | S | `celestina-rs` |

---

### RS-1 — `file://` URI → local path is implemented seven times, with a live bug in the shell

- **Severity / category:** Important / 4 Architecture and reuse (also 1 Correctness)
- **Evidence (the copies and how they differ):**
  - `celestina/src/provider_adapter/media.rs:169-179`:
    ```rust
    let path = url.trim().strip_prefix("file://")?;
    if path.is_empty() || !std::path::Path::new(path).is_absolute() { return None; }
    let metadata = std::fs::metadata(path).ok()?;
    ```
    There is no percent-decoding. MPRIS `mpris:artUrl` is a URI, so `file:///home/u/My%20Music/cover.jpg` is `metadata`'d literally, fails, and the panel shows no cover. The path is then published unchanged, and `celestina/qml/MediaMini.qml:55` does `"file://" + root.artPath`, which Qt decodes. The bounded-size and image-header check therefore runs on one file (literal `%2F…` name) while Qt loads another. The documented guarantee "checked … before Qt ever opens it" is defeated.
  - `siderita/src/dbus.rs:137-143`: `Some(index) => &rest[index..]` accepts **any** authority, so `file://otherhost/etc/x` becomes local `/etc/x`. The decode is lenient.
  - `grafita/src/url.rs:34-51`: a private `%XX` decoder that ends in `String::from_utf8(out).ok()`. Non-UTF-8 names passed as URIs are refused, which contradicts ADR 0008's byte-exact rule.
  - `fluorita/src/folders.rs:210-216` refuses `localhost`, while `fluorita/src/activation.rs:87-96` in the same app accepts it.
  - `celestina-rs/crates/magnetitad/src/devices.rs:369-393` is the strict one: localhost-only, `decode_strict`, NUL refusal, absolute check.
  - `celestina-rs/crates/celestina-shell-core/src/notifications.rs:118-121` keeps the encoded form (currently unpublished).
- **Why it matters:** the same input gets four different answers (remote host accepted or refused, `localhost` accepted or refused, strict or lenient, UTF-8-only or byte-exact). One consumer is visibly broken for common paths, and its validation can be bypassed by name.
- **Fix:** lift magnetitad's `path_for_file_uri` into `celestina_core::percent` (or a `file_uri` module) as the one typed parser (`Result<PathBuf, FileUriError>`), with tests. Then delegate each copy to it in its own prefixed unit. The shell must publish the decoded path, or better, the canonical `file://` re-encoding of the validated bytes.
- **Effort:** M (owner S, then one S unit per consumer). **Prefix:** `celestina-core`, then `celestina`, `siderita`, `grafita`, `fluorita`, `magnetita`.

### RS-2 — `atomic_file::replace` cannot create private files; clipboard history is world-readable

- **Severity / category:** Important / 2 Security
- **Evidence:** `celestina-rs/crates/celestina-core/src/atomic_file.rs:15` has `fs::create_dir_all(parent)?;`, and `:40-44` creates the temporary with `OpenOptions::new().write(true).create_new(true)` and no `.mode()`. The new file is therefore `0666 & !umask` (0644 under the usual umask), and missing parents are 0755. The XDG base-directory spec asks for 0700 when creating them.
  - `celestina/src/provider_adapter/clipboard.rs:74,110` persists the clipboard history (arbitrary copied text such as tokens and addresses; only password-manager-hinted selections are skipped) to `$XDG_STATE_HOME/celestina/clipboard.json` through this function.
  - By contrast, `celestina-shell-core/src/journal.rs:456,462` forces 0700/0600 for a journal that by policy never contains clipboard content.
  - `celestina-rs/crates/magnetita-net/src/cert.rs:145-149` states it "cannot be reused here because its temporary is created at the process umask" and re-implements the recipe (`write_private`). That is a second active path for the same invariant.
  - `replace` also discards an existing file's mode: a user who `chmod 600`s a state file gets 0644 back at the next write.
- **Why it matters:** on hosts where `$HOME` is traversable (0755 homes remain common), other local accounts can read the clipboard history and other privacy-relevant state (Grafita's recent documents list, Siderita bookmarks).
- **Fix:** give the owner a mode. Either use `replace_private(path, bytes)` (0600 temporary via `OpenOptionsExt::mode`, parents via `DirBuilderExt::mode(0o700)`) or have `replace` preserve the mode of an existing destination and default to 0600 for suite state. Then delete `write_private` and move the clipboard and recent-documents writers to it.
- **Effort:** S. **Prefix:** `celestina-core`, then `celestina` and `magnetita`.

### RS-3 — `XDG_RUNTIME_DIR` has no owner; copies fall back to `/tmp`

- **Severity / category:** Important / 2 Security (also 4 Reuse)
- **Evidence:** `celestina-core/src/xdg.rs` owns config/cache/data/state but not the runtime directory. Ad-hoc copies:
  - `celestina-rs/crates/magnetitad/src/mount.rs:19-23` builds FUSE mountpoints with `.unwrap_or_else(std::env::temp_dir).join("magnetita")`.
  - `magnetitad/src/link_wire/mirror.rs:56-62` puts video FIFOs under `.unwrap_or_else(std::env::temp_dir)`.
  - `magnetitad/src/artwork.rs:36-46` returns an error instead.
  - `magnetita/src/mirror_view.rs:130,321` does the same kind of lookup.
  - `celestina/src/provider_adapter/brightness.rs:216-219` puts the DDC lock under `temp_dir`.
  - `celestina/src/provider_adapter/melibea.rs:34` has its own lookup.
  - `magnetita-peer/src/lib.rs:33-38` re-implements `config_home` without the absolute-path check and falls back to `"."`.

  None of them validates that the value is absolute, which `xdg.rs` enforces for every other base dir.
- **Why it matters:** when the variable is unset (a helper started outside a logind session, or tests), predictable names in world-writable `/tmp` let another local user pre-create `/tmp/magnetita` (a symlink, or a directory they own) and capture the mount or FIFO location. Three different failure behaviours for one question also violate "every invariant has one owner".
- **Fix:** add `xdg::runtime_dir() -> Option<PathBuf>`. It returns the variable only when it is absolute (optionally owned by the euid with mode 0700, as the spec requires) and never falls back to `/tmp`. Consumers degrade the feature when it is `None`. Replace `magnetita-peer`'s copy with `xdg::config_home`.
- **Effort:** S (owner), S per consumer. **Prefix:** `celestina-core`, then `magnetita`, `celestina`.

### RS-4 — The `.desktop` application scan is copied three times, unbounded, partly on the Qt thread

- **Severity / category:** Important / 4 Architecture (also 1 thread affinity and unbounded input)
- **Evidence:** `celestina_core::desktop_entry` owns parsing and `application_dirs()`, but every consumer rewrites the walk.
  - `siderita/src/apps.rs:32-70` claims an id *after* a successful parse and restates listability as `parsed.is_application && !parsed.hidden && !parsed.no_display`, a second copy of `is_listable()` without the name check.
  - `celestina/src/provider_adapter/launcher.rs:102-137` claims an id *before* reading, so an unreadable override shadows the system entry differently from Siderita.
  - `hematita/src/processes.rs:572-578` does a per-id lookup on the Qt thread (acknowledged in its comment).

  All three use `std::fs::read_to_string(&path)` with no size or file-type check. `siderita/src/controller/shell.rs:54` calls `apps_for_mime` directly from the `open_with` invokable (Qt thread), reading every `.desktop` file on the system. `siderita/src/ownicon.rs:116-121` reads any `*.desktop` in a browsed folder, also from a Qt-thread invokable (`controller/glyphs.rs:49`). A FIFO named `x.desktop` in `/tmp` blocks the UI forever, and a huge one is read whole.
- **Why it matters:** divergent shadowing semantics, a duplicated domain rule, and blocking or unbounded IO on the GUI thread.
- **Fix:** add `desktop_entry::read(path)` to `celestina-core` (regular file only, size cap, e.g. 64 KiB, via `take`) and `desktop_entry::scan(cancel, max_entries) -> Vec<DesktopEntry>` with one shadowing rule and tests. Consumers delegate, and Siderita moves `open_with`'s scan to a worker.
- **Effort:** M. **Prefix:** `celestina-core`, then `siderita`, `celestina`, `hematita`.

### RS-5 — magnetitad carries two async executors

- **Severity / category:** Important / 4 Dependency hygiene
- **Evidence:** `celestina-rs/crates/magnetitad/Cargo.toml` declares `tokio = { workspace = true, features = ["rt-multi-thread", …] }` and also `zbus = "5"`. The manifest comment says "zbus runs its own small executor for the served interface". `cargo tree -p magnetitad -e features -i zbus` shows `default → async-io → async-executor, async-process, async-fs, blocking`. The resulting graph has `async-executor`, `async-io`, `async-process`, `async-signal`, `blocking`, `polling`, `piper`, `event-listener*`, `futures-lite` and more next to `tokio v1.53.1`. `docs/standards/rust-cpp-qt-qml.md:81` says "duplicate async executors require author approval and measured need", and a search finds no such record. The shell (`celestina/Cargo.toml:66`) chooses `default-features = false` deliberately; this crate does not.
- **Why it matters:** two reactors and thread pools in one daemon, more supply-chain surface, and an unapproved exception to a written standard.
- **Fix:** evaluate `zbus = { version = "5", default-features = false, features = ["tokio", "blocking-api"] }` on the runtime magnetitad already owns. This requires the zbus connection to be built in that runtime's context, so it needs verifying. Otherwise record the author's approval and the measured reason next to the dependency.
- **Effort:** S–M. **Prefix:** `magnetita`

### RS-6 — No owner for bounded state-file reads

- **Severity / category:** Minor / 1 Correctness (unbounded input) and 4 Reuse
- **Evidence:** `atomic_file::replace` owns writes for about 25 consumers, but each reader invents its own read.
  - `celestina/src/provider_adapter/settings.rs:46` `std::fs::read(path)` then `Settings::from_bytes`, whose `MAX_FILE_BYTES` check (`celestina-shell-core/src/settings.rs:268`) runs after the whole file is in memory.
  - `fluorita-engine/src/{catalogue_store.rs:69-85, edit_store.rs:142-148, source_store.rs:133-137}` use `metadata().len()`, then read. This is a TOCTOU window, and a FIFO reports 0 bytes and then blocks.
  - `grafita-core/src/{preferences.rs:60, recent.rs:39}`, `magnetitad/src/settings.rs:78`, `magnetita-net/src/trust.rs:73` and `cert.rs:49-50`, and `siderita/src/{bookmarks.rs:59, settings.rs:134}` use unbounded `read_to_string`.
  - Only `celestina/src/provider_adapter/clipboard.rs:83-91` and `celestina/src/niri_adapter.rs:365` bound correctly with `take(max + 1)`.
- **Why it matters:** "filesystem input is hostile and bounded" is enforced by convention in 3 of about 13 readers.
- **Fix:** add `atomic_file::read_bounded(path, max) -> io::Result<Option<Vec<u8>>>` (open with `O_NONBLOCK` or check the file type, regular file only, `take(max + 1)`, report oversize) next to `replace`, and migrate readers per prefix.
- **Effort:** M. **Prefix:** `celestina-core` (then consumers)

### RS-7 — `CancellationToken::is_cancelled` is a blocking call with a query's name

- **Severity / category:** Minor / 1 Correctness
- **Evidence:** `celestina-rs/crates/celestina-core/src/lib.rs:113-118`:
  ```rust
  pub fn is_cancelled(&self) -> bool {
      while self.paused.load(Ordering::Acquire) && !self.cancelled.load(Ordering::Acquire) {
          std::thread::sleep(std::time::Duration::from_millis(60));
  ```
  I checked every non-test caller in the monorepo (about 60 sites in `fluorita-*`, `siderita-*`, `hematita-*`, `grafita-core` and app workers), and all run on worker threads today. But any future call on the Qt thread, or while holding a lock the Qt thread needs, freezes for as long as a job is paused (Siderita's `controller/jobs.rs:281-283` pauses tokens). Resume latency is up to 60 ms of polling. `pause()` after `cancel()` leaves `is_paused() == true` on a cancelled token.
- **Why it matters:** a trap in the most widely shared primitive, which only a doc comment guards.
- **Fix:** keep the blocking behaviour under an explicit name (`checkpoint()` or `wait_unless_cancelled()`) and make `is_cancelled()` a plain load. Alternatively add `is_cancel_requested()` for non-worker callers. Park on a `Condvar` instead of sleeping, and make `pause()` a no-op once cancelled. Add a test for pause-after-cancel.
- **Effort:** S (API addition) or M (rename across consumers). **Prefix:** `celestina-core`

### RS-8 — `pathkey::decode` accepts keys `encode` never emits

- **Severity / category:** Minor / 1 Correctness
- **Evidence:** `celestina-rs/crates/celestina-core/src/pathkey.rs:75-91` checks only non-empty, ASCII, well-formed escapes and absoluteness. `decode("/home/a b")`, lowercase `%ff` and `"/a%2Fb"` are all accepted. The module doc (`:14-21`) promises "a value that did not come from `encode` is rejected". The case the ADR exists for is a caller that mistakenly passes display text. For an ASCII file literally named `/x/%41`, the display string `"/x/%41"` decodes to `/x/A`, a different file.
- **Fix:** add a canonical round-trip check, `if encode(&path) != key { return Err(PathKeyError::NotCanonical) }`, with tests. It is cheap, and it rejects every non-canonical spelling including the `%41` case (`encode("/x/A") == "/x/A"`).
- **Effort:** S. **Prefix:** `celestina-core`

### RS-9 — `atomic_file::replace` reports failure after a committed replacement

- **Severity / category:** Minor / 1 Correctness (also 5 Tests)
- **Evidence:** `atomic_file.rs:21-28` runs `fs::rename(&temporary, path)?;` and then `fs::File::open(parent)?.sync_all()`. If the directory open or fsync fails, the function returns `Err` although the new bytes are already in place. The cleanup `remove_file(&temporary)` is then a no-op. `celestina/src/provider_adapter/settings.rs:192-198` treats `Err` as "not saved" and keeps the previous value in force, so the session and the disk diverge until restart. The only test (`:62-80`) covers the success path.
- **Fix:** return a distinct outcome (for example `Ok(Durability::RenamedNotSynced)`, or an error kind the caller can tell apart) after a successful rename. Add tests for the temporary's cleanup on write failure and for the unwritable-parent path.
- **Effort:** S. **Prefix:** `celestina-core`

### RS-10 — `desktop_entry` spec gaps and a test that asserts nothing about order

- **Severity / category:** Minor / 1 Correctness and 5 Tests
- **Evidence:**
  - `desktop_entry.rs:105` stores values raw. The spec's string escapes (`\s \n \t \r \\`) are never undone before `split_exec`, so an `Exec` written as `"\\$HOME"` yields `\$HOME` instead of `$HOME`.
  - `semicolon_list` (`:71-78`) splits on every `;`, ignoring the spec's `\;`.
  - `application_dirs` (`:242-247`) keeps relative `XDG_DATA_DIRS` entries, which resolve against the CWD. `xdg.rs` rejects relative values for every other variable. It also treats a set-but-empty variable as "no system dirs" instead of the spec default.
  - The test `the_user_directory_comes_before_the_system_ones` (`:456-460`) asserts only `len() >= 2` and that the last entry is absolute, and it depends on the environment. Meanwhile `xdg.rs:70` states "no other test in this crate reads these variables", which is false.
- **Fix:** unescape values per spec before list and `Exec` handling, and filter relative and empty data dirs. Make the order test hermetic by passing the environment in (a small internal `application_dirs_from(env)` function) and asserting order.
- **Effort:** S. **Prefix:** `celestina-core`

### RS-11 — `dotfiles-core` is a speculative crate with no consumer

- **Severity / category:** Minor / 4 Architecture
- **Evidence:** `celestina-rs/crates/dotfiles-core/Cargo.toml:2` has `name = "celestina-dotfiles-core"`. A search of every `Cargo.toml`, `build.rs` and `*.rs` finds no dependent, only the workspace member entry. `celestina-rs/AGENTS.md` says "Do not create a speculative shared core without a real owner and contract", and `STATUS.md` says "the registered workspace crates are present and consumed". The crate also drops its error source: `PlanError` keeps `kind` and `message: String` instead of the `io::Error` (`lib.rs:97-102,192-198`). The package name does not match its directory or its `dotfiles-core` commit prefix.
- **Fix:** record the author's decision. Either remove the crate and its component scope, or name the consumer and plan in STATUS/ROADMAP. If it stays, keep the `io::Error` as `source`.
- **Effort:** S. **Prefix:** `celestina-rs`

### RS-12 — Three incompatible error-type conventions across crates

- **Severity / category:** Minor / 4 Architecture
- **Evidence:**
  - (a) **Flatten `io::Error` to strings** so the type can derive `Clone`/`Eq`: `siderita-ops/src/error.rs` `OpError::Io { kind, message: String }`, `siderita-core/src/scan.rs:118-124`, `dotfiles-core/src/lib.rs:97-102`. `source()` is lost.
  - (b) **Store the source but never expose it:** `magnetita-link/src/error.rs:46` `impl std::error::Error for LinkError {}` although it holds `Io(io::Error)`, `Protocol(DecodeError)` and `Pairing(PairError)`. `quinn` errors are stringified (`:66-75`).
  - (c) **Proper chaining:** `fluorita-engine/src/error.rs:93-96`, `hematita-core/src/usage/{walk,remove,duplicates}.rs`, `celestina-shell-core/src/lines.rs:155-163`.

  AGENTS.md requires "Typed errors retain context and source", and no workspace document says which convention a crate should pick.
- **Fix:** write one paragraph in `docs/standards/rust-cpp-qt-qml.md` or the workspace README: keep `io::Error` as `source`, and where `Clone` is needed use `Arc<io::Error>`. Implement `source()` on `LinkError` and keep typed `quinn` errors. Migrate (a) opportunistically.
- **Effort:** M. **Prefix:** `celestina-rs` (convention), `magnetita` (`LinkError`)

### RS-13 — `uniffi/cli` in `magnetita-mobile`'s library dependency

- **Severity / category:** Minor / 4 Dependency hygiene
- **Evidence:** `celestina-rs/crates/magnetita-mobile/Cargo.toml` declares `uniffi = { workspace = true, features = ["cli"] }` for the whole crate. Only `src/bin/uniffi-bindgen.rs` needs it. `cargo tree` shows `uniffi_bindgen`, `clap`, `askama`, `goblin` and `cargo_metadata` in normal dependencies: 58 of the crate's 124 normal dependencies come from `uniffi_bindgen`. They are also compiled for `magnetita-peer`, for magnetitad's tests (dev-dependency) and for the Android `cdylib` cross-build.
- **Fix:** add `[features] bindgen = ["uniffi/cli"]` with `required-features = ["bindgen"]` on the `[[bin]]`, and drop `cli` from the library dependency.
- **Effort:** S. **Prefix:** `magnetita`

### RS-14 — The declared MSRV is false and unchecked

- **Severity / category:** Minor / 7 Documentation truth (toolchain)
- **Evidence:** `celestina-rs/Cargo.toml` sets `rust-version = "1.85"` for every crate, and `README.md` says it "declares an MSRV floor of 1.85". The lockfile resolves `zbus 5.18.0`/`zvariant 5.13.1` (`rust-version = "1.87"`) for magnetitad and `time 0.3.54` (`1.88.0`, via `rcgen` in `magnetita-net`). No script builds with 1.85. `resolver = "2"` is not MSRV-aware.
- **Fix:** either raise the floor to what the lock needs (1.88) or give crates their own `rust-version` and add a cheap `cargo +1.85 check` guard for the crates that claim it. Update the README sentence.
- **Effort:** S. **Prefix:** `celestina-rs`

### RS-15 — Shared third-party crates are not centralised; some dependencies are unjustified

- **Severity / category:** Minor / 4 and 8 Quick win
- **Evidence:** `[workspace.dependencies]` covers serde, minicbor, spake2, uniffi, quinn and tokio, but not:
  - `rustls` (declared in `magnetita-net` with `tls12`/`logging`, and in `magnetita-link` without them);
  - `ring = "0.17"` (`magnetita-net`, `magnetita-proto`);
  - `zip = "2"` (three crates);
  - `flate2` (two crates);
  - `rustix` (`hematita-core` with `default-features = false`, `magnetitad` with defaults).

  `siderita-archive/Cargo.toml` repeats its normal `zip` dependency verbatim as a dev-dependency (a no-op). `magnetita-net` uses `celestina-core = { path = "../celestina-core" }` instead of `.workspace = true`. Seven `magnetita-*` manifests hard-code `license` instead of inheriting it. `evdev = "0.13"` (magnetitad) and `rustls` (magnetita-link) have no justification comment, which AGENTS.md ("Justify each dependency in its manifest") requires.
- **Fix:** move the crypto and archive stack into `[workspace.dependencies]` with the shared justification, delete the redundant dev-dependency, inherit `license`/`celestina-core`, and add the two missing justifications.
- **Effort:** S. **Prefix:** `celestina-rs`

### RS-16 — Three text-bounding helpers

- **Severity / category:** Minor / 4 Reuse
- **Evidence:**
  - `celestina-shell-core/src/lib.rs:113-124` `bounded(text, limit)` counts UTF-16 units, deliberately (the Qt host revalidates in UTF-16).
  - `magnetita-core/src/text.rs:18-23` `bounded(text, limit)` counts scalars.
  - `fluorita-core/src/streams.rs:80-87` `bounded(value)` trims, filters control characters and counts scalars.

  Magnetita's bounded strings also end in Qt labels and D-Bus properties, where the shell's UTF-16 reasoning applies equally. No evidence records why the units differ.
- **Fix:** do the semantic comparison the reuse rules require. Either move one `bounded_utf16` owner into `celestina-core` and use it at every Qt-bound seam, or record in both modules why scalar counting is correct for Magnetita and Fluorita.
- **Effort:** S. **Prefix:** `celestina-shell-core` (or `celestina-core` if extracted)

### RS-17 — Journal sink inefficiencies and an untested C++ twin

- **Severity / category:** Minor / 3 Performance and 4 Reuse
- **Evidence:**
  - `celestina-shell-core/src/journal.rs:555-560` `ends_with_newline` does `fs::read(path)` (up to `MAX_FILE_BYTES` = 4 MiB) to inspect one byte, every time the file is (re)opened, including every 30 s retry after a failure (`REOPEN_AFTER`).
  - `close()` (`:212-222`) busy-polls `is_finished()` every 10 ms instead of signalling through the existing `Condvar`.
  - The whole policy is restated in C++ in `celestina/src/diagnosticjournal.cpp` (`maxFiles = 8`, rotation, `%1-%2` hex run id, the three mirror-off spellings, `component-*.jsonl` retirement), and no shared fixture or parity test binds the two.
- **Fix:** open the file for reading, `seek(End(-1))` and read one byte. Wait on a completion `Condvar` with the deadline. Add a golden-file test fixture (a line format and retirement listing) that both the Rust and C++ tests consume.
- **Effort:** S. **Prefix:** `celestina-shell-core` (fixture consumer in `celestina`)

### RS-18 — Dead and noise `#[allow]` attributes

- **Severity / category:** Minor / 8 Quick win
- **Evidence:**
  - `celestina-shell-core/src/nightlight.rs:70-71` has `#[allow(non_snake_case)] let TEMPERATURE = temperature;`, an alias whose only purpose is to need an allow.
  - `weather.rs:95-98` and `nightlight.rs:183-187` allow `clippy::cast_possible_truncation`/`cast_sign_loss`. Those are pedantic lints that `[workspace.lints.clippy] all = "warn"` never enables, so the attributes suppress nothing. AGENTS.md: "Never add `#[allow]` to hide debt".
- **Fix:** use `temperature` directly and delete the two inert allows, or switch to `i16::try_from` / `u16::try_from` style conversions if the intent is to document the bound.
- **Effort:** S. **Prefix:** `celestina-shell-core`

### RS-19 — Documentation contradicts the checkout

- **Severity / category:** Minor / 7 Documentation truth
- **Evidence:**
  - `celestina-rs/README.md` Architecture table omits `hematita-core`, `magnetita-proto`, `magnetita-link`, `magnetita-peer` and `magnetita-mobile`. It describes `celestina-core` without `desktop_entry`, `pathkey` or `image`, and `celestina-shell-core` as "bounded helper framing, provider envelope and shell command vocabulary" although the crate has 32 modules, including network, bluetooth, notifications, nightlight, weather and a file-writing `journal`.
  - `celestina-rs/AGENTS.md` "Local boundary" names `magnetita-net` and `magnetitad` as *the* transport and service layers, but `magnetita-link` (tokio/quinn), `magnetita-mobile` (UniFFI cdylib) and `magnetita-peer` (binary) are unmentioned exceptions.
  - `STATUS.md` (updated 2026-08-03) says all registered crates are consumed; see RS-11.
  - `celestina-shell-core/src/lib.rs:6-66` lists modules without `diagnostics`, `journal`, `media` or `melibea`, says `runtime` is "the aggregate those three add up to", and ends with "Nothing here knows … IO as a `Write`", which `journal.rs` (files, permissions, a thread, environment variables) contradicts.
  - `celestina-core/src/image.rs:3-5` says covers arrive "over KDE Connect from a phone" and from players. Only `celestina/src/provider_adapter/media.rs` calls it: `magnetitad/src/artwork.rs` has no writer any more, and `magnetita_core::IncomingAlbumArt` has no user.
- **Fix:** update the README table and the local AGENTS boundary, and bring STATUS up to date. Correct the shell-core crate doc to state its one IO exception, and fix `image.rs`'s provenance paragraph.
- **Effort:** S. **Prefix:** `celestina-rs` (the shell-core `lib.rs` text under `celestina-shell-core`)

## Limits

- The product crates' internal logic (`siderita-*`, `grafita-core`, `fluorita-*`, `hematita-core`, `magnetita-*`, `magnetitad`) was read only where a cross-cutting recipe or dependency led there.
- `cargo tree` resolution needed a one-time registry index fetch, which was read-only and ran with `--locked`. Tests and clippy then ran `--offline --locked` with `CARGO_TARGET_DIR` in the scratchpad, so no production target or cache was touched. No 1.85 toolchain was installed, so RS-14 rests on the dependencies' declared `rust-version`, not on a compile attempt.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-6 | `RS-H1-A` | `celestina-rs/docs/plans/active/2026-09-26-hardening.md` | RS-1, RS-2, RS-3, RS-4, RS-6, RS-7, RS-9, RS-10, RS-19 |
| P-10 | `SURF-1-E` | `celestina/docs/plans/active/2026-08-20-persistent-carriers.md` | RS-2, RS-3 |
| P-11 | `MAG-D1-D` | `magnetita/docs/plans/active/2026-09-13-app-design.md` | RS-2, RS-3 |
| P-12 | `MAG-D1-E` | `magnetita/docs/plans/active/2026-09-13-app-design.md` | RS-5, RS-13 |
| P-15 | `SID-H1-C` | `siderita/docs/plans/active/2026-09-26-hardening.md` | RS-1, RS-4 |
| P-17 | `SURF-1-F` | `celestina/docs/plans/active/2026-08-20-persistent-carriers.md` | RS-1, RS-6, RS-18 |
| P-18 | `HEM-H1-B` | `hematita/docs/plans/active/2026-09-26-hardening.md` | RS-4 |
| P-19 | `GRA-H1-B` | `grafita/docs/plans/active/2026-09-26-hardening.md` | RS-1 |

Unscheduled backlog (Minor; taken when the file is next touched): RS-8, RS-11, RS-12, RS-14, RS-15, RS-16, RS-17.

RS-11 needs the author's decision: ruling R-A4 keeps `dotfiles-core`
and records its missing consumer as an exclusion of
`celestina-rs/docs/plans/active/2026-09-26-hardening.md`.

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

Each unit has one prefix. Consumer follow-ups are separate units under their owners' prefixes.

1. **`celestina-core:` Own file-URI parsing, runtime dir and private/bounded state IO.** Add a strict `file_uri → PathBuf` parser, `xdg::runtime_dir()`, a mode-aware `atomic_file::replace` (0600 file, 0700 parents, keep existing mode), `atomic_file::read_bounded`, and a truthful post-rename result, with tests. Covers RS-1 (owner), RS-2 (owner), RS-3 (owner), RS-6 (owner), RS-9. Value: highest; effort M.
2. **`celestina:` Adopt the owners in the shell.** Covers the media cover URI bug with decoded-path publication (RS-1), clipboard history via the private writer (RS-2), the DDC lock and Melibea runtime dir (RS-3), and the settings read through `read_bounded` (RS-6). Value: fixes the live bug and the clipboard exposure; effort S–M.
3. **`celestina-core:` One bounded `.desktop` reader and application scan.** Adds `desktop_entry::read`/`scan`, spec unescaping and a `XDG_DATA_DIRS` fix, and a hermetic order test. Covers RS-4 (owner) and RS-10. Follow-ups: `siderita:` (move `open_with` and `own_icon` off the Qt thread, delegate), `celestina:` (launcher), `hematita:` (per-id lookup).
4. **`magnetita:` Trim Magnetita's dependency and error surface.** zbus on tokio or a recorded approval (RS-5), gating `uniffi/cli` behind a bin-only feature (RS-13), `LinkError::source` with typed quinn errors (RS-12, link part), and the runtime-dir, private-key and peer-config adoption from unit 1 (RS-2, RS-3). Effort S–M.
5. **`celestina-core:` Make cancellation and path keys say what they do.** A non-blocking cancellation query, a named blocking checkpoint with a `Condvar`, pause-after-cancel handling, and a canonical `pathkey` round-trip check. Covers RS-7 and RS-8. Effort S.
6. **`celestina-rs:` Workspace hygiene and documentation truth.** A true MSRV and its check (RS-14), centralised crypto and archive dependencies, justifications and license inheritance (RS-15), the `dotfiles-core` decision (RS-11), the written error convention (RS-12, convention part), and the README/AGENTS/STATUS corrections (RS-19). Effort S.
7. **`celestina-shell-core:` Journal and crate-doc tidy-up.** A one-byte tail check and condvar close with a shared golden fixture (RS-17), removal of inert `#[allow]`s (RS-18), the text-bounding decision recorded or extracted (RS-16), and the corrected `lib.rs` module doc (RS-19, shell-core part). Effort S.
