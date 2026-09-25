//! The occupation of one folder while a modal shows it: which scan is
//! current, the tree it produced and where the person stands inside it.
//!
//! Plain state with no Qt in it, so the rules a stale scan, a drill and a
//! close obey are tested without a window. The walk, the tree and the
//! projection are `hematita_core::usage`'s; this only decides which tree is
//! current and projects the folder the person is looking at.

use std::path::PathBuf;

use celestina_core::CancellationToken;
use hematita_core::usage::layout::{squarify, Rect};
use hematita_core::usage::tree::{Kind, NodeId, Tree};
use hematita_core::usage::view::{
    children_rows, flat_rects, unreadable_below, MAX_ROWS, REMAINDER_ID,
};

/// The scan the modal asked for last and what it found.
#[derive(Default)]
pub struct UsageSession {
    /// Moves on every `begin` and `close`, so a result carrying an older one
    /// is known to be stale.
    pub generation: u64,
    pub root: Option<PathBuf>,
    pub tree: Option<Tree>,
    pub current: Option<NodeId>,
    /// [`unreadable_below`] of `tree`, indexed by node.
    pub unreadable: Vec<u32>,
    /// The running scan's token; `None` once its tree landed or it stopped.
    pub cancel: Option<CancellationToken>,
    /// Which surface opened the session last; only it may close it.
    pub owner: String,
}

/// The current folder as plain values, index-aligned per row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projection {
    /// Folder names from the scanned root down to the current folder.
    pub crumbs: Vec<String>,
    pub root_path: PathBuf,
    pub current_path: PathBuf,
    pub total_bytes: f64,
    pub files_below: f64,
    pub folders_below: f64,
    pub unreadable_below: f64,
    pub other_devices: f64,
    pub row_ids: Vec<f64>,
    pub row_names: Vec<String>,
    pub row_kinds: Vec<String>,
    pub row_allocated: Vec<f64>,
    pub row_shares: Vec<f64>,
    pub row_files: Vec<f64>,
    pub row_unreadable: Vec<f64>,
    pub row_merged: Vec<f64>,
    /// `[id, x, y, w, h]` per tile, in the unit square.
    pub rects: Vec<f64>,
}

impl UsageSession {
    /// Starts a scan of `root`: bumps the generation, cancels the running
    /// scan and forgets the tree. `None` when `root` is the folder already
    /// scanned or being scanned, whose tree or walk is kept.
    ///
    /// `owner` takes the session over: from now on only it may close it.
    pub fn begin(&mut self, root: PathBuf, owner: &str) -> Option<(u64, CancellationToken)> {
        owner.clone_into(&mut self.owner);
        if self.root.as_deref() == Some(root.as_path())
            && (self.tree.is_some() || self.cancel.is_some())
        {
            return None;
        }
        if let Some(token) = self.cancel.take() {
            token.cancel();
        }
        self.generation = self.generation.wrapping_add(1);
        self.root = Some(root);
        self.tree = None;
        self.current = None;
        self.unreadable.clear();
        let token = CancellationToken::new();
        self.cancel = Some(token.clone());
        Some((self.generation, token))
    }

    /// Takes the tree a scan produced; false, keeping nothing, when a later
    /// `begin` or a `close` made that scan stale.
    pub fn accept(&mut self, generation: u64, tree: Tree) -> bool {
        if generation != self.generation {
            return false;
        }
        self.unreadable = unreadable_below(&tree);
        self.current = Some(tree.root);
        self.tree = Some(tree);
        self.cancel = None;
        true
    }

    /// The scan that finished under `generation` stopped without a tree.
    pub fn failed(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.cancel = None;
        true
    }

    /// [`Self::close`] when `owner` opened the session last; false, changing
    /// nothing, when another surface has taken it over since.
    pub fn close_by(&mut self, owner: &str) -> bool {
        if self.owner != owner {
            return false;
        }
        self.close();
        true
    }

    /// Cancels the running scan and forgets everything; any result still on
    /// its way is stale.
    pub fn close(&mut self) {
        self.owner.clear();
        if let Some(token) = self.cancel.take() {
            token.cancel();
        }
        self.generation = self.generation.wrapping_add(1);
        self.root = None;
        self.tree = None;
        self.current = None;
        self.unreadable.clear();
    }

    /// Moves into `id` when it is a folder on the scanned device directly
    /// inside the current one.
    pub fn enter(&mut self, id: i32) -> bool {
        let (Some(tree), Some(current)) = (self.tree.as_ref(), self.current) else {
            return false;
        };
        let Ok(raw) = u32::try_from(id) else {
            return false;
        };
        let id = NodeId(raw);
        let inside = tree
            .node(current)
            .is_some_and(|node| node.children.contains(&id));
        let folder = tree
            .node(id)
            .is_some_and(|node| node.kind == Kind::Dir && !node.other_device);
        if !inside || !folder {
            return false;
        }
        self.current = Some(id);
        true
    }

    /// Moves to the current folder's parent; false at the scanned root.
    pub fn up(&mut self) -> bool {
        let (Some(tree), Some(current)) = (self.tree.as_ref(), self.current) else {
            return false;
        };
        if current == tree.root {
            return false;
        }
        match tree.node(current).and_then(|node| node.parent) {
            Some(parent) => {
                self.current = Some(parent);
                true
            }
            None => false,
        }
    }

    /// Moves to the crumb at `depth` (0 is the scanned root) in one step;
    /// false when `depth` is the current folder or deeper.
    pub fn up_to(&mut self, depth: usize) -> bool {
        let (Some(tree), Some(current)) = (self.tree.as_ref(), self.current) else {
            return false;
        };
        let mut chain = Vec::new();
        let mut cursor = Some(current);
        while let Some(id) = cursor {
            chain.push(id);
            if id == tree.root {
                break;
            }
            cursor = tree.node(id).and_then(|node| node.parent);
        }
        chain.reverse();
        if depth + 1 >= chain.len() {
            return false;
        }
        self.current = Some(chain[depth]);
        true
    }

    /// The path of node `id` while it is part of the tree.
    pub fn path_of(&self, id: i32) -> Option<PathBuf> {
        let tree = self.tree.as_ref()?;
        let id = NodeId(u32::try_from(id).ok()?);
        tree.is_live(id).then(|| tree.path_of(id))
    }

    /// The current folder: its crumbs, totals, rows biggest first with the
    /// remainder merged, and the treemap of those rows.
    pub fn project(&self) -> Projection {
        let mut projection = Projection {
            root_path: self.root.clone().unwrap_or_default(),
            ..Projection::default()
        };
        let (Some(tree), Some(current)) = (self.tree.as_ref(), self.current) else {
            return projection;
        };
        let Some(node) = tree.node(current) else {
            return projection;
        };

        let mut chain = Vec::new();
        let mut cursor = Some(current);
        while let Some(id) = cursor {
            if id == tree.root {
                chain.push(root_name(tree));
                break;
            }
            let Some(step) = tree.node(id) else {
                break;
            };
            chain.push(step.name.to_string_lossy().into_owned());
            cursor = step.parent;
        }
        chain.reverse();
        projection.crumbs = chain;
        projection.current_path = tree.path_of(current);

        let (folders, others) = descendants(tree, current);
        projection.total_bytes = node.allocated as f64;
        projection.files_below = node.files_below as f64;
        projection.folders_below = folders as f64;
        projection.unreadable_below = f64::from(
            self.unreadable
                .get(current.0 as usize)
                .copied()
                .unwrap_or(0),
        );
        projection.other_devices = others as f64;

        let rows = children_rows(tree, current, &self.unreadable, MAX_ROWS);
        for row in &rows {
            projection
                .row_ids
                .push(row.id.map_or(REMAINDER_ID, |id| f64::from(id.0)));
            projection
                .row_names
                .push(row.name.to_string_lossy().into_owned());
            projection.row_kinds.push(kind_token(row.kind).to_owned());
            projection.row_allocated.push(row.allocated as f64);
            projection.row_shares.push(row.share);
            projection.row_files.push(row.files_below as f64);
            projection
                .row_unreadable
                .push(f64::from(row.unreadable_below));
            projection.row_merged.push(row.merged as f64);
        }
        let ids: Vec<Option<NodeId>> = rows.iter().map(|row| row.id).collect();
        let sizes: Vec<u64> = rows.iter().map(|row| row.allocated).collect();
        let unit = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        projection.rects = flat_rects(&ids, &squarify(&sizes, unit));
        projection
    }
}

/// The scanned root's own name, or the whole path when it has none (`/`).
fn root_name(tree: &Tree) -> String {
    tree.path.file_name().map_or_else(
        || tree.path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Folders below `id` on the scanned device, and folders below it that
/// are another device's mount, in one iterative walk.
fn descendants(tree: &Tree, id: NodeId) -> (u64, u64) {
    let mut folders = 0u64;
    let mut others = 0u64;
    let mut stack: Vec<NodeId> = tree
        .node(id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    while let Some(next) = stack.pop() {
        let Some(node) = tree.node(next) else {
            continue;
        };
        if node.other_device {
            others += 1;
        } else if node.kind == Kind::Dir {
            folders += 1;
        }
        stack.extend(node.children.iter().copied());
    }
    (folders, others)
}

/// The kind token the page reads.
fn kind_token(kind: Kind) -> &'static str {
    match kind {
        Kind::Dir => "dir",
        Kind::File => "file",
        Kind::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::atomic::{AtomicU32, Ordering};

    use hematita_core::usage::walk::{scan, Progress};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A fresh empty folder under the system temporary directory.
    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "siderita-usage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create a temp dir");
        dir
    }

    /// `dir/{a/{x: 3000 bytes}, y: 1000 bytes}`.
    fn fixture() -> PathBuf {
        let dir = tempdir();
        std::fs::create_dir(dir.join("a")).expect("create a");
        std::fs::write(dir.join("a").join("x"), vec![7u8; 3000]).expect("write x");
        std::fs::write(dir.join("y"), vec![7u8; 1000]).expect("write y");
        dir
    }

    /// `dir/{f: 4096 bytes, g: hard link to f}`.
    fn hard_link_fixture() -> PathBuf {
        let dir = tempdir();
        std::fs::write(dir.join("f"), vec![7u8; 4096]).expect("write f");
        std::fs::hard_link(dir.join("f"), dir.join("g")).expect("link g");
        dir
    }

    fn scanned(dir: &Path) -> Tree {
        let mut progress = |_: Progress| {};
        scan(
            dir,
            &HashSet::new(),
            &CancellationToken::new(),
            &mut progress,
        )
        .expect("scan a temp dir")
    }

    fn name_of(dir: &Path) -> String {
        dir.file_name()
            .expect("a temp dir has a name")
            .to_string_lossy()
            .into_owned()
    }

    fn remove(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn begin_returns_a_token_and_a_later_begin_of_the_same_root_keeps_the_tree() {
        let dir = tempdir();
        let mut session = UsageSession::default();
        let (generation, _token) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(session.accept(generation, scanned(&dir)));
        assert!(session.begin(dir.clone(), "a").is_none());
        assert!(session.tree.is_some());
        remove(&dir);
    }

    #[test]
    fn a_stale_tree_is_refused() {
        let dir = tempdir();
        let other = tempdir();
        let mut session = UsageSession::default();
        let (old, _t) = session.begin(dir.clone(), "a").expect("first begin scans");
        let (_new, _t2) = session
            .begin(other.clone(), "a")
            .expect("another root scans");
        assert!(!session.accept(old, scanned(&dir)));
        assert!(session.tree.is_none());
        remove(&dir);
        remove(&other);
    }

    #[test]
    fn a_new_root_cancels_the_running_scan() {
        let dir = tempdir();
        let other = tempdir();
        let mut session = UsageSession::default();
        let (_old, token) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(
            session.begin(dir.clone(), "a").is_none(),
            "the same walk is kept"
        );
        assert!(!token.is_cancelled());
        let _ = session
            .begin(other.clone(), "a")
            .expect("another root scans");
        assert!(token.is_cancelled());
        remove(&dir);
        remove(&other);
    }

    #[test]
    fn enter_and_up_move_inside_the_tree_without_a_second_scan() {
        let dir = fixture();
        let mut session = UsageSession::default();
        let (g, _t) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(session.accept(g, scanned(&dir)));
        let projection = session.project();
        assert_eq!(projection.crumbs.len(), 1);
        let first = projection.row_ids[0] as i32; // biggest first: `a`
        assert_eq!(projection.row_kinds[0], "dir");
        assert!(session.enter(first));
        assert_eq!(
            session.project().crumbs,
            vec![name_of(&dir), "a".to_owned()]
        );
        assert_eq!(session.project().current_path, dir.join("a"));
        assert!(!session.enter(session.project().row_ids[0] as i32)); // `x` is a file
        assert!(session.up());
        assert!(!session.up()); // at the root
        remove(&dir);
    }

    #[test]
    fn close_forgets_everything() {
        let dir = tempdir();
        let mut session = UsageSession::default();
        let (g, token) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(session.accept(g, scanned(&dir)));
        session.close();
        assert!(session.tree.is_none() && session.root.is_none() && session.current.is_none());
        assert!(!session.accept(g, scanned(&dir)), "a late tree is stale");
        assert!(
            !token.is_cancelled(),
            "a landed scan has nothing left to stop"
        );
        remove(&dir);
    }

    #[test]
    fn only_the_last_owner_closes_the_session() {
        let dir = tempdir();
        let other = tempdir();
        let mut session = UsageSession::default();
        let (_a, _t) = session
            .begin(dir.clone(), "quicklook")
            .expect("first begin scans");
        let (b, _t2) = session
            .begin(other.clone(), "properties")
            .expect("B takes over");
        assert!(session.accept(b, scanned(&other)));
        assert!(
            !session.close_by("quicklook"),
            "A no longer owns the session"
        );
        assert!(session.tree.is_some());
        assert_eq!(session.root.as_deref(), Some(other.as_path()));
        assert!(session.close_by("properties"));
        assert!(session.tree.is_none());
        remove(&dir);
        remove(&other);
    }

    #[test]
    fn up_to_jumps_to_a_crumb_in_one_step() {
        let dir = tempdir();
        std::fs::create_dir_all(dir.join("a").join("b")).expect("create a/b");
        std::fs::write(dir.join("a").join("b").join("f"), vec![7u8; 100]).expect("write f");
        let mut session = UsageSession::default();
        let (g, _t) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(session.accept(g, scanned(&dir)));
        let a = session.project().row_ids[0] as i32;
        assert!(session.enter(a));
        let b = session.project().row_ids[0] as i32;
        assert!(session.enter(b));
        assert_eq!(session.project().crumbs.len(), 3);
        assert!(!session.up_to(2), "the current folder is not a jump");
        assert!(!session.up_to(7));
        assert!(session.up_to(0));
        assert_eq!(session.project().crumbs, vec![name_of(&dir)]);
        remove(&dir);
    }

    #[test]
    fn totals_count_a_hard_link_once() {
        let dir = hard_link_fixture();
        let mut session = UsageSession::default();
        let (g, _t) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(session.accept(g, scanned(&dir)));
        assert_eq!(session.project().files_below, 1.0);
        remove(&dir);
    }

    #[test]
    fn rows_tiles_and_paths_agree() {
        let dir = fixture();
        let mut session = UsageSession::default();
        let (g, _t) = session.begin(dir.clone(), "a").expect("first begin scans");
        assert!(session.accept(g, scanned(&dir)));
        let projection = session.project();
        assert_eq!(projection.row_names.len(), 2);
        assert_eq!(projection.folders_below, 1.0);
        assert_eq!(projection.files_below, 2.0);
        assert_eq!(projection.rects.len() % 5, 0);
        let first = projection.row_ids[0] as i32;
        assert_eq!(session.path_of(first), Some(dir.join("a")));
        assert_eq!(session.path_of(-1), None);
        assert_eq!(session.path_of(9999), None);
        remove(&dir);
    }
}
