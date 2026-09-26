# Evidence: the Hematita and Grafita audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): Hematita (`hematita/`, `hematita-core`) and Grafita (`grafita/`, `grafita-core`, including the document importer Siderita embeds), on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Audit HG: Hematita (system monitor and storage analyzer) and Grafita (text editor)".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
python3 scripts/agent-context.py hematita
python3 scripts/agent-context.py grafita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
cargo test -p hematita-core --offline
cargo test -p grafita-core --offline --no-fail-fast
```

GRA-1 and GRA-2 were reproduced with a probe binary in a scratchpad
crate against a debug build of `grafita-core`. QML was reviewed by
reading and `rg` only.

## Result

- **Exit:** the three guards printed OK; `hematita-core`: 94 unit and 11 capture tests passed and 22 of 28 `usage_tree` tests passed, the six failures being root-only (HEM-15); `grafita-core`: everything passed except one root-only failure in `tests/documents.rs` (GRA-7); the probe reproduced GRA-1 and GRA-2.
- **Observed:** 25 findings: 2 Critical, 8 Important, 15 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Checkout: `main` at `9d022dd`, read-only. Context read with `python3 scripts/agent-context.py hematita` and `... grafita`.

**Scope note.** The dispatch called Grafita "the image viewer". It is not one: Grafita is the suite's text editor (`grafita/README.md`), and `grafita-core` is also compiled into Siderita's embedded editor. The code never decodes images. Its untrusted-input surface is the **document importer** (`grafita-core/src/import/`: PDF, ZIP-based `.docx`/`.odt`/`.epub`, RTF and gzip). That importer is what this audit checked in place of "image decoding".

### Executive summary

1. **Healthy.** The guarded permanent deletion is careful work: it walks descriptors with `O_NOFOLLOW` and removes with `unlinkat`, does a dry pass before removing anything, re-checks dev/ino on every opened folder, reports partial results, and bounds its depth.
2. **Healthy.** The one-shot polkit path follows ADR 0010: a fixed `pkexec /usr/bin/kill -SIG PID` shape, run on a worker thread, with typed outcomes. The Hematita hub's generation and epoch tickets are consistent throughout.
3. **Healthy.** All three guards pass (architecture, language, documentation). The QML follows the token, `reducedMotion` and focus rules. `hematita-core` has broad unit tests.
4. **Top risk, Critical.** A crafted PDF of about 20 KB **aborts the process**. The PDF lexer recurses without bound, which overflows the stack, and it has several unchecked slice indexes, which panic under `panic = "abort"`. Siderita's `Space` preview runs the same core in-process, so the file manager dies too. I verified this with a probe binary.
5. **Top risk, Critical.** Decompression is unbounded (gzip, ZIP members, PDF `FlateDecode`). A 1 MiB `.gz` was accepted as a 1 GiB editable document. The 64 MiB limit applies only to the compressed file, so a 64 MiB file can inflate to about 64 GiB, which means out-of-memory.
6. **Top risk, Hematita correctness.** Mount boundaries are matched by lexical path. A root handed in by CLI, D-Bus or Siderita through a symlinked ancestor is never canonicalised. When that happens, a same-device bind mount below the root is walked and can be deleted through.
7. **Top risk, Hematita performance.** Each duplicate verdict rebuilds three O(tree) vectors and republishes the entire page. The page projects every child of a folder with no row cap, and it re-implements `hematita-core::usage::view::children_rows`, which Siderita uses with `MAX_ROWS`.
8. **Grafita performance.** The C++ highlighter converts UTF-8 to UTF-16 per run, which is quadratic on long lines. PDF object streams are re-inflated on every object lookup.
9. **Qt-thread IO.** `DocumentSession::receive` reads and atomically rewrites the recent list (two fsyncs) on the GUI thread of both Grafita and Siderita. Hematita's `ACTION_TIMEOUT` is still inert, and the code comment still claims it is bounded.
10. **Documentation and tests.** STATUS files are stale: Hematita's "current checkout truth" says it builds at 0.6.0, and Grafita's says 1.2.0. Seven tests fail when run as root.

Tests run:
- `cargo test -p hematita-core --offline`: 94 unit and 11 capture tests pass; 22 of 28 `usage_tree` tests pass, and the 6 failures are root-only.
- `cargo test -p grafita-core --offline --no-fail-fast`: everything passes except one root-only failure in `tests/documents.rs`.
- All three guards: OK.

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| GRA-1 | Critical | 1 Correctness/unsafe | `celestina-rs/crates/grafita-core/src/import/pdf/object.rs:184-194` | A hostile PDF aborts Grafita and Siderita: unbounded recursion plus unchecked slices under `panic=abort` | M | `grafita-core:` |
| GRA-2 | Critical | 1 Unbounded input | `celestina-rs/crates/grafita-core/src/import/gzip.rs:71-72` | Decompression bombs: gzip, ZIP and PDF Flate inflate without a ceiling; 1 MiB became 1 GiB | S | `grafita-core:` |
| HEM-1 | Important | 3 Performance / 4 Reuse | `hematita/src/analysis_session.rs:549-594` | Storage view publishes every child unbounded and duplicates the core `children_rows` projection | M | `hematita:` |
| HEM-2 | Important | 3 Performance | `hematita/src/analysis_workers.rs:157-164` | Each duplicate verdict does O(tree) re-marking plus a full republish, with no coalescing | M | `hematita:` |
| HEM-3 | Important | 1 Correctness / 2 Security | `celestina-rs/crates/hematita-core/src/usage/walk.rs:180-181` | Same-device mounts detected by lexical path against a non-canonical root; walk and delete can cross a bind mount | M | `hematita-core:` |
| HEM-4 | Important | 1 Unbounded input / 3 Performance | `celestina-rs/crates/hematita-core/src/usage/walk.rs:182-201` | Scan arena has no entry or memory cap (only `u32::MAX`), about 150 B per entry | L | `hematita-core:` |
| HEM-5 | Important | 1 Correctness / 7 Docs | `hematita/src/services.rs:31-37,259-262` | `ACTION_TIMEOUT` is inert; the comment says "still bounded"; the known item has no roadmap owner | S | `hematita:` |
| GRA-3 | Important | 3 Performance / 1 Unbounded | `celestina-rs/crates/grafita-core/src/import/pdf/file.rs:129-141,266-268` | Every in-stream object lookup re-inflates and re-lexes its whole object stream; `/N` sizes a `Vec` directly | S | `grafita-core:` |
| GRA-4 | Important | 3 Performance / 4 Reuse | `grafita/cpp/highlighter.cpp:83-87,158-160` | Quadratic UTF-8 to UTF-16 conversion per run on the GUI thread; a second owner of the UTF-16 mapping | M | `grafita:` |
| GRA-5 | Important | 1 Thread affinity | `celestina-rs/crates/grafita-core/src/session.rs:777-779,789-791` | Recent-list read plus atomic replace (2 fsyncs) runs on the host GUI thread in Grafita and Siderita | M | `grafita-core:` |
| HEM-6 | Minor | 1 Correctness | `hematita/src/sampler.rs:588-599` | Any D-Bus error reply is typed `Malformed` and keeps the connection; a real shape mismatch drops it | S | `hematita:` |
| HEM-7 | Minor | 1 Correctness | `hematita/src/sampler.rs:783-801` | Per-PID facts (uid, name, app) cached for the process's life ignore exec, setuid and scope moves | S | `hematita:` |
| HEM-8 | Minor | 1 Correctness / 3 Performance | `celestina-rs/crates/hematita-core/src/usage/duplicates.rs:68-79` | Duplicate candidates keyed by allocated blocks instead of `st_size` | S | `hematita-core:` |
| HEM-9 | Minor | 1 Unbounded input | `celestina-rs/crates/hematita-core/src/usage/duplicates.rs:194-199` | Content check opens by path, following links, with no nonblock and no dev/ino check; a FIFO parks the detached thread forever | S | `hematita-core:` |
| HEM-10 | Minor | 3 Performance | `celestina-rs/crates/hematita-core/src/usage/tree.rs:128-130` | Pruning k siblings is O(k·n); selection toggling is O(n·k) | S | `hematita-core:` |
| HEM-11 | Minor | 8 Quick win | `hematita/src/sampler.rs:784-786,846-848` | Whole `/proc/PID/cmdline` read for argv[0]; a `String` key allocated per process per tick | S | `hematita:` |
| HEM-12 | Minor | 1 Thread affinity / 7 Docs | `hematita/src/sampler.rs:381-395` | `stop()` joins on the Qt thread, documented as 100 ms, but a service tick can block about 4 s | S | `hematita:` |
| HEM-13 | Minor | 7 Docs | `hematita/STATUS.md:167,229` | STATUS says 0.6.0 / 0.4.0 / VAL-H1 pending; README says core has "no IO"; zbus justification stale | S | `hematita:` |
| HEM-14 | Minor | 4 Reuse | `hematita/qml/components/StoragePage.qml:95` | `bytesText` copied four times with divergent behaviour (TiB, `qsTr`) | S | `hematita:` |
| HEM-15 | Minor | 5 Tests | `celestina-rs/crates/hematita-core/tests/usage_tree.rs:115-119` | 6 walk and duplicate tests fail as root; the existing `running_as_root()` guard is not applied | S | `hematita-core:` |
| HEM-16 | Minor | 4 Reuse | `hematita/src/activation.rs:113-170`, `grafita/src/activation.rs:113-127` | Single-instance hand-off recipe copied with divergent path and error rules; lossy UTF-8 paths | M | `celestina-core:` |
| GRA-6 | Minor | 3 Performance | `grafita/qml/components/DocumentView.qml:248` | Every keystroke sends the whole document text through the bridge and scans it in O(n) | L | `grafita:` |
| GRA-7 | Minor | 5 Tests | `celestina-rs/crates/grafita-core/tests/imported.rs` | No hostile or malformed input tests for PDF, ZIP or gzip; `documents.rs:548` fails as root | S | `grafita-core:` |
| GRA-8 | Minor | 1 QObject lifetime | `grafita/cpp/highlighter.h:40,64` | `m_target` is a raw pointer to a QML-owned `QQuickTextDocument` | S | `grafita:` |
| GRA-9 | Minor | 7 Docs | `grafita/STATUS.md:3,29` | STATUS updated 2026-09-07 and says "Grafita is 1.2.0 and installed"; the checkout is 1.2.4 | S | `grafita:` |

---

### GRA-1: A hostile PDF aborts Grafita and Siderita (Critical)

**Evidence.**
- `import/pdf/object.rs:184-194`: arrays recurse into `self.object()` with no depth bound. Dictionaries do the same at `object.rs:333`.
  ```rust
  b'[' => { self.cursor += 1; let mut items = Vec::new(); loop { ... items.push(self.object()?); } ...
  ```
- `import/pdf/file.rs:317-318`: a cross-reference entry that is not UTF-8 becomes `""`, and slicing `""` panics.
  ```rust
  let text = std::str::from_utf8(entry).unwrap_or("");
  let offset = text[..10].trim().parse::<usize>().unwrap_or(0);
  ```
- `import/pdf/object.rs:161`: `if self.bytes[self.cursor..].starts_with(keyword)` panics when `startxref` points past the end of the file.
- More unchecked slices of the same kind: `file.rs:171` (`&self.bytes[start..end]`, where start can exceed end once `start + length` wraps), `file.rs:215` (`self.bytes[start..]`), `file.rs:303`, and `file.rs:397` (`content[at..at + width]`, where the xref-stream `/W` widths come from the file).
- All three apps build with `panic = "abort"` (`grafita/Cargo.toml:38`, `siderita/Cargo.toml:69`), so any of these panics ends the process.

**Reproduced.** I built a scratch probe against `grafita-core` (it lives in the scratchpad; nothing in the repo was touched) and called `Imported::open` on a thread with the default stack, as `DocumentWorker` does:
- `%PDF-1.4\n1 0 obj ` followed by 20 000 `[` → `thread has overflowed its stack` / `fatal runtime error: stack overflow, aborting` (exit 134). With 200 000 `[` it aborts the same way.
- 18 `0xFF` bytes in an xref entry → panic at `file.rs:318:30` (`byte index 10 is out of bounds`).
- `startxref 999999` in a 32-byte file → panic at `object.rs:161:22`.

**Why it matters.** `classify` treats any `%PDF-` prefix as `ImportedDocument`, which `is_editable()`. Siderita's `request_preview` (`siderita/src/editor.rs:228-233`, triggered by `Space`) runs `DocumentSession::open` → `Job::Open` → `Imported::open` inside the file manager. A single downloaded PDF can therefore crash Siderita on `Space`, and crash Grafita on open. A stack overflow cannot be caught at all. This breaks "treat filesystem input as hostile and bounded" and the no-panic rule.

**Fix.** Give `Lexer::object` a nesting depth limit (for example 64) that returns `PdfError::Malformed`. Replace every `bytes[a..]` / `[a..b]` / `text[..10]` with `get(..)` plus a typed error. Use `checked_add` for `start + length`. Add negative tests (see GRA-7).

**Effort:** M. **Prefix:** `grafita-core:`.

### GRA-2: Decompression bombs (Critical)

**Evidence.**
- `import/gzip.rs:71-72`: `GzDecoder::new(bytes).read_to_end(&mut inside)`, with no ceiling.
- `container.rs:147-149`: `Vec::with_capacity(member.uncompressed_size as usize)` pre-sizes from the attacker's header (up to 4 GiB), then `DeflateDecoder::new(data).read_to_end(&mut out)` has no ceiling either.
- `import/pdf/file.rs:185-188`: `ZlibDecoder::new(...).read_to_end(&mut out)` runs once per filter in a `/Filter` array, so repeated filters multiply the expansion.
- `open.rs:275-281,300-305`: `max_bytes` (64 MiB) is only checked against the **raw** file. The final `fs::read` (`open.rs:283`) is also unbounded if the file grows after the `stat`.

**Reproduced.** A 1 042 933-byte gzip of `'a'` × 1 GiB made `Imported::open` return `Ok`: a 1 GiB editable document, after 32 s of inflation in a debug build. A 64 MiB input scales to roughly 64 GiB, which means an out-of-memory abort or the OOM killer. Siderita's `Space` preview is again in-process.

**Fix.** Read every decoder through `Read::take(limit + 1)` and refuse with a typed `TooLarge` when output exceeds `Limits::max_bytes`. Cap the total across members and filters. Stop trusting `uncompressed_size` for `with_capacity` (use `min(declared, limit)`). Read the file itself with `take(max_bytes + 1)`.

**Effort:** S. **Prefix:** `grafita-core:`.

### HEM-1: Unbounded analysed rows, and a second copy of the core row projection (Important)

**Evidence.**
- `hematita/src/analysis_session.rs:549-594`: `view()` takes `tree.children_by_size(self.current)` (a clone and sort) and pushes **every** child into 13 parallel lists.
- `hematita/qml/components/StoragePage.qml:111-140`: `weaveAnalysed()` then builds a JS object for every row, with two `bytesText` calls and `toLocaleString` each, on every `revision`.
- The core already owns this projection with a cap: `celestina-rs/crates/hematita-core/src/usage/view.rs:16,39` (`MAX_ROWS: usize = 40`, `children_rows(..., limit)` with a merged remainder row). Siderita uses it (`siderita/src/usage_session.rs:244`).
- The module header (`usage/mod.rs`) says the view is "shared by every consumer of the tree", and STATUS S3 says "the usage projection lives in `hematita-core::usage::view`". Hematita itself only takes `flat_rects` and `unreadable_below` from it.

**Why it matters.** A folder with 100k entries (a maildir, `node_modules`, a photo dump) publishes 100k-element lists and re-weaves them in JS on every publish; with HEM-2 that happens hundreds of times in a row. Two projections of the same folder can also disagree: the treemap merges slivers while the list does not.

**Fix.** Build the analysed rows from `children_rows(tree, current, &unreadable, MAX_ROWS)` and apply the duplicate/empty filters on top of it (or add a filter parameter in core). Delete the local loop.

**Effort:** M. **Prefix:** `hematita:`.

### HEM-2: Verdict storm, O(tree) work and a full republish per group (Important)

**Evidence.**
- `analysis_workers.rs:157-164`: `apply_verified` → `session.set_verdict` → `self.publish()`.
- `analysis_session.rs:313-320` → `mark_duplicates()` (`:205-228`) allocates `vec![0_u32; tree.nodes.len()]` plus two `marks_*` vectors of `nodes.len()`, and clones every group row.
- `publish()` (`analysis.rs:639-711`) then recomputes `view()`: sort, squarify, and, when the filter is on, `tree.path_of(id)` for **every member of every listed group** (`analysis_session.rs:602-609`). It also writes about 40 properties.
- `spawn_confirm` queues one result per group (`usage_worker.rs:187-207`); there are up to `SHOWN_GROUPS = 500` groups.

**Why it matters.** Small groups verify in microseconds, so hundreds of full republishes land back to back on the Qt thread. With a 2M-node tree, each one allocates and clears about 24 MB and rebuilds every QML list, and the window stalls. This breaks "bursty sources are bounded or coalesced".

**Fix.** Coalesce verdicts: the worker batches results per 100–250 ms, as the scan's `PROGRESS_INTERVAL` already does, or the hub defers `publish` with a single-shot timer. Update the marks incrementally for the members of the changed group instead of rebuilding N-sized vectors.

**Effort:** M. **Prefix:** `hematita:`.

### HEM-3: Mount boundaries matched by lexical path against a non-canonical root (Important)

**Evidence.**
- `usage/walk.rs:180-181`: `let other_device = kind == Kind::Dir && (meta.dev() != device || boundaries.contains(&path));`. Here `path` is `root.join(...)`.
- `usage/remove.rs:229` and `:346`: `boundaries.contains(&target)` / `boundaries.contains(&path)`, again on paths built from `within`.
- `hematita/src/actions.rs:182` does the same.
- The boundaries come from `/proc/self/mountinfo` targets, which are canonical.
- The root is never canonicalised:
  - `hematita/src/main.rs:41-47` uses `std::path::absolute` (purely lexical);
  - D-Bus `Open` (`activation.rs:81-88`) → `open_path` (`analysis.rs:510-517`) pushes the string as given;
  - Siderita hands off its current path.

**Why it matters.** A bind mount of a directory on the same filesystem has the same `st_dev`, so the path is the only thing that marks it. The author's own mountinfo fixture has one (`/srv/data` bound from `/@home/toni/data`). Suppose the scanned root is reached through a symlinked ancestor: `/home` → `/var/home` on atomic distros, or any symlinked folder in Siderita. Then the walk descends into the bind mount (double counting), and `delete_tree`'s dry pass does not refuse it, so a permanent deletion removes files that live elsewhere on the filesystem. That breaks the AGENTS rule "refusing an inner mount before anything is removed". It is low-probability but causes data loss.

**Fix.** Recognise mounts by mount identity, not by path: `rustix::fs::statx(..., StatxFlags::MNT_ID)` on each opened folder, compared with its parent's `stx_mnt_id`, in both the walk and `open_folder`. Alternatively (or additionally), canonicalise the root on the scan worker and store the canonical path in `Tree::path`.

**Effort:** M. **Prefix:** `hematita-core:`, plus a small `hematita:` follow-up if the root is canonicalised.

### HEM-4: The scan arena has no cap (Important)

**Evidence.**
- `walk.rs:182-201`: one `Node` per entry. Each holds an `OsString` name, a `Vec` of children, 4×`u64` totals, dev and ino: about 110 B inline plus heap, roughly 150 B per entry in all.
- The only stop is `TooManyEntries` at `u32::MAX` (`walk.rs:182-187`).
- `findings()` adds `Vec<u32>` and several `Vec<bool>` of `nodes.len()`.
- `Arc::make_mut` clones the whole tree when a prune meets a running confirm (the residual STATUS already records).
- A cancelled scan's detached thread keeps its partial arena until its next check.

**Why it matters.** `/` is offered as a location. Ten million entries means about 1.5 GB before findings and marks, with no refusal and no memory signal. This is unbounded filesystem input.

**Fix.** Add a configurable entry ceiling (for example 20M) that fails with a typed `TooManyEntries` naming the count. Consider a compact node: names in one byte arena with offsets, children as index ranges built after the walk.

**Effort:** L. **Prefix:** `hematita-core:`.

### HEM-5: `ACTION_TIMEOUT` is inert, the comment says otherwise, and nothing tracks the fix (Important)

**Evidence.**
- `hematita/src/services.rs:36-37`: "Five minutes ... it is still bounded — a call that never answers is not a thread left forever".
- `services.rs:246,259-262`: `builder.method_timeout(ACTION_TIMEOUT)` then `proxy.call_with_flags(...)`.
- In zbus 5.18, `Proxy::call_with_flags` (`proxy/mod.rs:863-893`) awaits `call_method_raw` directly. Only `Connection::call_method` applies `method_timeout` (`connection/mod.rs:269-270`).
- `hematita/STATUS.md:463-470` records this as "booked for the next checkpoint", but `hematita/ROADMAP.md:3-4` is `idle` / `none`, with no unit.

**Why it matters.** An unanswered polkit prompt parks the `hematita-systemd` thread and its bus connection until the process exits. The code comment asserts a guarantee that does not hold.

**Fix.** Bound the wait yourself: run the call on the worker and `recv_timeout(ACTION_TIMEOUT)` on a channel, dropping the connection on expiry. Or use the async proxy wrapped in `async_io::timeout`. Correct the comment and add the item to ROADMAP.

**Effort:** S. **Prefix:** `hematita:`.

### GRA-3: PDF object streams re-inflated per lookup, and `/N` trusted (Important)

**Evidence.**
- `import/pdf/file.rs:129-141`: `Location::InStream` → `self.object_stream(container)?` on **every** `object(number)` call. `Pdf` caches nothing: its only fields are bytes, locations, trailer, `start_xref` and `xref_streams`.
- `object_stream` (`:244-280`) inflates the container and lexes all of its objects each time.
- `file.rs:266-268`: `let count = ...as_number().unwrap_or(0.0) as usize; ... Vec::with_capacity(count)`. A huge `/N` saturates to `usize::MAX`, which panics with "capacity overflow" (process abort).

**Why it matters.** Most modern PDFs keep nearly every object in object streams. Text extraction resolves pages, fonts and `ToUnicode` maps one object at a time, so the cost is O(objects × stream size) of inflate and lex work, on legitimate files as well as hostile ones.

**Fix.** Memoise decoded object streams (`RefCell<HashMap<u32, Rc<Vec<Object>>>>` or an eager decode during `parse`). Cap `count` by the stream length.

**Effort:** S. **Prefix:** `grafita-core:`.

### GRA-4: Quadratic highlighting on long lines, and a second UTF-16 mapping (Important)

**Evidence.**
- `grafita/cpp/highlighter.cpp:83-87`:
  ```cpp
  const int clamped = qMin<int>(static_cast<int>(byteOffset), utf8.size());
  return QString::fromUtf8(utf8.constData(), clamped).size();
  ```
  This decodes the line's prefix from the start. `:158-160` calls it twice per run.
- Four colour setters each call `rehighlight()` (`:117-143`), so the palette injection at start-up highlights the whole document four times.
- `grafita-core/src/display.rs:69-90` states it is "the only correct way" to map UTF-16 offsets, and STATUS says the core owns the UTF-16 mapping.

**Why it matters.** A 5 MB single-line minified JS or JSON file (well under the 64 MiB limit) has hundreds of thousands of runs, so roughly 10^12 byte decodes on the GUI thread, and the window freezes. The offset rule also gains a second owner in C++.

**Fix.** Have `grafita_colour_line` return UTF-16 `start`/`len` computed in Rust in one forward pass (or convert in a single incremental walk in C++). Coalesce the four palette setters, for example into one `setPalette` call or one deferred `rehighlight`.

**Effort:** M. **Prefix:** `grafita:` (the Rust side lives in `grafita/src/syntax.rs`).

### GRA-5: Recent-list and preferences IO on the GUI thread (Important)

**Evidence.**
- `grafita-core/src/session.rs:777-779` (open succeeded) and `:789-791` (open refused): `Recent::load(); recent.record(..); recent.store();`. These run inside `DocumentSession::receive`, which both hosts call on the Qt thread when a completion is queued back.
- `store()` → `atomic_file::replace` does a file `sync_all` plus a directory `sync_all` (`celestina-core/src/atomic_file.rs:19-24`).
- `session.rs:221-223` `recent_documents()` → `Recent::existing()` → `path.is_file()` for each entry (`recent.rs:72-76`), called from QML (`DocumentView.qml:383`) on every empty state.
- `grafita/src/preferences.rs:107` `stored.store()` (another fsync) runs on every Ctrl+wheel notch. `siderita/src/preferences.rs:122` does the same.

**Why it matters.** Two fsyncs per opened document, including every Siderita `Space` preview, stall the GUI on a busy disk. A `stat` of a recent path on a dead sshfs or NFS mount can hang the window. This breaks "Blocking IO never runs on the Qt thread".

**Fix.** Emit the recent-list change as a `Job` for the existing `DocumentWorker` (or a small writer thread). Resolve `existing()` on the worker and publish the result. Debounce preference writes off-thread.

**Effort:** M. **Prefix:** `grafita-core:` (the Siderita and Grafita adapters follow in their own units if their signatures change).

### HEM-6: D-Bus error replies mis-typed (Minor)

**Evidence.** `hematita/src/sampler.rs:588-599` maps `Err(zbus::Error::MethodError(..))` to `ReasonKind::Malformed` and keeps the connection. The comment reads: "The manager replied, and what it replied was not what this asked for". In zbus, `MethodError` is **any** D-Bus error reply (`zbus-5.18/src/error.rs:262-280`), for example `ServiceUnknown` when no user manager runs, or `AccessDenied`. A genuine shape mismatch is `zbus::Error::Variant`, which falls into the `_` arm: "bus unavailable", and a healthy connection is dropped every 5 s.

**Fix.** Map `Variant`/`InvalidReply` to `Malformed` and keep the connection. Map `MethodError` to unavailable (or a new `refused` reason). Add tests on the error mapping.

**Effort:** S. **Prefix:** `hematita:`.

### HEM-7: Stale per-PID facts (Minor)

**Evidence.** `hematita/src/sampler.rs:783-801`: `facts` (name from `cmdline`, `uid`, cgroup `application`) are computed once per `(pid, start_ticks)` and reused while the process lives. `status` is parsed every tick (`:771-776`), but its `uid` is ignored after the first sight. `processes.rs:538` gates signals on `reading.uid` from these facts.

**Why it matters.** After `exec` of a setuid binary, a `setuid()` drop, or a launcher moving a PID into its `app-*.scope`, the table keeps the old owner, name and application. The signal path then refuses: the live identity re-read no longer matches the stale snapshot uid. The privileged path is also never offered, because the row still looks like the user's own.

**Fix.** Take `uid` from the current tick's `status`. Refresh `name` and `application` when `stat.comm` changes, or every N ticks.

**Effort:** S. **Prefix:** `hematita:`.

### HEM-8: Duplicate candidates keyed by allocated blocks (Minor)

**Evidence.** `usage/duplicates.rs:68-79`: `by_size.entry(node.allocated)` groups by `st_blocks × 512`. The test `candidates_group_files_of_equal_allocation` pins this. Equal content implies an equal `st_size`, not equal blocks: sparse copies and preallocated files are missed. Conversely, every file of 1–4096 bytes lands in one 4096 group; the UI shows its member count as "N copias" (`StoragePage.qml:133-135`), and verifying it reads every member.

**Fix.** Key by `apparent` (`st_size`), excluding 0. Keep allocated only for ordering by space freed.

**Effort:** S. **Prefix:** `hematita-core:`.

### HEM-9: Content check can block forever (Minor)

**Evidence.** `usage/duplicates.rs:194-199` `File::open(path)` follows symlinks and opens FIFOs blocking. Nothing checks that the scanned dev/ino still holds. `usage_worker.rs:189-193` holds a strong `Arc<Tree>` for the whole group. The threads are detached (`usage_worker.rs:10-17`).

**Why it matters.** An entry replaced after the scan by a FIFO parks `hematita-confirm` in `open(2)` forever, where cancellation cannot reach it. Its strong `Arc` then makes every later prune clone the whole tree (`Arc::make_mut`). An entry replaced by a symlink to `/dev/zero` reads until cancelled.

**Fix.** Open with `O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` via `rustix::fs::open`, `fstat`, and require a regular file with the recorded dev/ino before reading. Drop the strong `Arc` before IO by copying the member paths first.

**Effort:** S. **Prefix:** `hematita-core:`.

### HEM-10: Batch prune and selection are quadratic (Minor)

**Evidence.**
- `usage/tree.rs:128-130`: `parent.children.retain(|child| *child != id)` runs once per pruned id (loop at `analysis_session.rs:402-404`), so trashing k of n siblings costs O(k·n) on the Qt thread.
- `analysis_session.rs:367-371` (`position` per toggle) and `:385-388` (`selection.contains` per id) are O(n·k) for large "all but one" selections.

**Fix.** Add a `Tree::prune_many(&[NodeId])` that retains against a `HashSet` once per parent. Keep the selection as an `IndexSet` or a `HashSet` beside the ordered `Vec`.

**Effort:** S. **Prefix:** `hematita-core:` (hematita adopts it).

### HEM-11: Unbounded cmdline read and per-tick allocations (Minor, quick win)

**Evidence.** `hematita/src/sampler.rs:784-786` reads `std::fs::read(dir.join("cmdline"))` whole; only `cmdline.first()` is used (`display_name`, `:853-857`), and a cmdline can be megabytes. `:846-848` `io_key` formats a `String` per own process per tick.

**Fix.** Read through `File::take(4096)` up to the first NUL. Key IO counters by `(u32, u64)`.

**Effort:** S. **Prefix:** `hematita:`.

### HEM-12: `sampler::stop` can freeze shutdown (Minor)

**Evidence.**
- `hematita/src/sampler.rs:381-395` documents "The thread checks the flag every hundred milliseconds, so this waits that long at worst", and `stop()` joins on the Qt thread (`resources.rs:174`, `Main.qml:260`).
- A service tick runs two `ListUnits` calls with `LISTING_TIMEOUT = 2 s` each (`:71`), and `Builder::system().build()` (`:614-619`) has no timeout at all.

**Fix.** Check `stop` between the two buses, give connection setup a deadline, and correct the comment; or detach the join from the Qt thread.

**Effort:** S. **Prefix:** `hematita:`.

### HEM-13: Stale documentation (Minor)

**Evidence.**
- `hematita/STATUS.md:167`: "The project is registered and builds a release binary at `0.6.0` ... Nobody has looked at any page on a real session yet (`VAL-H1` through `VAL-H5` all pending)". The same file says VAL-H1 passed; the version is `1.2.2` (`hematita/Cargo.toml:3`).
- `:229`: "installed at `~/.local/bin/hematita` at `0.4.0`".
- `hematita/README.md:25`: hematita-core has "no Qt, no IO". `usage::walk`, `remove` and `duplicates` do IO, and AGENTS.md says so.
- `README.md:13-14`: polkit "arrives ... in H5" (it was delivered).
- `hematita/Cargo.toml:32`: zbus is justified only by single-instance activation, yet it also drives the systemd listing and actions.

**Fix.** Rewrite the "Current checkout truth" head to the 1.2.2 state and fix the three lines.

**Effort:** S. **Prefix:** `hematita:`.

### HEM-14: Byte formatter copied four times (Minor)

**Evidence.**
- `hematita/qml/components/StoragePage.qml:95` has TiB, and `qsTr("%1 B")`.
- `ProcessTable.qml:73` and `PerformancePage.qml:130` have no TiB, and a bare `Math.round(bytes) + " B"`.
- `siderita/qml/dialogs/FolderUsage.qml:46` is a copy of Hematita's storage version.

**Why it matters.** It is the same rule with divergent output: a 2 TiB process rate reads "2048.0 GiB", and one copy translates "B" while two do not. The QML also holds a presentation rule that has three owners.

**Fix.** One owner inside Hematita (a single `pragma Singleton` helper, or unit tokens from the adapter). Promote it to `celestina-style` only under the sharing contract once Siderita's semantics match.

**Effort:** S. **Prefix:** `hematita:`.

### HEM-15: Tests that fail as root (Minor)

**Evidence.**
- `tests/usage_tree.rs:115-119` defines `running_as_root()` because "Root reads through a mode-000 directory". Six tests do not use it and fail under uid 0, with the `locked/secret` fixture counted:
  - `the_walk_counts_files_once_and_follows_no_link` (`:151`)
  - `candidates_group_files_of_equal_allocation` (`:288`)
  - `confirm_keeps_only_identical_content_together` (`:313`)
  - `progress_is_reported_on_a_large_folder` (`:211`)
  - `a_mount_boundary_is_a_leaf_even_on_the_same_device` (`:567`)
  - `only_a_second_name_of_a_linked_file_is_counted_as_a_hard_link` (`:790`)
- Observed in this container, where uid is 0.

**Fix.** Early-return or branch on `running_as_root()` in those tests, as the unreadable tests already do.

**Effort:** S. **Prefix:** `hematita-core:`.

### HEM-16: The single-instance hand-off recipe diverges between apps (Minor)

**Evidence.**
- `hematita/src/main.rs:41-47` makes the path absolute with `std::path::absolute` (lexical) and sends `to_string_lossy`. `hematita/src/activation.rs:148-170` treats only `ServiceUnknown`/`NameHasNoOwner` as "no instance".
- `grafita/src/activation.rs:123-126` uses `std::fs::canonicalize` then `to_string_lossy`, and treats *any* failure as "not accepted" (`:108-111`), so a refused call opens a second instance.
- The serve loop (`serve` + `DoNotQueue` + `park`) is copied as well; `siderita/src/dbus.rs:103-110` is another copy.

**Why it matters.** "Which path do we hand over, and what does a failure mean" has three owners with three answers. Non-UTF-8 paths cannot cross in any of them.

**Fix.** Extract a small `celestina-core` activation helper: canonicalise on the caller's side, send bytes (`ay`) or reject non-UTF-8 explicitly, and give failures one typed classification. Each app keeps its own interface object.

**Effort:** M. **Prefix:** `celestina-core:` (apps adopt it in their own units).

### GRA-6: Whole-document round trip per keystroke (Minor)

**Evidence.**
- `grafita/qml/components/DocumentView.qml:248`: `onTextChanged: root.session.applyText(text)`.
- `grafita/src/session.rs:435-443` converts the whole `QString` → `String`. `display::reconcile` (`grafita-core/src/display.rs:149-179`) scans the common prefix and suffix of the full text.
- `refresh_search` re-scans the buffer (`grafita-core/src/session.rs:902-908`).

**Why it matters.** Several O(n) passes per keystroke. The 64 MiB open limit makes multi-megabyte logs typeable but laggy.

**Fix.** Forward `QTextDocument::contentsChange(position, removed, added)` (a C++ or QML signal) as a positional edit, and keep `reconcile` as the fallback check.

**Effort:** L. **Prefix:** `grafita:`.

### GRA-7: No hostile-input tests for the importer, and one root-sensitive test (Minor)

**Evidence.**
- `tests/imported.rs` covers only well-formed `.docx`, `.odt`, `.epub`, `.rtf`, PDF, forms and gzip. There is no truncated, nested, oversized-`/N`, bad-xref or bomb case, which is how GRA-1 through GRA-3 went unnoticed.
- `tests/documents.rs:548-568` (`a_save_that_cannot_create_its_temporary_leaves_the_original_intact`) relies on a mode-0500 directory and fails as root.

**Fix.** Add a negative table for each format (the GRA-1 and GRA-2 probes are ready-made fixtures), each asserting a typed `Err`. Skip the permission test when euid is 0.

**Effort:** S. **Prefix:** `grafita-core:`.

### GRA-8: The highlighter keeps a raw pointer to a QML-owned object (Minor)

**Evidence.** `grafita/cpp/highlighter.h:64` `QQuickTextDocument *m_target = nullptr;`, exposed through `READ target` (`:40`). The `TextEdit` owns the `QQuickTextDocument`. `QSyntaxHighlighter` guards its own `QTextDocument`, but `m_target` dangles once the `TextEdit` goes first, and a binding that reads `target` gets a freed pointer.

**Fix.** `QPointer<QQuickTextDocument> m_target;` (and emit `targetChanged` on destruction).

**Effort:** S. **Prefix:** `grafita:`.

### GRA-9: Stale STATUS (Minor)

**Evidence.** `grafita/STATUS.md:3` says `Updated: 2026-09-07`, and `:29` says "Grafita is 1.2.0 and installed". `grafita/Cargo.toml:3` and `docs/version-history.tsv` say `1.2.4`. The same section lists 1.2.3 and 1.2.4 above it.

**Fix.** Update the date and the version line.

**Effort:** S. **Prefix:** `grafita:`.

## Limits

- No production build, deploy or QML smoke was run (per the brief). QML was reviewed by reading and grep only; there was no `qmllint` run and no AT-SPI check.
- `siderita-ops::trash` cancellation semantics (used by Hematita's trash) belong to another area and were not audited here.
- The GRA-1 and GRA-2 reproductions ran in a scratchpad probe crate against a debug build of `grafita-core`. Release stack frames are smaller, so the nesting depth that overflows may be higher there; the unchecked-slice panics are independent of the build profile.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-5 | `GRA-H1-A` | `grafita/docs/plans/active/2026-09-26-hardening.md` | GRA-1, GRA-2, GRA-3, GRA-7 |
| P-9 | `HEM-H1-A` | `hematita/docs/plans/active/2026-09-26-hardening.md` | HEM-3, HEM-4, HEM-8, HEM-9, HEM-10, HEM-15 |
| P-18 | `HEM-H1-B` | `hematita/docs/plans/active/2026-09-26-hardening.md` | HEM-1, HEM-2, HEM-5, HEM-6, HEM-7, HEM-11, HEM-12, HEM-13, HEM-14 |
| P-19 | `GRA-H1-B` | `grafita/docs/plans/active/2026-09-26-hardening.md` | GRA-4, GRA-5, GRA-8, GRA-9 |

Unscheduled backlog (Minor; taken when the file is next touched): GRA-6, HEM-16.

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

Ordered by value over effort.

1. **`grafita-core:` Harden the document importer against hostile input** (bug). Bound PDF nesting, replace unchecked slices with typed errors, cap decompressed output and `with_capacity` sizes, memoise object streams, and add a negative-input test table. Covers GRA-1, GRA-2, GRA-3 and GRA-7. Completion runs both Grafita and Siderita.
2. **`hematita:` Coalesce the storage publication and bound its rows** (bug). Batch verdicts, mark incrementally, and project rows through `usage::view::children_rows` (`MAX_ROWS`) with filters. Covers HEM-1 and HEM-2.
3. **`hematita-core:` Recognise mounts by identity and bound the storage readers** (bug). Compare statx mount ids in walk and delete; add an entry ceiling; key duplicates by `st_size`; open members `O_NOFOLLOW|O_NONBLOCK` with an identity check; add a batch prune API; make tests root-safe. Covers HEM-3, HEM-4, HEM-8, HEM-9, HEM-10 and HEM-15.
4. **`grafita-core:` Move recent and preference IO off the GUI thread** (bug). Recent-list write and `existing()` as worker jobs; debounced preference stores. Covers GRA-5.
5. **`grafita:` Colour lines in UTF-16 runs and guard the highlighter** (bug). Return UTF-16 offsets from Rust, coalesce the palette rehighlight, `QPointer` the target, and refresh STATUS. Covers GRA-4, GRA-8 and GRA-9. GRA-6 is a later milestone.
6. **`hematita:` Make the services and sampler honest** (bug). Real `ACTION_TIMEOUT` watchdog, correct D-Bus error typing, live uid/name facts, bounded cmdline read, a shutdown that does not stall, and doc and STATUS truth. Covers HEM-5, HEM-6, HEM-7, HEM-11, HEM-12, HEM-13 and HEM-14.
7. **`celestina-core:` Share the single-instance hand-off** (maintenance). One path and error rule, adopted by each app in its own follow-up. Covers HEM-16.
