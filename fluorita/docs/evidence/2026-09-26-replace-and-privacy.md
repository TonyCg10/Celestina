# Evidence: a Replace that keeps its original, and bounded file claims

- **Date:** 2026-09-26
- **Scope:** `FLU-H1-A` (program unit P-7) of the
  [FLU-H1 plan](../plans/active/2026-09-26-hardening.md): FLU-1, FLU-2, FLU-3,
  FLU-4, FLU-6, FLU-15 and FLU-16 of the
  [Fluorita audit](../../../docs/evidence/2026-09-26-monorepo-audit-fluorita.md),
  program row P-7 of the
  [monorepo audit](../../../docs/evidence/2026-09-26-monorepo-audit.md); it
  adopts the media-landing and bounded-read owners `RS-H1-A` added to
  `celestina-core` ([shared owners](../../../celestina-rs/docs/evidence/2026-09-26-shared-owners.md))
- **Environment:** session worktree `fluorita-FLU-H1-A` on branch
  `unit/fluorita/FLU-H1-A`, stacked on `unit/celestina-rs/RS-H1-A` at
  `14b0e02`; Linux container (kernel 6.18) running as uid 0; rustc and cargo
  1.97.1 from the pinned toolchain; Cargo `--offline` with target directories
  in the session scratchpad (see Limits for why not the shared one); no Qt 6
  SDK or CXX-Qt build, no libmpv, no Wayland session, no desktop Trash
  service and no AT-SPI bus
- **Artifact:** the landing builds it (Fluorita `complete-production.sh`, and
  Siderita and Magnetita, which link the Fluorita crates)

## Procedure

```sh
cd celestina-rs
cargo test -p fluorita-core -p celestina-core --offline
cargo clippy -p fluorita-core -p fluorita-engine -p celestina-core --all-targets --offline -- -D warnings
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo fmt --all -- --check
cargo doc -p fluorita-core -p fluorita-engine --no-deps --offline
cd ../fluorita && cargo fmt -- --check && cd ..
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

`fluorita-engine` links the system `libmpv`, which this container lacks, so
its unit tests were linked against a stand-in: a shared library built in the
session scratchpad whose 54 `mpv_*` functions (every function name in
`libmpv2-sys` 4.0.1's pregenerated bindings) call `abort()`. The engine's lib
test binary was then built and each test run in its own process:

```sh
cc -shared -fPIC -o libmpv.so.2 stub.c -Wl,-soname,libmpv.so.2   # stub.c: void mpv_x(void){abort();} per name
CARGO_TARGET_DIR=<scratch>/engine-target RUSTFLAGS="-L <stub dir>" \
  cargo test -p fluorita-engine --offline --lib --no-run
LD_LIBRARY_PATH=<stub dir> <test binary> --exact <name> --test-threads=1   # once per listed test
```

A test that reaches libmpv aborts in the stub and is reported as such rather
than as a pass. Neither the stub nor the script is part of the unit.

RED was recorded two ways. For the landing, the worker, and XMP and MPF, the
tests were written first and run against the unchanged code. For the
duration and catalogue fixes, which were written first, the finished code was
mutated back to the old behaviour (`Duration::from_secs_f64`, and a load that
returns an empty catalogue without setting the file aside), the tests were
run, and the files were restored from copies.

## Result

- **Exit:** every command above exited 0. `cargo doc` prints the three
  warnings `main` already had (`backend.rs` and `edit_store.rs`), and no new
  one.
- **`fluorita-core`:** 160 passed (158 before; 2 new). `celestina-core`: 82
  passed, unchanged.
- **`fluorita-engine` lib, against the stub:** 138 passed, 0 failed, 4 reached
  libmpv and aborted (116, 0 and the same 4 before the unit; 22 tests new or
  rewritten). The four are
  `engine::tests::a_missing_file_fails_with_a_typed_error_rather_than_a_panic`,
  `frame::tests::a_file_that_is_not_a_film_leaves_nothing_behind`,
  `instance::tests::a_rejected_required_option_is_a_typed_backend_failure` and
  `instance::tests::an_instance_starts_and_answers_properties`.
- **RED, before the fix:**
  - edit and metadata (tests first): `a_turn_on_a_jpeg_is_written_losslessly_and_the_renderer_is_never_asked`
    (the rewritten old `edit.rs:718` assertion; now the original must be in
    the bin), `a_same_format_replacement_keeps_the_originals_mode_and_trashes_it`,
    `a_same_format_replacement_the_trash_refuses_changes_nothing`,
    `a_copy_lands_beside_the_original_and_a_replacement_takes_its_place` and
    `removing_a_location_in_place_keeps_a_private_photographs_mode` failed:
    5 failed, 28 passed.
  - worker (tests first): `a_cancel_right_after_submit_reaches_the_job_it_followed`
    got no outcome within 5 s (the job ran to its end) and
    `cancelling_reaches_a_queued_job_and_spares_a_later_one` failed: 2 failed,
    5 passed.
  - XMP and MPF (tests first): `a_location_carried_only_in_xmp_is_reported`,
    `removing_a_location_strips_xmp_and_extended_xmp_and_keeps_the_picture`,
    `secondary_images_after_the_primary_are_reported_and_dropped` and
    `every_scan_of_a_progressive_primary_survives_the_strip` failed: 4 failed,
    17 passed.
  - duration and catalogue (mutation): `probe::…::a_crafted_duration_is_unknown_rather_than_fatal`
    and `session::…::a_crafted_position_or_duration_is_dropped_rather_than_fatal`
    panicked at `core/src/time.rs:962` — the conversion panic that aborts the
    release build — and both set-aside tests failed: 4 failed, 16 passed.
  - core (tests first): the duration and tag tests did not compile until
    `duration_from_seconds` and `claimed_tag` existed.
- **GREEN:** every test above passes on the finished code.

### Per finding

- **FLU-1 (Critical).** A new `fluorita-engine` module, `landing`, is the one
  owner of how new bytes reach a person's folder; `edit::save` and
  `metadata::write` both call it, and batch runs go through those two. The
  result is staged and synced through `atomic_file::stage_media`, then a
  replacement sends the original to the Trash through `siderita-ops`, then the
  result is published with `publish_without_replacing`. A replacement that
  keeps the original's name trashes before it publishes (the name is taken
  until then); one under a new name (the container changed) publishes first.
  `Saved.trashed_original` and `MetadataWritten.trashed_original` are now
  `Some` for every replacement. Tests: the in-place edit, retag, EXIF strip and
  a two-item batch replace each find the original's bytes in the bin; the
  landing tests observe the staged sibling in the directory at the moment the
  bin is asked to move the original.
- **FLU-2.** Edits, metadata writes and frame exports no longer use
  `atomic_file::replace`: the replace path stages with `stage_media`, and
  `frame::extract` lands with `land_media` (mode from the film). The result
  takes the source's permission bits and group; a `0600` photograph stays
  `0600` (edit, metadata and landing tests). A name that appeared since the
  free-name search is refused as `WriteError::TargetExists` inside
  `EngineError::Landing`, never overwritten (`a_copy_refuses_a_name_that_appeared_meanwhile`,
  `a_name_taken_after_the_original_left_is_refused_and_the_original_stays_in_the_bin`).
  Owner, ACLs and other extended attributes are still not copied, as the
  `stage_media` documentation states.
- **FLU-3.** `fluorita_core::duration_from_seconds` is the one conversion from
  claimed seconds: NaN, infinities, negatives and anything past
  `MAX_MEDIA_SECONDS` (`u32::MAX` seconds, so two accepted values can never
  overflow a `Duration` sum) return a typed `DurationRejected`. It replaces
  every `Duration::from_secs_f64` on file or surface data: `probe.rs` (the
  probed duration), `session.rs` (`time-pos` in `position_of` and in property
  changes, `duration`), and `fluorita/src/player.rs` (`seek` from QML and the
  frame-extraction position). `rg from_secs_f64` over the Fluorita crates and
  the Fluorita and Siderita sources finds only the helper's own comments.
- **FLU-4.** `fluorita_core::claimed_text` strips control characters, trims and
  keeps at most a given number of characters without allocating the rest;
  `claimed_tag` bounds it by `MAX_TAG_CHARACTERS`. The probe applies it to
  every tag, `streams` delegates its label bound to it, and a stored catalogue
  applies it on load, so a catalogue written before the cap cannot bring an
  unbounded title back. `catalogue_store::load` now reads through
  `atomic_file::read_bounded` and moves a stored file it cannot use (past the
  budget, not UTF-8, not a regular file, unreadable, or an unknown format
  version) to `catalogue.tsv.unreadable` before starting empty; if that move
  fails, `load` errs and the host (`fluorita/src/library/work.rs`) does not
  save over the file for the rest of the run.
- **FLU-6.** `EngineWorker::submit` creates the job's token and registers it
  under the lock that sends the job, and `cancel_current` cancels every
  submitted job not yet finished. A job cancelled before it starts reports
  `EngineError::Cancelled` without reaching the engine. A job submitted after
  the cancel runs.
- **FLU-15.** `strip_jpeg_exif` also removes every XMP packet (standard and
  extended), the MPF index, and everything after the primary picture's
  end-of-image marker, where a phone appends secondary images carrying their
  own EXIF. The primary's scans are copied byte for byte. The walk follows
  stuffed bytes, restart markers, fill bytes and the table segments between
  the scans of a progressive file. `private_facts` reads XMP keys (`:GPS*`
  under any prefix, IPTC and Photoshop location fields, camera and date
  properties) and the headers of up to 16 appended images, so the panel and
  a batch "forget" learn about a location carried only there.
- **FLU-16.** `Redaction` has one style, `Solid`: `FluoritaCanvas::redact`
  fills the area, aligned outwards to whole pixels, with opaque black in
  source composition mode. The pixelate and blur paths are gone. The editor's
  `addRedaction(area)` creates it, and recipes stored with `pixelate` or
  `blur` read back as solid. The app canvas test
  `a_redaction_keeps_nothing_of_the_pixels_it_covers` renders two different
  pictures under a full-area redaction and requires byte-identical PNGs.

## Limits

- **Not run here:** the Fluorita application crate (CXX-Qt and Qt 6 are
  absent), so `fluorita/src/player.rs`, `editor.rs`, `editor/copy.rs`,
  `rasteriser.rs` and `library/work.rs`, the C++ canvas and the two QML edits
  were checked by reading and by `cargo fmt --check` only. The new canvas test
  `a_redaction_keeps_nothing_of_the_pixels_it_covers` needs Qt and has never
  run. `qmllint` did not run.
- **Stubbed libmpv:** the engine's four libmpv tests and its `real_media`
  integration tests did not run. The author's `cargo test -p fluorita-core -p fluorita-engine`
  with the real libmpv is the exit the plan names.
- **Not the shared target directory:** a `cargo clippy` over the shared
  session target directory resolved `celestina-core` to an artifact built
  from another checkout (its dependency information names that checkout's
  files) and reported the `RS-H1-A` owners missing. Every result above
  therefore comes from target directories in this session's scratchpad.
- **The desktop Trash:** the tests use stand-in bins; `DesktopTrash` itself is
  `siderita-ops::trash`, unchanged. Whether the original really appears in the
  desktop's Trash is `VAL-FLU-EDIT` and `VAL-FLU-METADATA`.
- **The panel's prefix:** the metadata panel reads a 4 MiB prefix of a file, so
  a location carried only by a secondary image past that prefix is not
  reported there; batch "forget" reads the whole file, and the strip removes
  every secondary image whatever the panel reported.
- **After the Trash step:** when the original of a same-name replacement is in
  the Trash and publishing then fails (a file appeared under the name in that
  instant, or the filesystem refused the link), the result stays, synced, at
  its hidden name `.<stem>.fluorita-result-<pid>-<n>` (the stem is the
  original's name cut to 180 bytes) beside the original's
  name, and `EngineError::ReplacementNotPublished` names both files; nothing
  moves either one back automatically.
- **Disk and Trash use:** every replaced file keeps a full copy of its
  original in the Trash, so disk use doubles per replaced file until the Trash
  is emptied, and a batch replacement leaves one Trash entry per file.
  Metadata writes cover FLAC and JPEG only, each capped at
  `MAX_CONTAINER_BYTES`; videos are never retagged.
- **After the end-of-image marker:** stripping a location drops everything
  after the primary picture's EOI, including a Motion Photo's MP4, together
  with the XMP that referenced it. Segments between the scans of a
  progressive picture are copied through unread.
- **The probe's copy:** a tag is capped after `libmpv2` copies it out of the
  backend; the safe property API has no bounded read.
- **A truncated JPEG:** a primary picture with no end-of-image marker is
  stripped as before, keeping everything after its first scan. A malformed
  segment between two scans is refused instead of copied.

## Follow-up

- `VAL-FLU-EDIT` and `VAL-FLU-METADATA` check the Trash, the mode and the
  location removal on a real desktop.

## Fix round 1

Review of `80a74dd`. Commands as above; `fluorita-core` 160 passed,
`fluorita-engine` lib against the stub 140 passed with the same 4 libmpv
tests aborting, clippy `-D warnings` and the three guards clean.

- **A result kept after the Trash step.** A same-name replacement now lands
  the result as an ordinary synced file under a hidden name of its own
  (`atomic_file::land_media`), trashes the original, then moves the result
  into the freed name with `publish_without_replacing`; a failed move leaves
  the result at its hidden name, and `ReplacementNotPublished` carries
  `kept`. `celestina-core` is unchanged. RED by mutation (removing the kept
  file on failure): the extended landing test failed; GREEN after restore.
- **Catalogue set-aside.** The file moves to the first free
  `catalogue.tsv.unreadable[-n]` (at most 16) through
  `publish_without_replacing`, never over an earlier copy, and only when it is
  malformed (past the budget, not a regular file, not UTF-8, unknown format);
  a failed read errs and leaves it in place. Tests: two set-asides kept side by
  side, and the read-error classification.
- **Spoofing characters.** `claimed_text` also drops the bidirectional
  embeddings, overrides and isolates, directional marks, zero-width and
  invisible characters, the byte-order mark and the soft hyphen.
- **Extended XMP.** Chunks are grouped by GUID and joined in offset order
  before the keys are searched, so a GPS key split across two chunks is found
  and the strip is not skipped.

## Fix round 2

Commands as above; `fluorita-core` 160 passed, `celestina-core` 82 passed,
`fluorita-engine` lib against the stub 143 passed with the same 4 libmpv tests
aborting, clippy `-D warnings`, `fmt --check` and the three guards clean.

- **Long names.** The hidden result name keeps at most 180 bytes of the
  original's name, cut on a character boundary, so it and the temporary
  `atomic_file` writes it through stay within the 255-byte `NAME_MAX`.
  `a_name_as_long_as_the_filesystem_allows_is_still_replaced` replaces a
  250-byte name whose cut falls inside a two-byte character.
- **A bounded extended-XMP join.** Each packet is joined only up to the full
  length its chunks declare, never past 64 × 64 KiB, and from at most 64
  chunks; a chunk past those bounds is not read for facts and is still
  stripped. `an_extended_xmp_packet_is_joined_only_as_far_as_it_is_bounded`
  covers a short declaration and a `u32::MAX` one over 100 chunks.
- **One message for a kept result.** `EngineError::user_message` words
  `ReplacementNotPublished` through the engine's product-copy module
  `fluorita-engine/src/copy.rs`: the original is in the Papelera and the
  result was kept beside it under the hidden name it gives. The editor no
  longer words it itself, so the editor and the metadata panel say the same.

## Landing

- **Base revision:** `70d2d71456aeac70bc5d193fce251e3f35717561`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5d85dfdabfd735203fbbff02cc24a9c719c17a690025bae9abd99c6ed3ebda4d, verification_fingerprint sha256:33c8ff2eda0afca2a6864b65af6d918717b00555a2e3be4c9491d8c91c61e00c; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ea89a86f2c716e147c1bd0e21b30c19419c1ac3a751895c9d4d247ef96b5020f, verification_fingerprint sha256:5186dd5531f76f4416649288a783cc08e04043f19a7103cf6a0b5b086c9f9bb1; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f2682377b5e41bdda3aca83a51847bcd77804771fdf32859c78d6a8b6af1a84e, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:506b0b0190a7aee9e92eb79e4ddfbf0d6c2bac0c7fce3f2b96e01bd2271125d2, verification_fingerprint sha256:7384804718f24132de5b942dd5bc55a361b6d6c582a0b4a5872e6be7dcaea211
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh
