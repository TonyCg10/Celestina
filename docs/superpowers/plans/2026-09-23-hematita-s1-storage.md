<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita S1 — The storage analyzer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A sixth section, `Almacenamiento`, that lists the mount points, lets a person browse into any of them and scan where they are, shows the result as a size list beside a treemap, finds duplicates (verified by content on demand) and empty folders, and acts on a selection: open in Siderita, trash, delete permanently, details.

**Architecture:** `hematita-core::usage` is the pure domain: an indexed tree built by an iterative, device-bounded, cancellable walk with progress; empty-folder and duplicate finders (size candidates, then hash buckets, then byte comparison); the squarified treemap layout; a guarded permanent deletion; a `/proc/self/mounts` parser. `hematita/src` adds a locations reader (`statvfs` through `rustix`), a browse worker, a scan worker (one thread per scan, `Generation`-stamped, progress every 250 ms), the `HematitaAnalysis` hub publishing index-aligned lists with `revision` last, and an actions worker that reuses `siderita_ops::trash`. QML adds the section with three modes and the treemap as rectangle buttons. No new dependency beyond enabling `rustix`'s `fs` feature and adding `siderita-ops` as a path dependency.

**Tech Stack:** Rust 1.97.1, cxx-qt 0.9.1, `rustix 1.1.4` (`fs` feature added), `siderita-ops` (workspace crate), Qt 6.9+ QML, `celestina-style` symlinks.

**Spec:** [docs/superpowers/specs/2026-09-23-hematita-storage-design.md](../specs/2026-09-23-hematita-storage-design.md). Carry-overs: the H1 plan's inventory generator and commit procedure; the H2–H5 list-publishing shape (`revision` last, integer `ListView` models, `anchoring`, viewport restore).

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-shell-core`.
- **Language contract:** English in code and documents; Spanish only as `qsTr()` literals in QML; no Rust file carries a user-visible string. Names and paths from the filesystem are data shown raw (lossy display through `to_string_lossy`; the byte-exact path never returns from QML — see ADR 0008: QML acts on entry ids, never on text).
- **Build budget (author's rule):** four application build cycles — one each at the end of S1-B, S1-C and S1-D (release build + release-profile tests/clippy + `verify-production.sh`), one inside `complete-production.sh` at S1-Z. S1-A is crate-only. Never `cargo` under `hematita/` outside those moments; never `cargo clean`; never a window on the live or nested session (`VAL-S1`); no scan, trash, deletion or Siderita launch runs during verification.
- **Commits:** per unit after review; `hematita:` prefix; inventories through the H1 plan's generator (scratchpad `mkinv.py`); evidence under `hematita/docs/evidence/`, inventories under `hematita/docs/inventories/2026-09-23-s1-storage/`; hooks never bypassed; explicit pathspecs.
- **Safety invariants:** the walk never follows symlinks and never leaves the root's device; `delete_tree` refuses the scanned root, any path outside it, any mount root, and follows no symlink; trash goes only through `siderita_ops::trash`; every destructive action is confirmed in QML through `ConfirmDialog` and reported with a typed outcome; the hub prunes its tree only after the worker reports success per path.
- **Rust invariants:** no `unsafe`; no production `unwrap`/`expect`/`panic!`; typed errors carrying the path; blocking IO never on the Qt thread (locations, browse, scan, confirm, actions all on named threads); stale generations dropped; workers cancellable and joined or detached deliberately with the reason written down.
- **QML invariants:** every new file in `build.rs` `QML_FILES`; tokens only; `required property`; no tooltips; every list one Tab stop with arrows; no `Canvas`; no lint suppressions; the `hematita` qmllint row is `0` and may not rise; viewport preserved across rebuilds.
- **Constants once:** `PROGRESS_EVERY = 512` entries and `HEAD_BYTES = 64 KiB` in `usage/duplicates.rs`; `MIN_TILE_SHARE = 0.005` in `usage/layout.rs`; `PROGRESS_INTERVAL = 250 ms` in `usage_worker.rs`; `SHOWN_FILESYSTEMS` in `usage/mounts.rs`.
- **Analysis contracts (index-aligned lists on `HematitaAnalysis`):**
  - `mode`: `locations | browsing | scanning | analysed`.
  - locations: `locationNames, locationPaths, locationKinds (system|home|disk), locationUsed, locationTotal (doubles, bytes), locationReadable (0/1)`.
  - crumbs: `crumbNames, crumbPaths` (root first).
  - browse: `browseNames, browseKinds (dir|file|other), browseApparent (doubles)`.
  - progress: `progressFiles, progressBytes (doubles), progressPath`.
  - analysed (the current node's children): `entryIds (ints), entryNames, entryKinds (dir|file|other), entryAllocated, entryApparent (doubles), entryShares (0..1), entryFilesBelow (doubles), entryEmpty, entryDuplicate, entryUnreadable (0/1)`; `treemapRects` (flat doubles `[id, x, y, w, h]` × n, normalised 0..1, the merged remainder with id −1); `currentAllocated`, `currentUnreadable`.
  - duplicates: `groupSizes (doubles), groupCounts, groupVerified (0/1)`, `memberGroups (ints), memberIds (ints), memberNames, memberPaths`.
  - selection: `selectedIds (ints)`; filters `showDuplicates, showEmpty`; `revision`; `busy` (a worker other than the scan is running); `startFailed`.
  - outcomes: `actionOutcome ("" | done | partial | failed | refused)`, `actionKind (open|trash|delete)`, `actionDone`, `actionTotal` (ints).

---

## File structure

| Path | Responsibility |
|---|---|
| `celestina-rs/crates/hematita-core/src/usage/mod.rs` | module root: re-exports |
| `…/usage/tree.rs` | `NodeId`, `Node`, `Kind`, `Tree`, path reconstruction, pruning + re-aggregation |
| `…/usage/walk.rs` | iterative device-bounded scan with progress and cancellation |
| `…/usage/empty.rs` | empty folders |
| `…/usage/duplicates.rs` | candidates by size; confirmation by hash buckets and byte comparison |
| `…/usage/layout.rs` | squarified treemap |
| `…/usage/remove.rs` | guarded permanent deletion |
| `…/usage/mounts.rs` | `/proc/self/mounts` parser and the shown-filesystem rule |
| `…/tests/usage_tree.rs` | the fixed-tree integration tests (real temp directory) |
| `hematita/src/locations.rs` | mounts → locations with `statvfs`, disk model names |
| `hematita/src/browse.rs` | one-folder listing on a thread |
| `hematita/src/usage_worker.rs` | scan and confirm workers |
| `hematita/src/actions.rs` | open in Siderita, trash through `siderita-ops`, permanent deletion |
| `hematita/src/analysis.rs` | `HematitaAnalysis` hub |
| `hematita/qml/components/{StoragePage,PathCrumbs,LocationList,FolderList,UsageList,Treemap,DuplicateList,DetailsCard}.qml` | the section |
| `hematita/qml/Main.qml`, `build.rs`, `Cargo.toml`, `scripts/smoke.sh` | wiring |

Ledger units:

| Unit | Kind | Content | Build |
|---|---|---|---|
| S1-A | `hematita-maintenance` | `usage` module complete with tests; S1 opened in the documents | none |
| S1-B | `hematita-maintenance` | locations, browse, hub (locations/browsing), `StoragePage`, `PathCrumbs`, `LocationList`, `FolderList`, section, smoke line | one |
| S1-C | `hematita-maintenance` | scan worker, analysed mode, `UsageList`, `Treemap`, progress/cancel, filters, `DuplicateList` with confirmation, `DetailsCard` | one |
| S1-D | `hematita-maintenance` | actions worker, batch trash, permanent deletion, pruning, selection, all-but-one | one |
| S1-Z | `hematita-milestone` | 1.1.0, `complete-production.sh`, documents, plan archived | one |

---

### Task 1: Open S1 in the documents

Same shape as H5's Task 1 (copy the ledger header and table from `hematita/docs/plans/archive/2026-09-22-h5-services.md`): plan `hematita/docs/plans/active/2026-09-23-s1-storage.md` (Plan ID `s1-storage`, checkpoint `S1`, `VAL-S1`, hypothesis "a device-bounded walk with progress can analyse the author's 430 GiB home while the window stays live, and the list-plus-treemap with verified duplicates and empty folders lets the author act on what is found without leaving Hematita", five units, exclusions from spec §1); roadmap `active`/`S1`, rows S1-A (dep REL-1) … S1-Z, `## S1 — opened 2026-09-23` replacing the "further work opens…" sentence; STATUS; plan README; VALIDATION:

```markdown
## VAL-S1 — The storage analyzer on the real session

- **Status:** pending
- **Related implementation:** S1
- **Requires:** the deployed Hematita 1.1.0; a USB disk with something disposable on it
- **Procedure:** open the storage section; read the locations and their occupation against `df -h`; enter the home location, browse two levels, scan there and watch the progress; cancel a scan of the whole home halfway, then scan it fully; compare the biggest folder's size with `du -sh`; enter a `/mnt` disk and scan it; enable the duplicates filter, verify one group, select all but one and trash them, then find them in Siderita's trash; enable the empty-folders filter and trash three of them; on the USB disk delete one file permanently through the dialog; walk lists and treemap by keyboard; watch Hematita's CPU a minute after a scan
- **Pass condition:** occupation matches `df` within rounding; the scan advances visibly and cancels within a second; the folder size matches `du` within 1 %; verified duplicates are byte-identical (`cmp` on one pair); trashed entries appear in the trash and the tree updates without a rescan; the permanent deletion asks, names the count and size, and removes only what was selected; every list and the treemap reachable by keyboard; idle CPU under 2 % after a scan
- **Result:** not run
- **Evidence:** none
```

No commit of its own: the documents land inside S1-A's inventory.

---

### Task 2: `usage::tree` and `usage::walk`

**Files:** create `celestina-rs/crates/hematita-core/src/usage/{mod.rs,tree.rs,walk.rs}`; modify `src/lib.rs` (`pub mod usage;`).

**Interfaces:**

```rust
// tree.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Dir, File, Other }
#[derive(Clone, Debug)]
pub struct Node {
    pub name: std::ffi::OsString,
    pub kind: Kind,
    pub parent: Option<NodeId>,
    pub allocated: u64,
    pub apparent: u64,
    pub files_below: u64,
    pub unreadable: bool,
    pub children: Vec<NodeId>,
}
#[derive(Clone, Debug)]
pub struct Tree { pub root: NodeId, pub path: std::path::PathBuf, pub device: u64, pub nodes: Vec<Node>, pub unreadable_dirs: u32 }
impl Tree {
    pub fn node(&self, id: NodeId) -> Option<&Node>;
    pub fn path_of(&self, id: NodeId) -> std::path::PathBuf;      // root path + ancestors' names
    pub fn children_by_size(&self, id: NodeId) -> Vec<NodeId>;    // descending allocated, ties by name
    pub fn prune(&mut self, id: NodeId) -> Option<u64>;           // removes the subtree, re-aggregates ancestors, returns the allocated bytes removed; None for the root
}
```

```rust
// walk.rs
pub const PROGRESS_EVERY: u64 = 512;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress { pub files: u64, pub bytes: u64, pub current: std::path::PathBuf }
#[derive(Debug)]
pub enum ScanError { Cancelled, Root { path: std::path::PathBuf, source: std::io::Error }, NotADirectory { path: std::path::PathBuf } }
pub fn scan(root: &std::path::Path, cancel: &celestina_core::CancellationToken, progress: &mut dyn FnMut(Progress)) -> Result<Tree, ScanError>;
```

- [ ] **Step 1: Write `tree.rs` with its unit tests first** — a hand-built three-level tree: `path_of` joins the root path with the ancestor names; `children_by_size` orders `[c(30), a(20), b(20)]` as `c, a, b`; `prune(child)` subtracts the child's `allocated`/`apparent`/`files_below` from every ancestor and returns the bytes; `prune(root)` is `None`.

- [ ] **Step 2: Implement `tree.rs`**. `prune` walks `parent` links upward subtracting the pruned aggregates and removes the id from its parent's `children`; the node stays in the `Vec` (ids are stable), marked by clearing its children and setting `allocated`/`apparent`/`files_below` to 0.

- [ ] **Step 3: Write `walk.rs`**

```rust
//! The walk: what is under a folder, on this device, as an indexed tree.
//!
//! Iterative with an explicit stack, so a path a thousand levels deep cannot
//! overflow ours. `symlink_metadata` throughout: a symbolic link is an entry
//! with its own small size and is never followed, so a loop or a link to a
//! bigger disk cannot inflate the result. A directory on another device is
//! listed as a leaf with no size: mounts are analysed from their own root.
//! A hard link counts once per scan. An unreadable directory is marked and
//! the walk goes on: the person gets a partial truth with the count of what
//! was refused, never a silent hole.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use super::tree::{Kind, Node, NodeId, Tree};

pub const PROGRESS_EVERY: u64 = 512;
const BLOCK_BYTES: u64 = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub files: u64,
    pub bytes: u64,
    pub current: PathBuf,
}

#[derive(Debug)]
pub enum ScanError {
    Cancelled,
    Root { path: PathBuf, source: io::Error },
    NotADirectory { path: PathBuf },
}

impl std::fmt::Display for ScanError { /* three arms naming the path */ }
impl std::error::Error for ScanError {}

pub fn scan(
    root: &Path,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Tree, ScanError> {
    let meta = fs::symlink_metadata(root).map_err(|source| ScanError::Root { path: root.to_path_buf(), source })?;
    if !meta.is_dir() {
        return Err(ScanError::NotADirectory { path: root.to_path_buf() });
    }
    let device = meta.dev();
    let mut nodes = vec![Node {
        name: root.file_name().map(OsString::from).unwrap_or_else(|| OsString::from("/")),
        kind: Kind::Dir, parent: None, allocated: 0, apparent: 0, files_below: 0, unreadable: false, children: Vec::new(),
    }];
    let root_id = NodeId(0);
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
    let mut stack: Vec<(NodeId, PathBuf)> = vec![(root_id, root.to_path_buf())];
    let mut unreadable_dirs = 0u32;
    let mut entries_seen = 0u64;
    let mut files = 0u64;
    let mut bytes = 0u64;

    while let Some((dir_id, dir_path)) = stack.pop() {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let read = match fs::read_dir(&dir_path) {
            Ok(read) => read,
            Err(_) => {
                nodes[dir_id.0 as usize].unreadable = true;
                unreadable_dirs += 1;
                continue;
            }
        };
        for entry in read {
            let Ok(entry) = entry else { continue };
            let Ok(meta) = entry.metadata_symlink() else { continue }; // see note below
            entries_seen += 1;
            let kind = if meta.is_dir() { Kind::Dir } else if meta.is_file() { Kind::File } else { Kind::Other };
            let counted = kind == Kind::File && seen_inodes.insert((meta.dev(), meta.ino()));
            let allocated = if counted { meta.blocks().saturating_mul(BLOCK_BYTES) } else if kind == Kind::Other { meta.blocks().saturating_mul(BLOCK_BYTES) } else { 0 };
            let apparent = if counted || kind == Kind::Other { meta.len() } else { 0 };
            let id = NodeId(u32::try_from(nodes.len()).unwrap_or(u32::MAX));
            nodes.push(Node { name: entry.file_name(), kind, parent: Some(dir_id), allocated, apparent, files_below: u64::from(counted), unreadable: false, children: Vec::new() });
            nodes[dir_id.0 as usize].children.push(id);
            if counted { files += 1; bytes += allocated; }
            if kind == Kind::Dir && meta.dev() == device {
                stack.push((id, entry.path()));
            }
            if entries_seen % PROGRESS_EVERY == 0 {
                progress(Progress { files, bytes, current: dir_path.clone() });
            }
        }
    }
    aggregate(&mut nodes, root_id);
    Ok(Tree { root: root_id, path: root.to_path_buf(), device, nodes, unreadable_dirs })
}
```

`entry.metadata_symlink()` is not a std method: use `fs::symlink_metadata(entry.path())` (one `lstat` per entry; `DirEntry::metadata` follows nothing on Linux either, but be explicit). `aggregate` is a post-order pass: process nodes in reverse insertion order (children are always inserted after their parent, so reverse order visits every child before its parent) adding each node's `allocated`/`apparent`/`files_below` into its parent.

- [ ] **Step 4: Tests in `tests/usage_tree.rs`** — a helper `fixture() -> (PathBuf, Cleanup)` builds under `std::env::temp_dir().join(format!("hematita-usage-{}-{}", std::process::id(), nanos))`:

```text
root/
  big.bin         (4 MiB of 0xAB)
  copy-of-big.bin (identical bytes)
  same-size.bin   (4 MiB of 0xCD)
  docs/
    a.txt (3 KiB)  b.txt (3 KiB, identical to a.txt)
    link-to-a.txt  (hard link to a.txt)
  empty/
    deeper/        (empty)
  out.lnk -> /tmp   (symlink)
  locked/          (chmod 000 after creating locked/secret.txt; restored by Cleanup)
```

Assertions: `files_below` of root = 6 (big, copy, same-size, a, b, secret is unreadable → 5 counted files + the hard link counted once → total 5); `allocated(root) == sum of counted blocks`; `docs.allocated` counts `a.txt` once; the symlink is `Kind::Other` with `apparent == link length`; `locked.unreadable && tree.unreadable_dirs == 1`; `empty/deeper` has `files_below == 0`; progress was called at least once for a fixture padded with 600 small files under `many/`; a pre-cancelled token returns `Cancelled`; a file root returns `NotADirectory`. (Compute the exact expected numbers from `fs::symlink_metadata` in the test rather than hard-coding block counts, which vary by filesystem.)

- [ ] **Step 5: Run** `cd celestina-rs && cargo test -p hematita-core usage` → RED before the implementation, GREEN after; fmt and clippy clean.

---

### Task 3: `usage::empty`, `usage::duplicates`, `usage::layout`, `usage::remove`, `usage::mounts`

**Files:** create the five modules; extend `tests/usage_tree.rs`.

**Interfaces:**

```rust
// empty.rs
pub fn empty_folders(tree: &Tree) -> Vec<NodeId>;   // Dir nodes with files_below == 0 and !unreadable, excluding the root, deepest first

// duplicates.rs
pub const HEAD_BYTES: usize = 64 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Group { pub size: u64, pub nodes: Vec<NodeId> }
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Verified { pub size: u64, pub nodes: Vec<NodeId> }
#[derive(Debug)] pub enum ConfirmError { Cancelled, Read { path: PathBuf, source: io::Error } }
pub fn candidates(tree: &Tree) -> Vec<Group>;       // files with equal allocated > 0, count >= 2, groups by size descending, nodes by id
pub fn confirm(tree: &Tree, group: &Group, cancel: &CancellationToken, progress: &mut dyn FnMut(u64)) -> Result<Vec<Verified>, ConfirmError>;

// layout.rs
pub const MIN_TILE_SHARE: f64 = 0.005;
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
#[derive(Clone, Debug, PartialEq)] pub struct Tile { pub index: Option<usize>, pub rect: Rect }  // None = the merged remainder
pub fn squarify(sizes: &[u64], rect: Rect) -> Vec<Tile>;

// remove.rs
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Removed { pub entries: u64, pub bytes: u64 }
#[derive(Debug)] pub enum RemoveError { Cancelled, Refused { path: PathBuf, reason: Refusal }, Io { path: PathBuf, source: io::Error } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Refusal { IsRoot, Outside, MountRoot, Missing }
pub fn delete_tree(path: &Path, within: &Path, cancel: &CancellationToken) -> Result<Removed, RemoveError>;

// mounts.rs
pub const SHOWN_FILESYSTEMS: &[&str] = &["btrfs", "ext4", "ext3", "ext2", "xfs", "f2fs", "vfat", "exfat", "ntfs", "ntfs3", "fuseblk"];
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Mount { pub source: String, pub target: PathBuf, pub fstype: String }
pub fn parse_mounts(text: &str) -> Vec<Mount>;      // unescapes \040 etc.; keeps only SHOWN_FILESYSTEMS
pub fn is_shown_location(mount: &Mount) -> bool;    // target is "/", "/home", or under /mnt, /run/media, /media
```

- [ ] **Step 1: `empty.rs`** — collect, sort by depth descending (depth via `parent` links), test on the fixture: `[empty/deeper, empty]` in that order; `locked` (unreadable) is not listed.

- [ ] **Step 2: `duplicates.rs`** — `candidates`: a `HashMap<u64, Vec<NodeId>>` over `Kind::File` nodes with `allocated > 0`, keep `len >= 2`, sort. `confirm`: bucket by `hash_head` (read `HEAD_BYTES` with `fs::File::open` + `read`), then within each bucket by `hash_whole` (stream 1 MiB chunks through `DefaultHasher`, calling `progress(bytes)`), then within each bucket compare byte-for-byte pairwise against the first member, splitting non-matching files into new buckets; only buckets with ≥ 2 files become `Verified`. Check `cancel` between files. Tests: `candidates` on the fixture yields two groups (the 4 MiB trio, the 3 KiB pair; the hard link is one node, so the pair is `a.txt`/`b.txt`); `confirm` of the trio yields one `Verified` with `big.bin`+`copy-of-big.bin` and leaves `same-size.bin` out; `confirm` of the pair yields both; a cancelled token → `Cancelled`; a group whose file was removed → `Read`.

- [ ] **Step 3: `layout.rs`** — the squarified algorithm (Bruls, Huizing, van Wijk): sort indices by size descending; sizes below `MIN_TILE_SHARE` of the total are merged into one remainder tile placed last; lay rows along the shorter side, adding items while the worst aspect ratio does not worsen. Tests: areas proportional to sizes within 1e-9; tiles pairwise non-overlapping (check every pair); every tile within `rect`; empty input → empty; one item → the whole rect; the remainder appears once with `index: None`.

- [ ] **Step 4: `remove.rs`** — `delete_tree`: canonicalise `within` and `path` (via `Path::components` normalisation of the lexical path, since `canonicalize` would follow the final symlink — use `fs::symlink_metadata` for existence and compare lexically after stripping `.`/`..`); refuse `IsRoot` when equal, `Outside` when `path` does not start with `within`, `MountRoot` when `symlink_metadata(path).dev() != symlink_metadata(parent).dev()`, `Missing` when absent. Then iterative post-order removal: a symlink → `remove_file` on the link; a file → `remove_file`; a dir → walk with `symlink_metadata`, files first, `remove_dir` last; check `cancel` per entry; count entries and `blocks × 512`. Tests: refuses `within`, refuses `../sibling`, refuses a symlink-to-outside by removing only the link (the target survives), removes a nested tree and reports the count; cancelled → `Cancelled` with nothing else touched after the check.

- [ ] **Step 5: `mounts.rs`** — parser over `/proc/self/mounts` lines (`source target fstype options 0 0`, octal escapes `\040` → space, `\011`, `\012`, `\134`); tests with the author's real first lines (btrfs `/`, devtmpfs dropped, tmpfs dropped, `vfat /boot/efi` kept by fstype but not shown by location, `/mnt/samsung990` shown, a target with `\040`).

- [ ] **Step 6: Run the crate, fmt, clippy; close S1-A** — evidence `2026-09-23-s1-core.md` (test counts, the fixture description, the temp-dir cleanup note), ledger, roadmap, STATUS, inventory (`src/lib.rs`, the seven `usage` files, `tests/usage_tree.rs`, the documents from Task 1), guards, commit `hematita-maintenance: Add the storage usage domain with the walk, the finders, the treemap layout and the guarded deletion`.

---

### Task 4: Locations, browsing, the hub's first two modes, the section — S1-B

**Files:** create `hematita/src/{locations,browse,analysis}.rs`, `hematita/qml/components/{StoragePage,PathCrumbs,LocationList,FolderList}.qml`; modify `Cargo.toml` (`rustix` features `["process", "param", "fs"]`; add `siderita-ops = { path = "../celestina-rs/crates/siderita-ops" }` with the comment "The suite's one trash implementation; the analyzer's trash verb goes through it, reversible from Siderita, instead of growing a second"), `build.rs`, `main.rs`, `Main.qml`, `scripts/smoke.sh`.

- [ ] **Step 1: `locations.rs`** — `read_locations(disk_models: &HashMap<String, String>) -> Vec<Location>` on the caller's thread: parse `/proc/self/mounts` through `usage::mounts`, keep `is_shown_location`, for each `rustix::fs::statvfs(target)` → `total = f_blocks × f_frsize`, `used = (f_blocks − f_bfree) × f_frsize`, readable = `fs::read_dir(target).is_ok()`; kind `system` for `/`, `home` for `/home`, `disk` otherwise; name: `Sistema`/`Inicio` are QML words (publish the kind, QML names them), a disk's name is the model of the block device behind `source` (strip `/dev/` and partition digits: `nvme0n1p1` → `nvme0n1`, `sda1` → `sda`; look it up in the H2 disk-model cache exposed from `sampler.rs` as `pub fn disk_model_of(name: &str) -> Option<String>` reading `/sys/block/<name>/device/model` through the existing `disk_info`), falling back to the mount's last path segment. `/` is dropped when unreadable. Order: system, home, then disks by path.

- [ ] **Step 2: `browse.rs`** — `list_folder(path, generation) -> Result<Listing, io::Error>` on a named thread `hematita-browse`: `read_dir` + `symlink_metadata`, entries with name (lossy for display, bytes kept in the hub), kind, apparent size (`len()` for files, 0 for dirs), sorted dirs first then name (a case-insensitive comparison local to Hematita (`siderita_core::name_order` is a private module; do not expose it for this)); the result queued with its `Generation`, stale ones dropped.

- [ ] **Step 3: `analysis.rs`** (S1-B part): the bridge with all contract properties declared now (analysed/duplicate/selection lists start empty), `mode`, `crumbs`; invokables `open()` (reads locations on a thread, sets `mode = locations`), `enter(index)` for a location or a browse entry, `up()`, `refresh()`; internal `PathBuf` stack for crumbs (byte-exact), `crumbNames` lossy. `scanHere`, `cancel`, `confirmDuplicates`, actions are declared and answer `refused` until S1-C/S1-D fill them (a declared-but-inert invokable is acceptable inside one plan; say so in the evidence).

- [ ] **Step 4: QML** — `StoragePage.qml` (bar + body by mode; the filters and actions capsules present but disabled until S1-C/S1-D), `PathCrumbs.qml` (a `Row` of ghost buttons, `Accessible.role: Navigation`; the last crumb is `text` colour), `LocationList.qml` (card; row: kind glyph `monitor`/`go-home`/`hard-drive`, name (`qsTr("Sistema")`/`qsTr("Inicio")`/model), path in `textMuted`, an occupation bar coloured `glyphAccentAmber` and `qsTr("%1 de %2").arg(used).arg(total)`; Enter/double click enters), `FolderList.qml` (card; row: kind glyph `folder`/`file`, name, apparent size or `—` for dirs; Enter/double click enters a dir; Backspace up). Both lists: integer model, single Tab stop, arrows, viewport restore as `ProcessTable`.

- [ ] **Step 5: `Main.qml`** — sixth section `{ key: "storage", icon: "hard-drive", label: qsTr("Almacenamiento") }`, `HematitaAnalysis { id: analysisHub }`, `StoragePage { analysis: analysisHub; backdrop: window.contentItem }`; `onCurrentSectionChanged` calls `analysisHub.open()` when entering section 5; under `smokeSections` print once `hematita-storage <locationNames.length>` when the walk reaches the section and `mode === "locations"`; `smoke.sh` asserts `hematita-storage [1-9][0-9]*$`.

- [ ] **Step 6: The one build cycle; close S1-B** — evidence `2026-09-23-s1-locations.md` (Limits: no scan; the inert invokables), inventory, guards, commit `hematita-maintenance: Add the storage section with the mount locations and folder browsing`.

---

### Task 5: The scan, the analysed mode, the treemap, the finders — S1-C

**Files:** create `hematita/src/usage_worker.rs`, `hematita/qml/components/{UsageList,Treemap,DuplicateList,DetailsCard}.qml`; modify `analysis.rs`, `StoragePage.qml`, `build.rs`.

- [ ] **Step 1: `usage_worker.rs`** — `pub const PROGRESS_INTERVAL: Duration = 250 ms`; `spawn_scan(root, generation, qt) -> Result<ScanHandle, io::Error>` with a `CancellationToken` in the handle; the thread calls `usage::walk::scan` with a progress closure that rate-limits to `PROGRESS_INTERVAL` and queues `apply_progress(generation, progress)`; at the end queues `apply_tree(generation, Result<Tree, ScanError>)`. `spawn_confirm(tree_snapshot: Arc<Tree>, groups, generation, qt)` likewise for `duplicates::confirm` group by group (queues `apply_verified(generation, group_index, Vec<Verified>)` per group so the page fills progressively). Dropping a handle cancels; the thread is detached (it holds only an `Arc<Tree>` and a `CxxQtThread`; a result arriving after the hub is gone is dropped by `queue`).

- [ ] **Step 2: `analysis.rs`** (S1-C part) — `scanHere()`: issues a new `Generation`, cancels the previous handle, `mode = scanning`, spawns; `apply_progress` sets the progress properties if the generation is current; `apply_tree`: on `Ok` stores `Arc<Tree>`, `current = root`, `mode = analysed`, `publish_current()`; on `Cancelled` returns to `browsing`; on `Root`/`NotADirectory` sets `actionOutcome = failed` and returns to `browsing`. `publish_current()`: `children_by_size(current)`, apply filters (`showDuplicates` → members of candidate groups; `showEmpty` → `empty_folders` set), fill the entry lists, `treemapRects` from `squarify(sizes, unit rect)`, `revision` last. `enterId(id)` sets `current` (a `Dir` with children) and republishes; `up()` goes to `parent` while inside the tree, else leaves to `browsing` at the scanned root. `confirmDuplicates()`: `busy = true`, `spawn_confirm` over `candidates(tree)`; `apply_verified` marks `groupVerified[i] = 1` and replaces the group's members with the verified sets (a verified group that splits becomes several rows); `busy = false` at the end.

- [ ] **Step 3: QML** — `UsageList.qml` (rows: name; `entryShares` bar in kind colour: dir `glyphAccentBlue`, file `glyphAccentViolet`, duplicate `glyphAccentCoral`, empty `glyphAccentAmber`, unreadable `textFaint`; size; share `%`; a selection mark; Space toggles `toggleSelected(id)`; Enter enters a dir); `Treemap.qml` (`Repeater` over `treemapRects.length / 5`; each tile an `AbstractButton` positioned by `x*width`, `y*height`, sized `w*width`, `h*height`, minus a `spaceXs / 2` gap; fill in the kind colour at `accentSoftOpacity` for rest, hover `contentHover` over it, selected `surfaceSelected`; name `Text` elided, shown only when `w*width > 3 * fontCaption`; the remainder tile in `card` with `qsTr("otros")`; click selects, double click enters; arrows move a current tile in row-major order; `Accessible.role: Grouping`, name `qsTr("Mapa de %1").arg(currentName)`); `DuplicateList.qml` (group header rows: `qsTr("%1 copias · %2").arg(count).arg(size)`, a `check` glyph `qsTr("Comprobar contenido")` and after verification `qsTr("verificado")` plus `qsTr("Seleccionar todas menos una")`; member rows indented with name and full path in `textMuted`); `DetailsCard.qml` (name, path, allocated, apparent, files below, `qsTr("Copias idénticas")` list when in a verified group); the scanning state in both panels (`qsTr("%1 archivos · %2").arg(files).arg(bytes)`, `progressPath` elided left, an `x` cancel button `qsTr("Cancelar")`). `StoragePage` gains the analysed body: `RowLayout` list (0.42) + treemap, the filters capsule enabled, `gauge` becomes `view-refresh` (`qsTr("Volver a escanear")`) once analysed.

- [ ] **Step 4: adapter tests** — pure helpers in `analysis.rs` under `#[cfg(test)]`: filter projection on a hand-built tree; the flat `treemapRects` encoding round-trips; `all_but_one(group) -> Vec<NodeId>` keeps the lowest id.

- [ ] **Step 5: The one build cycle; close S1-C** — the smoke still never scans (the section walk stays in `locations`); evidence `2026-09-23-s1-analysis.md`, inventory, guards, commit `hematita-maintenance: Add the scan with progress, the size list and treemap, and the duplicate and empty-folder finders`.

---

### Task 6: Actions — S1-D

**Files:** create `hematita/src/actions.rs`; modify `analysis.rs`, `StoragePage.qml`, `UsageList.qml`, `DuplicateList.qml`, `build.rs`.

- [ ] **Step 1: `actions.rs`** — three named-thread workers, each reporting `(generation, ActionReport { kind, done, total, outcome, removed: Vec<(NodeId, u64)> })` through `qt.queue`:
  - `open_in_siderita(path)`: `Command::new("siderita").arg(path).spawn()` (the desktop entry's `Exec=siderita %U`; a missing binary → `failed`); detached child.
  - `trash(items: Vec<(NodeId, PathBuf)>, cancel)`: per item `siderita_ops::trash(&path, &cancel, &mut |_| {})`; count `done`; `outcome = done | partial | failed`; `removed` lists the successful ids with their allocated bytes (from the tree snapshot).
  - `delete(items, within, cancel)`: per item `usage::remove::delete_tree(&path, &within, &cancel)`; `Refused` counts as not done and is named in the evidence's outcome mapping (a refusal of every item → `refused`).
- [ ] **Step 2: `analysis.rs`** (S1-D part) — `selectedIds` state, `toggleSelected(id)`, `clearSelection()`, `selectAllButOne(group)`; `openSelected()` (first selected), `trashSelected()`, `deleteSelected()` spawn the workers with the current tree's paths; `apply_action` sets the outcome properties, prunes each removed id from the tree (`Tree::prune`), drops pruned ids from the selection and from the candidate/empty caches, republishes; the scanned root itself is never selectable.
- [ ] **Step 3: QML** — actions capsule enabled by selection; `Delete` key → trash flow; `ConfirmDialog.ask` for trashing more than one (`qsTr("¿Enviar %1 elementos (%2) a la papelera?")`) and always for deletion (`qsTr("¿Borrar definitivamente %1 elementos (%2)? No se podrán recuperar.")`, confirm `qsTr("Borrar")`); the outcome line: `done` → `qsTr("%1: hecho (%2)")`, `partial` → `qsTr("%1: %2 de %3")`, `failed` → `qsTr("%1: no se pudo")`, `refused` → `qsTr("%1: rechazado")`; verbs `qsTr("Abrir")`, `qsTr("Papelera")`, `qsTr("Borrar")`. `DetailsCard` shown by the `info` action.
- [ ] **Step 4: tests** — `outcome_of(done, total, refused) -> &'static str`; prune-then-republish keeps ids stable and shares re-sum to ≤ 1.
- [ ] **Step 5: The one build cycle; close S1-D** — evidence `2026-09-23-s1-actions.md` (no action run in verification; the safety invariants checked by grep: `remove_dir_all` appears nowhere, `delete_tree` only in `actions.rs`, `Command::new("siderita")` only there), inventory, guards, commit `hematita-maintenance: Add the storage actions with batch trash, guarded permanent deletion and selection`.

---

### Task 7: Implementation exit — S1-Z

As H5-Z with: `bump hematita milestone --unit S1-Z --summary "Add the storage analyzer"`; `complete-production.sh`; hashes; roadmap `idle`/`none` with `## S1 — closed 2026-09-23`; STATUS `Delivered as 1.1.0: S1`; README's user contract gains "Almacenamiento: mount points, browsing, a scan where you are, size list and treemap, duplicates verified by content, empty folders, and actions through Siderita's trash or a confirmed permanent deletion."; AGENTS.md bullet "Permanent deletion exists only in `hematita-core::usage::remove` and is called only from `actions.rs`; it refuses the scanned root, anything outside it and any mount root, and follows no symlink."; plan archived (`Successor: none`); READMEs; evidence `2026-09-23-s1-production-completion.md`; inventory with the deleted active path; guards; commit `hematita-milestone: Add the storage analyzer`.

---

## Self-review

**Spec coverage:** §1 items 1–6 → Tasks 4 (locations, browsing, scan-here entry), 5 (scan, list+treemap, filters, verified duplicates, empties), 6 (actions, batch, all-but-one); §3 decisions → Task 2 (allocated blocks, hard links once, symlinks not followed), Task 3 (empty definition); §4 modules → Tasks 2–3 one to one, plus `mounts`; §5 → Tasks 4–6; §6 components → Tasks 4–6; §7 phases → the ledger table; §8 verification → each task's tests, the smoke line in Task 4, `VAL-S1` in Task 1; §9 decisions → carried in Global Constraints (no dependency beyond `rustix` `fs` and `siderita-ops`; session-only results; deletion in `hematita-core`).

**Placeholder scan:** the walk's `entry.metadata_symlink()` line is explicitly replaced by `fs::symlink_metadata(entry.path())` in the text; `Display` arms are described, not elided of content. No TBD.

**Type consistency:** `NodeId`, `Tree::{path_of, children_by_size, prune}` (Task 2) ↔ Tasks 5–6; `Progress`, `ScanError` ↔ Task 5's worker; `Group`, `Verified`, `confirm` ↔ Task 5; `Tile`/`squarify` ↔ `treemapRects`; `Removed`, `RemoveError`, `delete_tree(path, within, cancel)` ↔ Task 6; `Mount`, `parse_mounts`, `is_shown_location` ↔ Task 4; the QML property names are the camelCase of the contract and are what the pages read; `ConfirmDialog.ask(question, confirmText, payload)` from H5 is reused unchanged.
