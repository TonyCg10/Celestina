<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita S2 — Hardening the storage analyzer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close what the S1 reviews left open: a permanent deletion that cannot be redirected by a folder swapped for a link while it runs, a tree that stays truthful after a deletion fails or is cancelled midway, the memory and safety details of the walk and the hub, and a hub small enough to reason about.

**Architecture:** `usage::remove` is rewritten on directory file descriptors: the analysed root is opened once with `O_DIRECTORY | O_NOFOLLOW`, every component down to the target is opened the same way from the previous descriptor, and entries are removed with `unlinkat` relative to their parent descriptor — a path is never re-resolved after the listing. A deletion that stops midway returns what it removed, and the hub grafts a fresh sub-scan of that entry in place of the stale subtree. The hub loses its analysed-session state to `analysis_session.rs` (tree, findings, verdicts, marks, selection, epochs, pruning, grafting), keeping only Qt glue and publication in `analysis.rs`. `hematita-core` gains `rustix` (`fs`), justified: the `*at` family of syscalls is what makes the deletion honest, and `std` does not expose it.

**Tech Stack:** Rust 1.97.1, `rustix 1.1.4` with `fs` (new to `hematita-core`; already in `hematita`), cxx-qt 0.9.1, QML.

**Spec:** [docs/superpowers/specs/2026-09-23-hematita-storage-design.md](../specs/2026-09-23-hematita-storage-design.md) §4 `remove`, §5 pruning; the S2-A booking in `hematita/ROADMAP.md`; the S1 final review's findings recorded in `hematita/STATUS.md` (removal window, hub debt, stale size after a partial removal).

## Global Constraints

- **The Celestina shell is in standby.** No reference to `celestina/` or `celestina-shell-core`.
- **Language contract:** English in code and documents; Spanish only inside `qsTr()`.
- **Build budget:** two application build cycles — S2-B (release build + release-profile tests/clippy + `verify-production.sh`) and S2-Z (`complete-production.sh`). S2-A is crate-only. No window; no scan or action in verification.
- **Commits:** per unit after review; `hematita:` prefix; the H1 generator (`mkinv.py`); inventories under `hematita/docs/inventories/2026-09-23-s2-hardening/`; explicit pathspecs; hooks never bypassed.
- **Safety invariants (unchanged and strengthened):** deletion only from `actions.rs`, only for ids resolved against the current tree, within the scanned root, refusing the root, outside paths, mount roots (by boundary set and by `st_dev`), symlinked components and changed identities; every destructive action confirmed; outcomes typed; the hub prunes or grafts only what the worker reports.
- **Rust invariants:** no `unsafe` (rustix is safe Rust); no production `unwrap`/`expect`/`panic!`; blocking IO never on the Qt thread; generation and epoch discipline kept.
- **QML invariants:** tokens only; no suppressions; qmllint row 0; the dialog's confirmed count is bound.
- **Dependency justification** (`hematita-core/Cargo.toml`): "`rustix` for the `*at` syscalls (`openat`, `unlinkat`, `statat`, `fstat`, `Dir`): a permanent deletion that opens each directory once and removes children relative to that descriptor cannot be redirected by a component swapped for a symbolic link while it runs; `std` exposes no `unlinkat`. Safe Rust, `fs` feature only."

---

## File structure

| Path | Responsibility |
|---|---|
| `celestina-rs/crates/hematita-core/Cargo.toml` | `rustix = { version = "1.1.4", default-features = false, features = ["std", "fs"] }` |
| `…/usage/remove.rs` | descriptor-based deletion; `Failure { error, removed }` on a midway stop |
| `…/usage/walk.rs` | `seen_inodes` gated on `nlink > 1`; `scan_subtree` for grafts |
| `…/usage/tree.rs` | `Tree::graft(id, Tree)`; `Tree::is_live(id)` |
| `…/tests/usage_tree.rs` | new cases |
| `hematita/src/actions.rs` | `Step::Partial(Removed)`; report carries `partial: Vec<NodeId>` |
| `hematita/src/analysis_session.rs` | the analysed session: tree, findings, verdicts, marks, selection, epochs, prune, graft |
| `hematita/src/analysis.rs` | Qt glue and publication only |
| `hematita/src/usage_worker.rs` | `spawn_graft(root_path, boundaries, id, generation, qt)` |
| `hematita/qml/components/StoragePage.qml` | bound confirmation count |
| documents | plan, roadmap, STATUS, VALIDATION (`VAL-S2`), evidence, inventories |

Ledger units:

| Unit | Kind | Content | Build |
|---|---|---|---|
| S2-A | `hematita-maintenance` | descriptor-based deletion with partial results; walk nlink gate; `graft`; tests; S2 opened | none |
| S2-B | `hematita-maintenance` | hub split; partial removals grafted; `Arc` drop before `make_mut`; live-id check; bound confirmation | one |
| S2-Z | `hematita-bug` | 1.1.1, `complete-production.sh`, documents, plan archived | one |

---

### Task 1: Open S2 in the documents

As S1's Task 1: plan `hematita/docs/plans/active/2026-09-23-s2-hardening.md` (Plan ID `s2-hardening`, checkpoint `S2`, `VAL-S2`, hypothesis "a deletion that walks descriptors cannot be redirected, and a tree that grafts what a stopped deletion left behind never shows a size that is no longer true"); roadmap `active`/`S2`, rows S2-A (dep S1-Z), S2-B, S2-Z, `## S2 — opened 2026-09-23` replacing the planned-first-unit section; STATUS; README of plans; VALIDATION:

```markdown
## VAL-S2 — Hardened deletion on the real session

- **Status:** pending
- **Related implementation:** S2
- **Requires:** the deployed Hematita 1.1.1; a USB disk with disposable folders
- **Procedure:** on the USB disk, scan, select a large folder and delete it, cancel halfway: the row shows the new size within a second and matches `du`; make a folder unwritable inside a selected tree (`chmod 000`), delete the selection: the outcome names the count, the failed entry keeps its true size; replace a selected folder by a symbolic link to another folder after the scan and delete: refused, the link's target untouched; select a folder and its child and read the dialog's count (one); keyboard as in VAL-S1
- **Pass condition:** no size on screen disagrees with `du` after any stopped deletion; the symbolic-link case is refused; the dialog count equals what is acted on; the section still idles under 2 % CPU
- **Result:** not run
- **Evidence:** none
```

---

### Task 2: Descriptor-based deletion with partial results — S2-A

**Files:** `remove.rs`, `walk.rs`, `tree.rs`, `Cargo.toml`, `tests/usage_tree.rs`.

**Interfaces:**

```rust
// remove.rs
#[derive(Debug)]
pub struct Failure { pub error: RemoveError, pub removed: Removed }
pub fn delete_tree(path: &Path, within: &Path, boundaries: &HashSet<PathBuf>, expected: Option<(u64, u64)>, cancel: &CancellationToken) -> Result<Removed, Failure>;
// `expected` is the scanned (dev, ino) of the target; `None` skips the identity check (tests only).

// walk.rs
pub fn scan_subtree(root: &Path, boundaries: &HashSet<PathBuf>, cancel: &CancellationToken) -> Result<Tree, ScanError>;  // `scan` without progress, for grafts

// tree.rs
impl Tree {
    pub fn graft(&mut self, id: NodeId, fresh: Tree) -> bool;   // replaces the subtree at `id` (same name) with `fresh`'s nodes re-indexed; re-aggregates ancestors; false when `id` is not live
    pub fn is_live(&self, id: NodeId) -> bool;                  // reachable from the root through `children`
}
```

- [ ] **Step 1: tests first** (in `tests/usage_tree.rs`): (a) `delete_tree` with `expected` mismatched → `Failure { error: Refused Changed, removed: 0 }`; (b) a folder made unreadable after listing… replaced by: a fixture whose inner directory is `chmod 000` *before* deletion → `Failure { error: Io, removed: N > 0 }` counting the siblings removed before the failure (order is post-order, so the failure lands after some removals); (c) the symlink-swap test from S1-A2 still refuses; (d) a new test: after opening, replace an intermediate directory by a symlink *during* deletion cannot be simulated deterministically — instead test that removal happens through `unlinkat` on the parent descriptor by deleting a tree whose parent directory was renamed after the descriptors were opened: use a fixture, start `delete_tree` on a thread, and in the test thread `rename` the analysed root's parent before joining… too racy; skip and document. Test `graft`: a hand-built tree where a subtree is replaced by a fresh two-node tree, ancestors' totals updated, ids of untouched nodes unchanged, `is_live` false for the old subtree's ids. Test `scan_subtree` equals `scan` on the same folder minus progress. Test the nlink gate: a file with `nlink == 1` is not inserted into `seen_inodes` (observe through a debug counter exposed as `pub fn scan_with_stats` or by asserting the set is not needed — simplest: expose `Tree.hard_link_names: u64` counting second names, and assert it is 1 for the fixture's hard link, 0 for a fixture without links).

- [ ] **Step 2: implement `remove.rs` on descriptors**

```rust
use rustix::fs::{fstat, openat, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags, CWD};
use std::os::fd::OwnedFd;

const DIR_FLAGS: OFlags = OFlags::RDONLY.union(OFlags::DIRECTORY).union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);

/// Opens the analysed root, then each component of `target` relative to the
/// previous descriptor with `O_NOFOLLOW`; a component that is a link fails
/// with ELOOP and is reported as `Symlink { at }`.
fn open_parent(root: &Path, target: &Path) -> Result<(OwnedFd, OsString), RemoveError> { … }

pub fn delete_tree(path, within, boundaries, expected, cancel) -> Result<Removed, Failure> {
    // lexical checks as before (IsRoot, Outside), then:
    let (parent_fd, name) = open_parent(&root, &target)?;
    let stat = statat(&parent_fd, &name, AtFlags::SYMLINK_NOFOLLOW)?;   // Missing on ENOENT
    if let Some((dev, ino)) = expected { if (stat.st_dev, stat.st_ino) != (dev, ino) { refuse Changed } }
    if boundaries.contains(&target) { refuse MountRoot }
    let parent_stat = fstat(&parent_fd)?; if stat.st_dev != parent_stat.st_dev { refuse MountRoot }
    let mut removed = Removed::default();
    match remove_entry(&parent_fd, &name, &stat, stat.st_dev, boundaries, &target, cancel, &mut removed) {
        Ok(()) => Ok(removed),
        Err(error) => Err(Failure { error, removed }),
    }
}

/// Post-order over descriptors: a directory is opened from its parent with
/// DIR_FLAGS, its entries listed with `Dir::read_from`, each removed relative
/// to that descriptor, and finally the directory itself with REMOVEDIR
/// relative to *its* parent. `path` is carried only for error messages and
/// the boundary set.
fn remove_entry(parent: &OwnedFd, name: &OsStr, stat: &Stat, device: u64, boundaries, path: &Path, cancel, removed: &mut Removed) -> Result<(), RemoveError> {
    if cancel.is_cancelled() { return Err(RemoveError::Cancelled); }
    if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
        if boundaries.contains(path) || stat.st_dev != device { return Err(Refused MountRoot); }
        let fd = openat(parent, name, DIR_FLAGS, Mode::empty())?;
        let mut seen = HashSet::new();
        for entry in Dir::read_from(&fd)? {
            let entry = entry?;
            let child = entry.file_name();
            if child == "." || child == ".." { continue; }
            let child_stat = statat(&fd, child, AtFlags::SYMLINK_NOFOLLOW)?;
            remove_entry(&fd, child, &child_stat, device, boundaries, &path.join(OsStr::from_bytes(child.to_bytes())), cancel, removed)?;
        }
        unlinkat(parent, name, AtFlags::REMOVEDIR)?;
        removed.entries += 1;
    } else {
        unlinkat(parent, name, AtFlags::empty())?;
        removed.entries += 1;
        removed.bytes += bytes_of(stat, &mut seen);   // hard-linked inode once, as before
    }
    Ok(())
}
```

Recursion depth equals directory depth here (a descriptor per level is what makes it safe); keep an explicit depth cap (`MAX_DEPTH = 4096`) that fails with `Io` beyond it. Map `rustix::io::Errno` into `io::Error` for `RemoveError::Io`. `Dir::read_from` needs a duplicated fd or the iterator consumes it — read rustix's docs: `Dir::read_from(&fd)` takes `AsFd` and keeps its own; confirm and note. `check_identity` moves into `delete_tree` through `expected`; keep the standalone function for the trash path.

- [ ] **Step 3: `walk.rs`** — `seen_inodes.insert` only when `meta.nlink() > 1`; `Tree.hard_link_names` counter; `scan_subtree`.

- [ ] **Step 4: `tree.rs`** — `graft`: remove the old subtree's ids from their parent's children (they stay in `nodes` detached, like `prune`), append `fresh.nodes` re-indexed by an offset with `parent` fixed up (the fresh root's parent becomes the old node's parent, its name kept), re-aggregate ancestors with the delta; `is_live` walks `parent` links to the root and checks each child membership.

- [ ] **Step 5: crate green; close S2-A** — evidence `2026-09-23-s2-core.md` (the dependency justification, what the descriptor walk guarantees and what it still cannot: a rename of the analysed root itself during deletion, documented), inventory, guards, commit `hematita-maintenance: Add the descriptor-based deletion with partial results and the subtree graft`.

---

### Task 3: The hub split and the partial-removal graft — S2-B

**Files:** `actions.rs`, `usage_worker.rs`, `analysis_session.rs` (new), `analysis.rs`, `analysis_view.rs` (unchanged API), `StoragePage.qml`, `build.rs` (rerun line), `main.rs` (`mod analysis_session;`).

- [ ] **Step 1: `analysis_session.rs`** — move out of `HematitaAnalysisRust`: `tree: Option<Arc<Tree>>`, `current`, `findings`, `verdicts`, `unreadable_groups`, `selection`, `confirm`, `confirm_epoch`, `action`, `action_epoch`, `*_exact/*_below` marks, `candidate_copies`, and the methods `mark_duplicates`, `stop_confirm`, `confirm_current`, `action_items`, `selected_totals`, `prune`, `chain`, `node_id`, plus the new `graft`. `Session` is a plain struct with plain methods and unit tests; `analysis.rs` keeps the bridge, the locations/browse state, the workers' spawning, `apply_*` and `publish_*`, each delegating to `self.session`. Target: `analysis.rs` under 800 lines; record the two numbers in the evidence.
- [ ] **Step 2: `prune` drops the confirm worker's `Arc` first** — `stop_confirm()` already drops the handle; the detached thread may still hold its clone, so `Arc::make_mut` may copy; make the worker hold a `Weak<Tree>` upgraded per group instead, so a cancelled worker releases the strong count at its next step; document the residual copy when a group is mid-comparison.
- [ ] **Step 3: `node_id` rejects pruned ids** through `Tree::is_live`.
- [ ] **Step 4: partial removals** — `actions.rs`: `Step::Partial(Removed)` from `Failure { removed }` with `removed.entries > 0`; the report gains `partial: Vec<NodeId>`; the hub, for each partial id, asks `usage_worker::spawn_graft(tree.path_of(id), boundaries, id, generation, qt)` (a thread running `scan_subtree`), and `apply_graft(generation, id, Result<Tree, ScanError>)` calls `session.graft(id, fresh)` (or prunes the id if the folder is gone) and republishes; `busy` while grafts run; a rescan cancels them.
- [ ] **Step 5: bound confirmation** — `requestTrash`/`requestDelete` pass `{ kind, count: selectedCount }`; `trashSelected(expected: i32)` / `deleteSelected(expected)` refuse (`actionOutcome = "refused"`) when `expected != selected_totals().0`.
- [ ] **Step 6: the one build cycle; close S2-B** — evidence `2026-09-23-s2-hub.md` (line counts before/after, the graft flow, the `Weak` note), inventory, guards, commit `hematita-maintenance: Split the analysis hub and graft what a stopped deletion left behind`.

---

### Task 4: Implementation exit — S2-Z

As S1-Z with kind `bug`: `bump hematita bug --unit S2-Z --summary "Fix the deletion window, the stale sizes after a stopped deletion and the analysis hub size"`; `complete-production.sh`; roadmap `idle`/`none` with `## S2 — closed 2026-09-23`; STATUS `Delivered as 1.1.1: S2`, the removal-window and hub-debt entries replaced by what shipped and the residual (rename of the analysed root during deletion; `Weak` mid-comparison copy); AGENTS.md's deletion bullet updated ("walks descriptors with `O_NOFOLLOW` and removes with `unlinkat`"); plan archived (`Successor: none`); evidence `2026-09-23-s2-production-completion.md`; inventory; commit `hematita-bug: Fix the deletion window, the stale sizes after a stopped deletion and the analysis hub size`.

---

## Self-review

**Coverage:** every S2-A booking line maps to a step (partial removals → Task 2 `Failure` + Task 3 graft; `openat` deletion → Task 2; `nlink` gate → Task 2 Step 3; `Arc` drop → Task 3 Step 2; pruned ids → Task 3 Step 3; bound count → Task 3 Step 5; hub split → Task 3 Step 1). **Placeholders:** the abandoned racy test in Task 2 Step 1 (d) is explicitly replaced by documentation. **Types:** `Failure`, `Removed`, `graft`, `is_live`, `scan_subtree` (Task 2) ↔ Task 3; `Step::Partial`, `partial` ↔ `apply_graft`; QML payload `{ kind, count }` ↔ `trashSelected(expected)`.
