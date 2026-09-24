# The storage section's locations and browsing — S1-B

- **Date:** 2026-09-23
- **Scope:** `S1-B` of
  [`../plans/archive/2026-09-23-s1-storage.md`](../plans/archive/2026-09-23-s1-storage.md):
  the mount locations, folder browsing, the `HematitaAnalysis` hub with
  its whole contract declared, the Almacenamiento section and its smoke line
- **Environment:** the author's checkout; offscreen Qt only, no window on
  the live or nested session
- **Artifact:** `hematita/target/release/hematita` built once by
  `scripts/build-production.sh`, verified by `scripts/verify-production.sh`;
  not deployed (S1-Z deploys 1.1.0)

## What changed

1. **`locations.rs`.** Parses `/proc/self/mounts` through
   `usage::mounts`, keeps `is_shown_location`, measures each mount with
   `rustix::fs::statvfs` (`total = f_blocks × f_frsize`,
   `used = (f_blocks − f_bfree) × f_frsize`) and marks it readable when
   `read_dir` succeeds; `/` is dropped when unreadable. Kinds are the
   tokens `system`, `home`, `disk`; QML names the first two. A disk is
   named by the model of the block device behind its source
   (`disk_of_source`: `nvme0n1p1` → `nvme0n1`, `sda1` → `sda`), read
   through the new `sampler::disk_model_of`, which reuses the disk
   section's `disk_info`; without a model, the mount point's last segment.
   Order: system, home, then disks by path; a mount point listed twice is
   kept once.
2. **`browse.rs`.** `list_folder` reads one directory with
   `symlink_metadata` (a link is `other`, never followed), folders first,
   then a case-insensitive name comparison local to Hematita with the exact
   bytes as the tie-break.
3. **`analysis.rs` (`HematitaAnalysis`).** Every property of the plan's
   analysis contract is declared; the scan, duplicate and selection lists
   are published empty. `mode` is `locations` or `browsing` in this unit.
   Locations are read on `hematita-locations`, listings on
   `hematita-browse`; each request takes the next generation and older
   results are dropped. The browsed path is a stack of byte-exact
   `PathBuf`s; `crumbNames`/`crumbPaths` are lossy, for display. QML acts
   by index: `open()`, `enter(index)`, `up()`, `refresh()` and
   `enterCrumb(index)` (an addition to the contract: a crumb button goes
   straight to its folder; below zero returns to the locations). `busy` is
   set before the publication that starts a read, so the in-between empty
   list is never read as the answer. `browseFailed` (also an addition)
   lets the list say that a folder could not be read.
4. **Declared but inert.** `scanHere`, `cancel`, `confirmDuplicates`,
   `toggleSelected`, `clearSelection`, `selectAllButOne`, `openSelected`,
   `trashSelected` and `deleteSelected` exist and answer
   `actionOutcome = "refused"` (with `actionKind` `open`, `trash` or
   `delete` for the three actions) until S1-C and S1-D fill them. The page
   does not call them: their buttons are disabled.
5. **QML.** `StoragePage` (bar with `PathCrumbs`, the filters and actions
   capsules disabled, a refresh button; body by mode), `PathCrumbs` (a
   button back to the locations, then one ghost button per crumb; Qt has
   no navigation-landmark role, so the row is an `Accessible.ToolBar`),
   `LocationList` (kind glyph, name, path in `textMuted`, an occupation
   bar in `glyphAccentAmber`, `%1 de %2`) and `FolderList` (kind glyph,
   name, size or a dash for a folder; Backspace goes up). Both lists: an
   integer model, one Tab stop, arrows, Enter and double click enter, the
   viewport kept across a rebuild of the same list; a new folder starts at
   its top. Entering or leaving hands the keyboard to the list on show.
6. **Wiring.** Sixth section `storage` (`hard-drive`, `Almacenamiento`);
   `analysisHub.open()` whenever the section is entered; the smoke prints
   `hematita-storage <locations>` once and `smoke.sh` asserts it above
   zero.
7. **Dependencies.** `rustix` gains the `fs` feature. The `siderita-ops`
   path dependency is deferred to S1-D, its first caller: added now it
   would be compiled and linked with nothing using it.

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
cd hematita && cargo test --release --locked --all-targets \
  && cargo clippy --release --all-targets --locked -- -D warnings \
  && cargo fmt --all --check
hematita/scripts/verify-production.sh
# the offscreen walk, 14 s, with a throwaway runtime dir and no session bus
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 14 hematita/target/release/hematita
```

## Result

- **Exit:** 0 for every command; the walk ended by `timeout` (124).
- **Tests:** `hematita` `34 passed` (11 new: `locations` 4, `browse` 3,
  `analysis` 4); `hematita-core` `87`, `11`, `18` passed.
- **Clippy:** first reported one `useless_vec` in a test, replaced by an
  array; then clean.
- **qmllint:** `org.celestina.hematita (0 non-fatal baseline warning(s))`,
  the row stays `0`.
- **Smoke:** `smoke: OK` with the Storage page listing locations.
- **Walk:** `hematita-shape cpu 3 60`, `hematita-sensors 9 44`,
  `hematita-services 143 true false`, `hematita-storage 7` (`/`, `/home`
  and five `/mnt` disks, matching `/proc/self/mounts`); no QML error.
- **Found by the smoke:** the first verify printed `hematita-storage 0`:
  `open()` published the empty in-between list before setting `busy`.
  `busy` now precedes the publication in `open()` and in the browse path.

## Limits

- No scan runs: `scanning` and `analysed` are S1-C. The inert invokables
  above answer `refused`.
- Browsing, the crumbs, the keyboard path and the occupation against
  `df -h` are for `VAL-S1` on the real session; the smoke proves the page
  constructs and the locations are read.

## Follow-up

`S1-C`. `VAL-S1` stays pending.
