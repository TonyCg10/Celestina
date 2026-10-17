# Evidence: CEL-2-G — the clipboard history

- **Date:** 2026-10-17
- **Scope:** unit CEL-2-G on its session worktree `celestina-CEL-2-G`, not
  landed. Areas: `celestina-rs/crates/celestina-shell/src/{clipboard,settings,lib}.rs`,
  `celestina/src/clipboard/` (`mod`, `store`, `worker`, `thumb`, `verbs`),
  `celestina/src/pieces/clipboard.rs`, `celestina/src/island/pages/clipboard.rs`,
  additive hunks in `celestina/src/{app,command,message,main,copy,lib}.rs`,
  `celestina/scripts/smoke.sh`, and the documents of this unit.
- **Design:** [privacy design](2026-10-17-clipboard-privacy-design.md);
  [plan](2026-10-17-clipboard-plan.md).
- **Environment:** Linux 7.2.10-1-cachyos; the toolchain
  `celestina/rust-toolchain.toml` pins. The shared Cargo target of the session
  worktrees is used, so the smoke ran on a copy of the binary taken right after
  `cargo build --locked` (another worktree can relink `debug/celestina`).
- **Artifact:** none sealed here; a session never builds production
  (landing contract). The smoke ran on the debug binary through
  `scripts/smoke.sh --binary`.

## What was built

The `duties.clipboard` duty, off by default: a worker on `ext-data-control`
that records the regular selection (text and `image/png`), skips a selection
that offers `x-kde-passwordManagerHint` without reading it, keeps 200 texts and
20 images (images in memory only), persists text only to
`$XDG_STATE_HOME/celestina/clipboard.json` (mode 0600, directory 0700, atomic,
7-day expiry), deletes the file when the duty goes off, and offers the verbs
`clipboard select N [S]|remove N [S]|clear|pause|resume`. The bus answers `ok`
as soon as the worker has queued a verb; how it ended (a stale snapshot, a
missing entry) goes to the journal only, and no verb returns content. A bar piece and an Island page show previews and thumbnails.

## Procedure

From `celestina/`:

```sh
cargo fmt --all --check                                          # clean
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked                                  # 835 passed, 0 failed, 1 ignored
cargo build --locked
sh scripts/smoke.sh --binary <copy of target/debug/celestina>    # exit 0, "smoke: OK"
```

## Result

From `celestina-rs/`: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets --locked -- -D warnings` (clean) and `cargo test --workspace
--locked` (2240 passed, 0 failed, 4 ignored; `celestina-shell` alone 196 plus 2
salvage). From the worktree root: `bash scripts/check-architecture-contract.sh`
(`Architecture contract: OK`) and `python3 scripts/check-language-contract.py`
(`Language contract: OK (131 legacy file(s) ratcheted)`).

`celestina` workspace tests: library 590, `tests/cli.rs` 3,
`tests/notifications_bus.rs` 1, `tests/wallpaper_decode.rs` 26 (1 ignored),
`celestina-paint` 51 plus 2, `celestina-ui` 81 plus 7 plus 9,
`celestina-wayland` 65.

`verify-production.sh` was not run: a session worktree refuses production runs
(`production-artifact: this is a session worktree`); the landing runs it.

## The clipboard smoke (a sixth nested run)

It runs only inside the nest of `dev-session.sh --own-bus --monitor none`, with
a throwaway `XDG_CONFIG_HOME` and `XDG_STATE_HOME`, a system bus that does not
exist, and a driver that refuses to start any client unless `NIRI_SOCKET` is set
and `WAYLAND_DISPLAY` differs from the host's. No fake client was written:
`wl-copy`/`wl-paste` 2.3.0 are used, and the run checks that they speak
ext-data-control to the nest (`WAYLAND_DEBUG` capture contains
`ext_data_control_source_v1`, and the `--sensitive` copy offers
`x-kde-passwordManagerHint`). The author's clipboard is never reachable. The
shell runs with `RUST_LOG=debug`. Observed only through the journal (counts,
sizes, reasons) and the state file; no verb returns content.

Asserted, and passing in the final run:

- a text selection grows the history 0 to 1; a `--sensitive` copy adds nothing
  (`skipped a selection that carries an exclusion hint`, once); a PNG is
  accepted (total 2); sequence of totals `0 1 2 2 2 0`, never 3;
- state file mode `600`, directory `700`, one entry in the directory, the text
  present, the hinted string absent, no image bytes (`iVBOR`, `PNG`,
  `image/png` absent);
- `clipboard select` twice (text, then image) re-selects without duplicating;
  two `skipped the echo of its own selection` lines; `wl-paste` returns the
  text unchanged and the PNG byte-identical;
- `duties.clipboard` to `off` in `shell.json`: `the history file was deleted` (logged only on success; a failure is
  logged separately), file absent, a verb then answers `the duty is off`; back to `on`: `entries=0` and
  no file;
- privacy: the two fixed test strings (`clipsmoke-visible-…`,
  `clipsmoke-secret-…`) have zero hits in the shell's stderr journal (debug
  level for the whole shell), its stdout, the nest's log and the driver's
  notes. Journal lines for clipboard events carry `rows`, `total`, `bytes`,
  `kind`, `mimes` only.
- metrics (after round B, debug binary copied from the shared target, full
  smoke exit 0): `winit frames=17 rest_frames=2 late=0`, `rss_mib=97.0`. The two
  rest frames are the two duty switches (`off`, then `on`), the allowance;
  history changes (6) cost none. A first run failed with 3 rest frames: the
  third was the clock's minute tick at :00, because the run could start up to
  :40 into a minute and lives 30 s; the start window is now :02 to :22.
- the second-start assertion reads the real log: the worker's `duty on` line
  says `load=false` after an off (the first start says `load=true`), the file
  is absent, and the first publication is `rows=0 total=0`.

Other sections in the same smoke (default, launcher, bar, wallpaper, energy)
passed in the final run: rest frames 2, 0, 0, rss 96.9 and 95.2 MiB.

## Findings

- **Redraw with nothing to show (fixed in round B).** Each published history
  change used to redraw every surface even with the bar off and no page open
  (5 frames at rest in the clipboard run). Now a history change redraws every
  Island only when the duty's `paused` flag changed, otherwise only the Islands
  whose Clipboard page is shown; `Island::frame` invalidates the bar only when
  `on` or `paused` changed; toggling `duties.clipboard` always redraws every
  Island (the bar's piece appears or goes even over an empty history). The
  smoke's allowance for that run is lowered from one rest frame per history
  change to one per duty switch the journal reports, and the full smoke is
  green with it (5 rest frames before, 2 now).
- **Flaky rest-frame gates under host load.** With the host at load average
  about 10 (other sessions), two of three full smoke runs failed in earlier
  sections (default run 11 rest frames; wallpaper run 9), not in the clipboard
  section; the third passed. Not caused by this unit; the pre-existing gates
  are sensitive to load.
- **Deletion hazard introduced by this unit.** A shell whose clipboard duty is
  off (the default) deletes `$XDG_STATE_HOME/celestina/clipboard.json` at start.
  Before this unit no such file existed, so every script or nest that starts
  the binary with the author's real `XDG_STATE_HOME` can now delete the
  author's history. The smoke exports a throwaway `XDG_STATE_HOME` for every
  run; round B did the same in `scripts/wallpaper-backdrop-probe.sh` and
  documented it in the README. `scripts/dev-session.sh` deliberately inherits
  its caller's environment (it is the author's own interactive nest) and is
  covered by the README note only.
- The shared Cargo target means `debug/celestina` may belong to another
  worktree; the first clipboard smoke attempt ran a binary without the
  clipboard and timed out waiting for `clipboard duty on`.

## Reviews and rounds

Two reviews of the unit were run after the first green pass.

Round A (privacy and lifecycle, `celestina/src/clipboard/`, `celestina-shell`)
addressed the findings of the privacy review: a snapshot number on every
published change and the verbs `select N S` / `remove N S` that the worker
refuses when the history moved on; deletion on `off` through a reaper that
revokes the worker's keeper first, so a worker still saving can never recreate
the file; sweeping of the plaintext temporaries an interrupted write leaves;
and no deletion on the protected defaults the shell falls back to when its
settings file exists but cannot be read (deferred to the first clean `off`).

Round B (surface, application and documents) addressed the findings of the
second review: the page sends `SelectShown` / `RemoveShown` with the snapshot
of the rows it drew; a held Delete removes one entry (the toolkit does not say
which key event is an auto-repeat, so the Island requires the release in
between); the redraw narrowing above and the lowered smoke allowance; rows that
do not fit are counted in a last line (`clipboard_more` in `copy.rs`) and the page makes room for it;
the confirmation step focuses Cancel when it appears and Clear when it is
given up (`Interaction::focus_key` in `celestina-ui`); `CLIPBOARD_EMPTY` as a short phrase instead of
a sentence for the empty history; the tab strip (labels now at the caption
size, measured with the real Inter font: "Notificaciones" is 92 px at the body
size in a 91 px tab when there were four tabs; after the rebase onto the glance
pieces there are six tabs of about 58 px, labels use the caption size and the
short forms of `tabs.rs` (the clipboard's is `Portap.`), and each tab clips its
label; the real-font fit test of `tabs.rs` runs at both text scales); the probe script's state home; the log that said the file was deleted
when deletion could have failed; a test that fails when a page is added to the
table without being pinned or declared pinned elsewhere; the thumbnail budget
in the design; and the README sentence on verb answers.

The two reviews' own texts are not stored in the repository; this section
records what they led to.

## Residual risks

- A selection is served to a paste synchronously on the worker's thread, so a
  slow reader of the source holds the worker (and its publications) for as long
  as the transfer lasts; serving on a thread of its own is not done.
- On a duty `off` the loop waits for the previous reaper before starting a new
  one, and `shutdown` waits for the reaper: the loop can wait up to about 2 s
  (the worker join bound) on the thread that runs the shell.
- A history file whose deletion fails on a transient error (permissions,
  busy filesystem) is left, logged once as not deleted, and retried only by
  the next clean `off`; with the duty already off nothing retries it.
- A secret copied without `x-kde-passwordManagerHint` is recorded and lives
  up to seven days (text, on disk at mode 0600; images in memory only).
- The larger text scales clip the longest tab label ("Notificaciones") instead
  of overflowing it; the look is queued for the author.
- A thumbnail with transparency is composited over black, so on a light
  surface it looks darker than the image. Compositing over a theme surface
  would need a colour passed to the decoder the wallpaper unit shares; left.

## Limits

Nothing was checked in the real session. Not proven: a real password manager's
copy (the hint is exercised with `wl-copy --sensitive`), the look of the piece
and page, select and paste round trip with real applications, real screenshot
copies. These are `VAL-CEL2-CLIPBOARD-LIVE`, `VAL-CEL2-CLIPBOARD-SECRET` and
`VAL-CEL2-CLIPBOARD-IMAGE` in [VALIDATION.md](../../VALIDATION.md). The
primary selection is never watched. Content heuristics are deliberately absent:
a secret copied without the hint is recorded and lives at most seven days.

## Landing

Filled by the landing (`scripts/land-unit.py`): base revision, build, verify,
deploy. The version is bumped by the landing. The root `AGENTS.md` salvage list
must name this unit's copies (`celestina-qt/src/provider_adapter/clipboard.rs`,
`celestina-shell-core/src/clipboard.rs`) before the first commit.

## Landing

- **Base revision:** `99bbbda980dc36a45b0beae19303d2e9c29b576a`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5867a79adba934cf26b54772feeeabb568684ba99601a6f329d4e6815665d23f, verification_fingerprint sha256:2786a973f410962af8ae477655bd2147a26122a9fdbb4ee27d3e7e75adc2ad1b; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5cc5e8dd488244b3959990e8abaa30ecbb875cd3c00dd2ab271298a28dae1182, verification_fingerprint sha256:f3dd02f7200f196f178f563d50d05b49f20ace092eefe249f560837b65f661b4
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
