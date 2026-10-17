# Evidence: CEL-2-E — the wallpaper duty

- **Date:** 2026-10-16
- **Scope:** unit CEL-2-E of plan `cel-2-parity` on `unit/celestina/CEL-2-E`,
  worktree `celestina-CEL-2-E`, based on `68fd8959`. Not committed; the landing
  commits the unit. Files:
  `celestina-rs/crates/celestina-shell/src/{wallpaper,settings,lib}.rs`,
  `celestina-rs/crates/celestina-shell/tests/salvage.rs`,
  `celestina/crates/celestina-wayland/src/{runtime,surface}.rs` (Background
  layer), `celestina/crates/celestina-paint/src/{painter,backend}.rs` (large
  opaque texture, 1:1 draw), `celestina/src/wallpaper/{mod,decode,surface}.rs`,
  `celestina/src/providers/{wallpaper,settings,mod}.rs`, `celestina/src/{app,
  message,command,bus,main,lib}.rs`, `celestina/src/island/mod.rs`,
  `celestina/tests/wallpaper_decode.rs`, `celestina/scripts/{smoke.sh,
  wallpaper-layers-probe.sh,wallpaper-backdrop-probe.sh}`,
  `celestina/Cargo.toml` and `Cargo.lock` (one new dependency, `image`) and the
  documents listed under Landing.
  `celestina/src/copy.rs` was not modified: the command-line usage text stays
  English like the rest of the CLI.
- **Environment:** Linux 7.2.10-1-cachyos; the toolchain
  `celestina/rust-toolchain.toml` pins; cargo commands run from `celestina/`.
- **Artifact:** none sealed by this record; the landing builds and seals its own.
- **Plan:** the unit's decisions and deviations are in this record; the ledger
  row is in [the CEL-2 ledger](../plans/active/2026-10-16-cel-2-parity.md).
  The working implementation plan stayed with the session that executed it,
  outside the repository.

## What was built

- **The duty.** `shell.json` gains `duties.wallpaper`, `off` by default. When
  it is `on`, the shell paints one still image per output on a
  `celestina-wallpaper` surface in the Background layer. With the duty `off`
  every wallpaper surface is destroyed and every texture is released.
- **Settings.** The `wallpaper` table holds `folder` (absolute path, no
  default), `mode` (`fill`, `fit`, `stretch` or `center`; default `fill`) and
  `outputs` (connector name to file name inside the folder). Out-of-bounds
  values clamp to the default. A value of the wrong type or an unknown name (a
  `mode` of `tile`, a `duties.wallpaper` of `"yes"`, a numeric `folder`, a
  non-string `outputs` entry) falls back to its own default and leaves the rest
  of the file in force, so one typo cannot make the shell treat the file as
  unreadable and lose do-not-disturb. A new `outputs` entry past the cap of 16
  is refused; entries the person already has are never evicted. The schema stays at 1;
  the fields are additive and a file without them reads as before. The shell
  follows the file live.
- **Selection.** For an output: its entry in `outputs`, else `<connector>.<ext>`
  in the folder, else `default.<ext>`, else the fallback state. The fallback
  paints nothing and shows no stand-in; it never shows another output's
  picture. Formats are `png`, `jpg`/`jpeg` and `webp`. AVIF is dropped on
  purpose: its decoder is a C library (dav1d) in a process that reads hostile
  files, and this shell does not take that dependency. The candidates are
  probed directly in the folder (the explicit entry, `<connector>.<ext>`,
  `default.<ext>`), so the choice does not depend on the size or order of the
  folder; the listing used for `next` and `prev` reads at most 8192 raw entries
  and offers the 512 smallest showable names, and logs when it truncates. The
  compositor `Layers` request has 2 s read and write timeouts; a timeout is
  logged and treated as free.
- **Restore at start.** The choices in `shell.json` are applied when the shell
  starts with the duty on.
- **Owner detection.** The provider asks niri (`Request::Layers`, one blocking
  request on its own thread) which surfaces hold the Background layer of an
  output. A surface in that layer that is not ours holds the output: the
  output gets no surface and the shell logs the holder once. The check runs
  when the duty turns `on`, when an output appears, and on `wallpaper retry`.
  It does not poll.
- **Verbs** on `org.celestina.Shell1`: `wallpaper set <output|all> <file>`,
  `wallpaper next [output]`, `wallpaper prev [output]`, `wallpaper mode <mode>`
  and `wallpaper retry`. `next` and `prev` walk the folder's sorted showable
  files. A choice is written to `shell.json` before it is applied.
- **Decode.** A worker owns file IO and decoding (`image` 0.25.6, default
  features off, with `png`, `jpeg` and `webp`). Limits: 8192 by 8192 px per
  source, a 40 MP source area, a 320 MiB decode budget, and a 128 MiB file cap.
  Lossless WebP (VP8L) and WebP with lossless alpha are refused, because the
  `image-webp` lossless decoder can allocate over 1 GiB from a small file. The
  output buffer is at most 7680 by 4320. The file is checked
  before its pixels are decoded; the open is `O_NONBLOCK` and the path must be
  a regular file, so a FIFO, a device or a directory is refused. The source
  bytes are not retained. EXIF orientation is applied. The worker scales the
  image to the output's device size for the chosen mode (Lanczos3 for a
  downscale), so the texture matches the buffer and the draw is 1:1, with no
  resample per frame.
- **Textures.** One texture per output, uploaded once. The CPU copy is dropped
  after the upload.
- **Fractional scale.** The surface is `Anchoring::Fill` and its device size is
  `buffer_size(logical, scale)`. When the scale arrives after the first
  configure, the loop asks the worker for the new device size, and the old
  texture keeps painting until the new one is current.
- **Surfaces for nothing.** The fallback state creates no layer surface, so
  the compositor's own background shows. The surface has an empty input region
  and `KeyboardInteractivity::None`.
- **Rest.** One frame when a texture is uploaded, then none. There is no
  redraw timer. The nested smoke allows 2 frames at rest in the run, summed over
  its surfaces: the picture arriving (decoded off the loop, so after the first
  second) and its redecode when the surface scale changes after the first
  configure. Each is a frame somebody needed though nothing animates in it; any
  other frame at rest fails the smoke.
- **Tests.** Unit tests beside each module, and 15 hostile-file tests in
  `celestina/tests/wallpaper_decode.rs`. Those tests generate their fixtures in
  a temporary directory.

## Procedure

From `celestina/`, in the unit worktree:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
sh scripts/smoke.sh --binary <target>/debug/celestina
```

From the worktree root:

```sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
```

## Result

Run on the unit worktree after the review fixes, on 2026-10-16. The release
profile is the one the production verification uses; debug runs cover the
same tests and were green as well.

| Command | Working directory | Exit | Observed result |
|---|---|---|---|
| `cargo fmt --all --check` | `celestina/` | 0 | clean |
| `cargo clippy --workspace --release --all-targets --locked -- -D warnings` | `celestina/` | 0 | finished, no warnings |
| `cargo test --workspace --release --locked` | `celestina/` | 0 | all green, including 26 decode tests (1 ignored: the largest JPEG, about 13 s in debug) |
| `cargo test --offline --workspace` (debug) | `celestina/` | 0 | 314 lib tests, 26 decode tests, toolkit crates 51, 81, 63 and the rest green |
| `cargo fmt/clippy/test -p celestina-shell` | `celestina-rs/` | 0 | 97 unit tests and 2 salvage tests, clippy clean |
| `bash scripts/check-architecture-contract.sh` | worktree root | 0 | `Architecture contract: OK` |
| `scripts/smoke.sh --binary <target>/release/celestina` | `celestina/` | 0 | `smoke: OK`: `celestina-wallpaper` is in `Background` on `winit`; with the wallpaper present `frames=14 rest_frames=0 over 2 surfaces`; rss 77.3 MiB; the held-output variant is skipped (no layer-shell client in the repository; unit-tested) |
| `scripts/verify-production.sh` | `celestina/` | 1 | refused by design in a session worktree: "production runs happen at landing". Its internal steps were run by hand (rows above); the runner's seal is the landing's |

The language and documentation checks were run by the agent that wrote this
record; their output is recorded in the section below.

### Language and documentation checks run for this record

- `python3 scripts/check-language-contract.py`: `Language contract: OK (131 legacy file(s) ratcheted)`.
- `bash scripts/check-documentation-contract.sh`: re-run after the rebase onto
  the CEL-2 opening commit; see the result in the table above and the landing.

## Backdrop measurement (spec section 7)

`celestina/scripts/wallpaper-backdrop-probe.sh` was run once on 2026-10-16
against a debug build of the unit, in the nested niri 26.04 (`dev-session.kdl`
copy with a private bus and `XDG_CONFIG_HOME`; the live session was not
touched). The nest took a screenshot of its output in the normal view and in
the overview and counted the share of sampled pixels carrying the fixture
picture's marker colour. `niri msg layers` lists the surface in `Background`
on the nested output in every variant and carries no placement field, so the
pixels are the only evidence.

| Profile | Normal view | Overview |
|---|---|---|
| with `place-within-backdrop` rule | 0.0% | 47.1% |
| without the rule | 97.7% | 34.8% |
| rule, plus `layout { background-color "transparent" }` | 97.7% | 81.9% |
| no rule, plus transparent background | 97.7% | 34.8% |

Answer: in the nest, a `celestina-wallpaper` surface matched by the rule is
placed within the backdrop; the wallpaper then shows only in the overview,
unless the workspace background is transparent, as in the author's real
`~/.config/niri/config.kdl` (`layout { background-color "transparent" }`), where
it shows in the normal view too. This is one run of one nest at one output
size; it does not establish what the author's session does, which is part of
VAL-CEL2-WALLPAPER.

## Limits

Nothing was checked in a real session. Not exercised: a Wayland session with
the wallpaper on, the perceptual quality of the scaling at 1.5, the owner
check against Noctalia's own wallpaper surface, hotplug, and a restart with a
saved choice. These are the author's row VAL-CEL2-WALLPAPER in
[VALIDATION.md](../../VALIDATION.md). The rest of the record is unit tests and
the nested smoke.

## Open risks

- **Peak decode memory.** The worst case for any accepted file is about
  322 MiB: `max(320 MiB, D + O) + 2 MiB`, where the decode budget is 320 MiB
  (predicted per format from the header and file size before any pixel is
  decoded), `D` is the decoded source and `O` the output buffer (at most
  7680 by 4320, 126.6 MiB). See the module docs of `src/wallpaper/decode.rs`.
- **Failed GPU upload.** A failed upload leaves a transparent surface until the
  next event.
- **Initial double decode on fractional scale.** Outputs with a fractional
  scale are decoded twice at start.
- **Overlap with CEL-2-A and CEL-2-B.** The `shell.json` follower added here
  (`providers/settings.rs`) may overlap the follower CEL-2-A or CEL-2-B puts on
  `main` first. When that happens the first landing owns the follower and the
  later unit consumes it.
- **Memory cost.** One texture per output, at its device size.
- **Backdrop placement.** The author's niri `wallpaper` rule is an unanchored
  regex that also matches `celestina-wallpaper`, so the surface may be placed
  within the backdrop. See the measurement below: that hides the wallpaper
  outside the overview unless the workspace background is transparent.

## Rulings and known limitations

- The Background layer is mapped from `Layer::Background` in
  `celestina-wayland`; `Top` and `Overlay` are unchanged.
- The duty ships `off`. The shell refuses an output another client holds in
  the Background layer; it does not stack over Noctalia's wallpaper.
- The `image` crate is the only new dependency; its features are the three
  formats above and nothing else.

## Follow-up

The author's row VAL-CEL2-WALLPAPER in [VALIDATION.md](../../VALIDATION.md).
The roadmap's CEL-2 registration and the ledger's structural items belong to
the program's first landing.

## Landing

TODO-FILL: the landing writes the base revision, the inventory link, the
diffstat and the production check for this unit.

## Landing

- **Base revision:** `5ccf48d7a080aeb90c881c3429a2ded533ee6f16`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:6df472c73c9e851d33763439c6fc4e613a4321fdb91118bf347444f157d55d22, verification_fingerprint sha256:abe9f9cb9f40342bb3259f932d000df1ff03a87f6db019b80108288ef5b1e0da; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a6e7cedbf84f6ba69cf157c799893e0dc45021edfb961aff248ca7d6182fbe61, verification_fingerprint sha256:7d197df1cb6a821fbf3bdd02a307b1ef5f957937f78ca2078316004ae1ea0534
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
