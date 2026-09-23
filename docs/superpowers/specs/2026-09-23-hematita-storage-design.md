<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita storage analyzer — design

- **Date:** 2026-09-23
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** a sixth section of Hematita, `Almacenamiento`, replacing and
  improving GNOME Disk Usage Analyzer (Baobab)
- **Checkpoint:** `S1`; version 1.1 at closure

## 1. Goal and scope

The author uses Baobab to find what fills a disk and finds two things wrong
with it: the result is not clear, and once something is found there is little
one can do about it. The section therefore answers two questions — where the
weight is, and what to do with it — and adds two findings Baobab lacks:
duplicate files and empty folders.

In scope:

1. an initial zone listing the mount points: `/` as *Sistema* when readable,
   `/home` as *Inicio*, every mount under `/mnt`, `/run/media` and `/media`,
   each with real occupation;
2. browsing any location like a file manager before scanning, and
   a "scan here" action (`qsTr("Escanear aquí")`) at any depth, which analyses the folder one is in;
3. a progressive, cancellable scan that never crosses to another device;
4. the result as a size-ordered list beside a treemap of the current folder;
5. filters: duplicates (candidates by size at once, verified by content on
   demand) and empty folders;
6. actions on a selection of one or many entries: open in Siderita, move to
   the trash, delete permanently (confirmed), details; "select all but one"
   per duplicate group.

Out of scope: persisting scan results across runs, scanning across mounts,
network filesystems' quotas, scheduled scans, any shell integration (the
shell is in standby), and any privilege (a folder the user cannot read is
reported, never escalated).

## 2. Constraints the design honours

- Root `AGENTS.md`: pure domain in `celestina-rs/`, Qt adaptation in
  `hematita/src`, presentation in `hematita/qml`, tokens from
  `celestina-style`, every QML file registered, no blocking IO on the Qt
  thread, typed errors, no production `unwrap`, dependencies justified. A
  write never removes the source before confirming the destination; the trash
  operation is the suite's, reused.
- The reuse rule: `siderita-ops::trash` is the one trash implementation and
  is consumed as a crate, not copied; `celestina-core::{CancellationToken,
  Generation}` are the shared cancellation and staleness types.
- Style: grouped cards, one accent per kind from existing glyph tokens, no
  `Canvas`; the treemap is a set of rectangles.
- Author rules: icon-first actions, no tooltips, Spanish product copy through
  `qsTr()` only, builds only when strictly necessary.
- No new dependency: duplicate confirmation uses `std::hash::DefaultHasher`
  for bucketing and byte-for-byte comparison for the verdict.

## 3. Decisions taken in brainstorming

- Layout B: size list + treemap of the current folder; the list answers "what
  do I delete", the treemap "where is the weight"; filters tint both.
- Duplicates: option 3 — instant candidates by allocated size, and a
  `Comprobar contenido` step that verifies before anything is deleted.
- Start zone: mount points; enter one, browse, scan where you are.
- Actions: option 3 — open, trash, delete permanently, details, and batch
  selection with "all but one" per duplicate group.
- Sizes are allocated blocks (`st_blocks × 512`, what `du` shows and what
  matters on compressed btrfs); the apparent size appears in details.
- An empty folder contains no file at any depth.
- Hard links count once per scan (`(dev, ino)` remembered); symbolic links are
  listed as themselves and never followed.

## 4. `hematita-core::usage`

Pure module, testable on a temporary directory, no Qt.

- `tree`: `Node { name: OsString, kind: Dir | File | Other, allocated: u64,
  apparent: u64, children: Vec<NodeId>, files_below: u64, unreadable: bool }`
  stored in a `Vec<Node>` indexed by `NodeId(u32)`; `Tree { root: NodeId,
  path: PathBuf, device: u64, nodes, unreadable_dirs: u32 }`. Every node's
  `allocated`/`apparent`/`files_below` are aggregates of its subtree.
- `walk`: `scan(root: &Path, cancel: &CancellationToken, progress: &mut dyn
  FnMut(Progress)) -> Result<Tree, ScanError>`. Iterative (explicit stack),
  `symlink_metadata` only, stays on `root`'s device, reports `Progress
  { files, bytes, current: PathBuf }` every 512 entries, marks an unreadable
  directory and continues, returns `ScanError::Cancelled` or
  `ScanError::Root { path, source }` when the root itself cannot be read.
- `empty`: `empty_folders(tree: &Tree) -> Vec<NodeId>` — directories with
  `files_below == 0`, deepest first so a parent whose only content is empty
  folders is also listed.
- `duplicates`: `candidates(tree: &Tree) -> Vec<Group>` where `Group { size:
  u64, nodes: Vec<NodeId> }` holds files of equal allocated size > 0 and
  count ≥ 2; `confirm(tree, group, cancel, progress) -> Result<Vec<Verified>,
  ConfirmError>` with `Verified { size, nodes }`: bucket by hash of the first
  64 KiB, then by hash of the whole file, then split by byte comparison; only
  files proven identical are returned together.
- `layout`: `squarify(sizes: &[u64], rect: Rect) -> Vec<Rect>` — the
  squarified treemap of one folder's children in a unit rectangle, sorted
  descending, with a minimum area below which children are merged into one
  `otros` rectangle; pure arithmetic, tested for area proportionality, no
  overlap, containment.
- `remove`: `delete_tree(path: &Path, within: &Path, cancel) ->
  Result<Removed, RemoveError>` — refuses a path equal to `within` or outside
  it, refuses a mount root, never follows symlinks (removes the link itself),
  iterative, cancellable, reports how much it removed; the only permanent
  deletion in the suite and the one place it is spelled.

Errors are typed and carry the path. Tests build a fixed tree under
`tempdir` with known sizes, a hard link, a symlink pointing outside, a
directory without read permission, nested empty folders, two true duplicate
pairs and one same-size-different-content pair; `squarify` has its own
numeric tests; `delete_tree` has refusal tests.

## 5. `hematita/src`

- `locations.rs`: parses `/proc/self/mounts` (pure parser in
  `hematita-core::usage::mounts`, tested), keeps real filesystems only
  (btrfs, ext4, xfs, vfat, exfat, ntfs, f2fs, fuse.* that are not portals),
  `statvfs` per mount for capacity/used, names the device with the disk
  model cache of H2, orders `/` (readable check), `/home`, then the rest by
  path. Refreshed when the section is entered and every `SERVICE_TICKS`.
- `browse.rs`: lists one folder (name, kind, apparent size, entry count for
  directories without descending) on a named thread, `Generation`-stamped.
- `usage_worker.rs`: one named thread per scan holding a `CancellationToken`;
  queues `Progress` to Qt at most every 250 ms and the `Tree` at the end; a
  new scan cancels the previous; results older than the current
  `Generation` are dropped.
- `analysis.rs` (`HematitaAnalysis`, the hub): state `mode` (`locations |
  browsing | scanning | analysed`), `crumbs` (list of names + paths), the
  location lists (`locationNames, locationPaths, locationKinds, locationUsed,
  locationTotal, locationReadable`), the browse lists (`browseNames,
  browseKinds, browseApparent`), progress (`progressFiles, progressBytes,
  progressPath`), the analysed lists for the current node (`entryIds,
  entryNames, entryKinds, entryAllocated, entryApparent, entryShares,
  entryFilesBelow, entryEmpty, entryDuplicate, entryUnreadable`) and
  `treemapRects` (flat list `[id, x, y, w, h, …]` normalised to 0..1),
  filters (`showDuplicates, showEmpty`), the duplicate lists (`groupSizes,
  groupCounts, groupVerified, memberGroups, memberIds, memberPaths`),
  `selection` (list of entry ids), `revision` last, `startFailed`.
  Invokables: `enter(path)`, `enterId(id)`, `up()`, `scanHere()`, `cancel()`,
  `confirmDuplicates()`, `selectAllButOne(group)`, `toggleSelected(id)`,
  `clearSelection()`. The hub owns the `Tree` and answers navigation from it
  without touching the disk.
- `actions.rs`: `openInSiderita(path)` spawns `siderita <path>` on a thread;
  `trash(ids)` runs `siderita_ops::trash` per path on a thread with progress
  and cancellation; `deletePermanently(ids)` runs `usage::remove::delete_tree`
  per path within the scanned root. Outcomes are typed (`done | partial |
  failed | refused`) with counts; on success the hub prunes the removed nodes
  and re-aggregates ancestors instead of rescanning.

## 6. `hematita/qml`

Sixth section `{ key: "storage", icon: "hard-drive", label: qsTr("Almacenamiento") }`.

- `StoragePage.qml`: the bar (`PathCrumbs.qml` with one ghost button per
  crumb; a filters capsule with `copy` → `qsTr("Duplicados")` and `folder` →
  `qsTr("Vacías")`, checkable; an actions capsule with `gauge` →
  `qsTr("Escanear aquí")` while browsing, `folder-open` → `qsTr("Abrir en
  Siderita")`, `user-trash` → `qsTr("Papelera")`, `x` → `qsTr("Borrar
  definitivamente")`, `info` → `qsTr("Detalles")`, enabled by selection) and
  the body by `mode`.
- `LocationList.qml`: one card, one row per location: name, path, an
  occupation bar in the kind colour, `usado / total`; Enter enters.
- `FolderList.qml`: the browse listing; double click or Enter enters,
  Backspace goes up.
- `UsageList.qml` + `Treemap.qml` side by side: the list has name, size,
  share of the parent and a proportional bar coloured by kind (directory
  blue, file violet, duplicate coral, empty amber, unreadable faint); the
  treemap is a `Repeater` of rectangle buttons over `treemapRects`, name
  inside when it fits, hover and selection with the content ramp, double
  click enters; with a filter active, non-matching rectangles fade to
  `unavailableContentOpacity`. While scanning both show the progress
  (`qsTr("%1 archivos · %2").arg(files).arg(bytes)`, the current path elided)
  and a cancel glyph; the result replaces the progress in place.
- `DuplicateList.qml`: with the filter on, entries grouped by set (size and
  copies), `check` → `qsTr("Comprobar contenido")` per group or for all,
  `qsTr("verificado")` after, `qsTr("Seleccionar todas menos una")` per
  verified group.
- `ConfirmDialog` (existing) for trashing more than one entry and always for
  permanent deletion, with count and total size in the question.
- Keyboard: each list is one Tab stop; arrows move; Backspace up; Enter
  enters; Space toggles selection; Delete asks to trash; the treemap is
  reachable with arrows in row-major order.
- Details: a small card with name, path, allocated and apparent sizes, files
  below, and for a duplicate the other copies.

## 7. Phases

| Unit | Outcome | Build |
|---|---|---|
| S1-A | `hematita-core::usage` complete with tests | none |
| S1-B | locations, browsing, `StoragePage`, `PathCrumbs`, `LocationList`, `FolderList`, sixth section, smoke line | one |
| S1-C | scan worker, analysed mode, `UsageList`, `Treemap` (squarify in Rust), progress and cancel, filters, `DuplicateList` with confirmation | one |
| S1-D | actions: open in Siderita, batch trash through `siderita-ops`, permanent deletion with safeguards, pruning, selection and all-but-one | one |
| S1-Z | `complete-production.sh`, 1.1.0, documents, plan archived | one |

## 8. Verification

- Crate: the fixed-tree tests (sizes, hard link once, symlink not followed,
  unreadable dir marked, nested empties, true and false duplicates), the
  squarify numeric tests, the mounts parser tests, the deletion refusal
  tests.
- Adapter: tests for the pure decisions (filter projection, all-but-one,
  pruning and re-aggregation).
- Smoke: the section walk covers six sections and asserts
  `hematita-storage <locations>` with locations > 0; no scan, no action runs
  in verification.
- Guards before each commit.
- `VAL-S1` (author): scan the whole home with progress visible and cancel
  halfway once; enter a `/mnt` disk; enable duplicates, verify a group,
  select all but one and trash them; delete something permanently on the USB
  disk; keyboard through lists and treemap; idle cost after a scan.

## 9. Decisions recorded here

- Own scanner over `siderita-core::scan` (a single-folder listing without
  aggregates) and over external tools (`du`, `fdupes`): progress,
  cancellation and testability need the walk to be ours.
- Hash by the standard library plus byte comparison instead of a
  cryptographic crate: the comparison makes the verdict exact; the hash only
  buckets.
- Results live for the session; a rescan is a button. Persistence is a later
  decision if the author asks.
- Permanent deletion is a new, small, guarded operation in `hematita-core`
  rather than an extension of `siderita-ops`: it is not a file-manager verb
  and it must never be reachable from Siderita by accident.
