//! The storage domain against a real directory tree.
//!
//! Every test builds its own fixture under the system temporary directory
//! with a unique name, so the tests run in parallel without sharing state.
//! The fixture's `locked/` directory is made unreadable (mode `000`); the
//! [`Fixture`] guard restores `755` before removing everything, so a failing
//! test never leaves a directory `cargo test` cannot delete. Expected sizes
//! are read back with `symlink_metadata` rather than hard-coded, because block
//! counts differ between filesystems.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::CancellationToken;
use hematita_core::usage::duplicates::{candidates, confirm, ConfirmError, Group};
use hematita_core::usage::empty::empty_folders;
use hematita_core::usage::remove::{
    check_identity, delete_tree, Failure, Refusal, RemoveError, Removed,
};
use hematita_core::usage::tree::{Kind, Node, NodeId, Tree};
use hematita_core::usage::walk::{scan, scan_subtree, Progress, ScanError};

const MIB: usize = 1024 * 1024;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.root.join("locked"), fs::Permissions::from_mode(0o755));
        remove_all(&self.root);
    }
}

/// Test-only recursive removal that never follows a symbolic link.
fn remove_all(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.is_dir() {
        if let Ok(read) = fs::read_dir(path) {
            for entry in read.flatten() {
                remove_all(&entry.path());
            }
        }
        let _ = fs::remove_dir(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

fn unique_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "hematita-usage-{}-{}-{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// ```text
/// root/
///   big.bin          4 MiB of 0xAB
///   copy-of-big.bin  identical bytes
///   same-size.bin    4 MiB of 0xCD
///   docs/a.txt       3 KiB
///   docs/b.txt       3 KiB, identical to a.txt
///   docs/link-to-a.txt  hard link to a.txt
///   empty/deeper/    nothing at any depth
///   out.lnk -> /tmp  symbolic link
///   locked/secret.txt  locked/ is mode 000
///   many/0..599      only when `padded`
/// ```
fn fixture(padded: bool) -> Fixture {
    let root = unique_dir();
    let fixture = Fixture { root: root.clone() };
    fs::create_dir_all(root.join("docs")).expect("docs");
    fs::create_dir_all(root.join("empty/deeper")).expect("empty");
    fs::create_dir_all(root.join("locked")).expect("locked");
    fs::write(root.join("big.bin"), vec![0xAB; 4 * MIB]).expect("big");
    fs::write(root.join("copy-of-big.bin"), vec![0xAB; 4 * MIB]).expect("copy");
    fs::write(root.join("same-size.bin"), vec![0xCD; 4 * MIB]).expect("same size");
    fs::write(root.join("docs/a.txt"), vec![b'a'; 3 * 1024]).expect("a");
    fs::write(root.join("docs/b.txt"), vec![b'a'; 3 * 1024]).expect("b");
    fs::hard_link(root.join("docs/a.txt"), root.join("docs/link-to-a.txt")).expect("hard link");
    std::os::unix::fs::symlink("/tmp", root.join("out.lnk")).expect("symlink");
    fs::write(root.join("locked/secret.txt"), b"secret").expect("secret");
    if padded {
        fs::create_dir_all(root.join("many")).expect("many");
        for i in 0..600 {
            fs::write(root.join(format!("many/{i}")), b"x").expect("small file");
        }
    }
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).expect("chmod");
    fixture
}

/// Root reads through a mode-000 directory, so the unreadable assertions only
/// hold for an ordinary user.
fn running_as_root() -> bool {
    fs::metadata("/proc/self")
        .map(|m| m.uid() == 0)
        .unwrap_or(false)
}

fn allocated(path: &Path) -> u64 {
    fs::symlink_metadata(path).expect("metadata").blocks() * 512
}

fn no_bounds() -> HashSet<PathBuf> {
    HashSet::new()
}

fn scan_ok(root: &Path) -> Tree {
    scan(root, &no_bounds(), &CancellationToken::new(), &mut |_| {}).expect("scan")
}

fn child(tree: &Tree, parent: NodeId, name: &str) -> NodeId {
    tree.node(parent)
        .expect("parent")
        .children
        .iter()
        .copied()
        .find(|id| tree.node(*id).is_some_and(|n| n.name == name))
        .unwrap_or_else(|| panic!("no child {name}"))
}

#[test]
fn the_walk_counts_files_once_and_follows_no_link() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let root = tree.node(tree.root).expect("root");

    // big, copy, same-size, a/link (one inode), b; secret is behind locked/.
    assert_eq!(root.files_below, 5);
    let expected = allocated(&fx.path("big.bin"))
        + allocated(&fx.path("copy-of-big.bin"))
        + allocated(&fx.path("same-size.bin"))
        + allocated(&fx.path("docs/a.txt"))
        + allocated(&fx.path("docs/b.txt"))
        + allocated(&fx.path("out.lnk"));
    assert_eq!(root.allocated, expected);

    let docs = tree.node(child(&tree, tree.root, "docs")).expect("docs");
    assert_eq!(docs.files_below, 2);
    assert_eq!(
        docs.allocated,
        allocated(&fx.path("docs/a.txt")) + allocated(&fx.path("docs/b.txt"))
    );

    let link = tree.node(child(&tree, tree.root, "out.lnk")).expect("link");
    assert_eq!(link.kind, Kind::Other);
    assert_eq!(link.apparent, "/tmp".len() as u64);
    assert!(link.children.is_empty());

    let empty = child(&tree, tree.root, "empty");
    let deeper = tree.node(child(&tree, empty, "deeper")).expect("deeper");
    assert_eq!((deeper.files_below, deeper.allocated), (0, 0));
    assert_eq!(
        tree.path_of(child(&tree, empty, "deeper")),
        fx.path("empty/deeper")
    );
}

#[test]
fn an_unreadable_directory_is_marked_and_the_walk_goes_on() {
    if running_as_root() {
        eprintln!("skipped: root reads through a mode-000 directory");
        return;
    }
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let locked = tree
        .node(child(&tree, tree.root, "locked"))
        .expect("locked");
    assert!(locked.unreadable);
    assert!(locked.children.is_empty());
    assert_eq!(tree.unreadable_dirs, 1);
    assert_eq!(tree.node(tree.root).expect("root").files_below, 5);
}

#[test]
fn progress_is_reported_on_a_large_folder() {
    let fx = fixture(true);
    let mut reports: Vec<Progress> = Vec::new();
    let tree = scan(
        &fx.root,
        &no_bounds(),
        &CancellationToken::new(),
        &mut |p| reports.push(p),
    )
    .expect("scan");
    assert!(!reports.is_empty());
    assert!(reports.windows(2).all(|w| w[0].files <= w[1].files));
    assert_eq!(tree.node(tree.root).expect("root").files_below, 605);
}

#[test]
fn a_cancelled_token_stops_the_walk() {
    let fx = fixture(false);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        scan(&fx.root, &no_bounds(), &cancel, &mut |_| {}),
        Err(ScanError::Cancelled)
    ));
}

#[test]
fn a_root_that_is_not_a_folder_is_refused() {
    let fx = fixture(false);
    let result = scan(
        &fx.path("big.bin"),
        &no_bounds(),
        &CancellationToken::new(),
        &mut |_| {},
    );
    assert!(matches!(result, Err(ScanError::NotADirectory { path }) if path == fx.path("big.bin")));
    let missing = scan(
        &fx.path("nope"),
        &no_bounds(),
        &CancellationToken::new(),
        &mut |_| {},
    );
    assert!(matches!(missing, Err(ScanError::Root { .. })));
    let link = scan(
        &fx.path("out.lnk"),
        &no_bounds(),
        &CancellationToken::new(),
        &mut |_| {},
    );
    assert!(matches!(link, Err(ScanError::NotADirectory { .. })));
}

fn name(tree: &Tree, id: NodeId) -> String {
    tree.node(id)
        .expect("node")
        .name
        .to_string_lossy()
        .into_owned()
}

fn names(tree: &Tree, ids: &[NodeId]) -> Vec<String> {
    ids.iter().map(|id| name(tree, *id)).collect()
}

#[test]
fn empty_folders_are_listed_deepest_first() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let empty = child(&tree, tree.root, "empty");
    let deeper = child(&tree, empty, "deeper");
    // locked/ holds nothing the walk could see, but it is unreadable, so it
    // is not called empty.
    assert_eq!(empty_folders(&tree), vec![deeper, empty]);
}

#[test]
fn candidates_group_files_of_equal_allocation() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let groups = candidates(&tree);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].size, allocated(&fx.path("big.bin")));
    let mut trio = names(&tree, &groups[0].nodes);
    trio.sort();
    assert_eq!(trio, ["big.bin", "copy-of-big.bin", "same-size.bin"]);
    assert!(groups[0].nodes.windows(2).all(|w| w[0].0 < w[1].0));
    // The hard link is one file: whichever name the walk met first carries
    // the size, and b.txt pairs with it.
    let pair = names(&tree, &groups[1].nodes);
    assert_eq!(pair.len(), 2);
    assert!(pair.contains(&"b.txt".to_string()));
    assert!(pair.contains(&"a.txt".to_string()) || pair.contains(&"link-to-a.txt".to_string()));
}

#[test]
fn confirm_keeps_only_identical_content_together() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let groups = candidates(&tree);
    let mut read = 0;
    let trio = confirm(&tree, &groups[0], &CancellationToken::new(), &mut |b| {
        read = b
    })
    .expect("confirm trio");
    assert_eq!(trio.len(), 1);
    let mut verified = names(&tree, &trio[0].nodes);
    verified.sort();
    assert_eq!(verified, ["big.bin", "copy-of-big.bin"]);
    assert_eq!(trio[0].size, groups[0].size);
    assert!(read > 0);

    let pair =
        confirm(&tree, &groups[1], &CancellationToken::new(), &mut |_| {}).expect("confirm pair");
    assert_eq!(pair.len(), 1);
    assert_eq!(pair[0].nodes, groups[1].nodes);
}

#[test]
fn confirm_stops_on_cancel_and_names_an_unreadable_file() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let groups = candidates(&tree);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        confirm(&tree, &groups[0], &cancel, &mut |_| {}),
        Err(ConfirmError::Cancelled)
    ));

    fs::remove_file(fx.path("copy-of-big.bin")).expect("remove copy");
    let result = confirm(&tree, &groups[0], &CancellationToken::new(), &mut |_| {});
    assert!(
        matches!(&result, Err(ConfirmError::Read { path, .. }) if *path == fx.path("copy-of-big.bin")),
        "{result:?}"
    );
    let lone = Group {
        size: 1,
        nodes: vec![groups[0].nodes[0]],
    };
    assert!(
        confirm(&tree, &lone, &CancellationToken::new(), &mut |_| {})
            .expect("one file")
            .is_empty()
    );
}

#[test]
fn a_scanned_node_carries_its_device_and_inode_and_a_replacement_is_refused() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let big = child(&tree, tree.root, "big.bin");
    let path = tree.path_of(big);
    let meta = fs::symlink_metadata(&path).expect("metadata");
    let node = tree.node(big).expect("node");
    assert_eq!((node.dev, node.ino), (meta.dev(), meta.ino()));
    let root = tree.node(tree.root).expect("root");
    let root_meta = fs::symlink_metadata(&fx.root).expect("root metadata");
    assert_eq!((root.dev, root.ino), (root_meta.dev(), root_meta.ino()));
    assert!(check_identity(&path, node.dev, node.ino).is_ok());

    // Replace the file with another of the same name.
    let spare = fx.root.join("spare.bin");
    fs::write(&spare, b"other").expect("spare");
    fs::rename(&spare, &path).expect("replace");
    assert_eq!(
        refused_error(check_identity(&path, node.dev, node.ino).map(|()| unreachable_removed())),
        Refusal::Changed
    );
    fs::remove_file(&path).expect("remove");
    assert_eq!(
        refused_error(check_identity(&path, node.dev, node.ino).map(|()| unreachable_removed())),
        Refusal::Missing
    );
}

fn unreachable_removed() -> Removed {
    Removed::default()
}

fn refused_error(result: Result<Removed, RemoveError>) -> Refusal {
    match result {
        Err(RemoveError::Refused { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A refusal comes before anything is touched, so it removed nothing.
fn refused(result: Result<Removed, Failure>) -> Refusal {
    match result {
        Err(Failure { error, removed }) => {
            assert_eq!(removed, Removed::default(), "a refusal removed something");
            refused_error(Err(error))
        }
        Ok(removed) => panic!("expected a refusal, removed {removed:?}"),
    }
}

#[test]
fn delete_tree_refuses_the_root_the_outside_and_the_missing() {
    let fx = fixture(false);
    let token = CancellationToken::new();
    assert_eq!(
        refused(delete_tree(&fx.root, &fx.root, &no_bounds(), None, &token)),
        Refusal::IsRoot
    );
    assert_eq!(
        refused(delete_tree(
            &fx.root.join("docs/.."),
            &fx.root,
            &no_bounds(),
            None,
            &token
        )),
        Refusal::IsRoot
    );
    let sibling = fx.root.join("../sibling");
    assert_eq!(
        refused(delete_tree(&sibling, &fx.root, &no_bounds(), None, &token)),
        Refusal::Outside
    );
    assert_eq!(
        refused(delete_tree(
            &fx.path("docs/../../x"),
            &fx.root,
            &no_bounds(),
            None,
            &token
        )),
        Refusal::Outside
    );
    assert_eq!(
        refused(delete_tree(
            Path::new("/tmp"),
            &fx.root,
            &no_bounds(),
            None,
            &token
        )),
        Refusal::Outside
    );
    assert_eq!(
        refused(delete_tree(
            &fx.path("nope"),
            &fx.root,
            &no_bounds(),
            None,
            &token
        )),
        Refusal::Missing
    );
    assert!(fx.path("docs/a.txt").exists());
}

#[test]
fn delete_tree_refuses_a_mount_root() {
    // /proc is its own filesystem; / is the scanned "root" here only for the
    // refusal, which happens before anything is touched.
    let token = CancellationToken::new();
    assert_eq!(
        refused(delete_tree(
            Path::new("/proc"),
            Path::new("/"),
            &no_bounds(),
            None,
            &token
        )),
        Refusal::MountRoot
    );
}

#[test]
fn delete_tree_removes_a_link_and_never_its_target() {
    let fx = fixture(false);
    let target = unique_dir();
    fs::create_dir_all(&target).expect("target");
    fs::write(target.join("keep.txt"), b"keep").expect("keep");
    let guard = Fixture {
        root: target.clone(),
    };
    std::os::unix::fs::symlink(&target, fx.path("docs/outside.lnk")).expect("link");

    let removed = delete_tree(
        &fx.path("docs/outside.lnk"),
        &fx.root,
        &no_bounds(),
        None,
        &CancellationToken::new(),
    )
    .expect("delete link");
    assert_eq!(removed.entries, 1);
    assert!(fs::symlink_metadata(fx.path("docs/outside.lnk")).is_err());
    assert!(target.join("keep.txt").exists());

    // A folder holding a link to the outside: the link goes, the target stays.
    std::os::unix::fs::symlink(&target, fx.path("empty/deeper/out.lnk")).expect("link");
    delete_tree(
        &fx.path("empty"),
        &fx.root,
        &no_bounds(),
        None,
        &CancellationToken::new(),
    )
    .expect("delete empty");
    assert!(target.join("keep.txt").exists());
    drop(guard);
}

#[test]
fn delete_tree_removes_a_nested_tree_and_reports_it() {
    let fx = fixture(false);
    let expected_bytes = allocated(&fx.path("docs/a.txt"))
        + allocated(&fx.path("docs/b.txt"))
        + allocated(&fx.path("docs"));
    let removed = delete_tree(
        &fx.path("docs"),
        &fx.root,
        &no_bounds(),
        None,
        &CancellationToken::new(),
    )
    .expect("delete docs");
    // docs/, a.txt, b.txt, link-to-a.txt; the hard link's blocks count once.
    assert_eq!(removed.entries, 4);
    assert_eq!(removed.bytes, expected_bytes);
    assert!(!fx.path("docs").exists());
    assert!(fx.path("big.bin").exists());
}

#[test]
fn delete_tree_touches_nothing_once_cancelled() {
    let fx = fixture(false);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        delete_tree(&fx.path("docs"), &fx.root, &no_bounds(), None, &cancel),
        Err(Failure {
            error: RemoveError::Cancelled,
            removed: Removed {
                entries: 0,
                bytes: 0
            },
        })
    ));
    assert!(fx.path("docs/a.txt").exists());
    assert!(fx.path("docs/b.txt").exists());
}

#[test]
fn a_folder_holding_only_a_link_is_not_empty() {
    let fx = fixture(false);
    fs::create_dir_all(fx.path("linkonly")).expect("linkonly");
    std::os::unix::fs::symlink("/tmp", fx.path("linkonly/pointer")).expect("link");
    let tree = scan_ok(&fx.root);
    let linkonly = child(&tree, tree.root, "linkonly");
    assert_eq!(tree.node(linkonly).expect("linkonly").others_below, 1);
    assert!(!empty_folders(&tree).contains(&linkonly));
    // out.lnk and pointer
    assert_eq!(tree.node(tree.root).expect("root").others_below, 2);
}

#[test]
fn a_mount_boundary_is_a_leaf_even_on_the_same_device() {
    let fx = fixture(false);
    let bounds: HashSet<PathBuf> = [fx.path("docs")].into_iter().collect();
    let tree = scan(&fx.root, &bounds, &CancellationToken::new(), &mut |_| {}).expect("scan");
    let docs = tree.node(child(&tree, tree.root, "docs")).expect("docs");
    assert!(docs.other_device);
    assert!(docs.children.is_empty());
    assert_eq!(tree.node(tree.root).expect("root").files_below, 3);
    // The scanned root itself being a mount target is no boundary.
    let own: HashSet<PathBuf> = [fx.root.clone()].into_iter().collect();
    let whole = scan(&fx.root, &own, &CancellationToken::new(), &mut |_| {}).expect("scan");
    assert_eq!(whole.node(whole.root).expect("root").files_below, 5);
}

#[test]
fn delete_tree_refuses_a_listed_mount_at_or_below_the_target() {
    let fx = fixture(false);
    let token = CancellationToken::new();
    let bounds: HashSet<PathBuf> = [fx.path("empty/deeper")].into_iter().collect();
    assert_eq!(
        refused(delete_tree(
            &fx.path("empty/deeper"),
            &fx.root,
            &bounds,
            None,
            &token
        )),
        Refusal::MountRoot
    );
    assert_eq!(
        refused(delete_tree(
            &fx.path("empty"),
            &fx.root,
            &bounds,
            None,
            &token
        )),
        Refusal::MountRoot
    );
    assert!(fx.path("empty/deeper").is_dir());
}

#[test]
fn delete_tree_refuses_a_folder_swapped_for_a_link_after_the_scan() {
    let fx = fixture(false);
    let _tree = scan_ok(&fx.root);
    let outside = unique_dir();
    fs::create_dir_all(outside.join("sub")).expect("outside");
    fs::write(outside.join("sub/keep.txt"), b"keep").expect("keep");
    let guard = Fixture {
        root: outside.clone(),
    };
    // docs/ becomes a link to the outside between the scan and the deletion.
    fs::rename(fx.path("docs"), fx.path("docs-moved")).expect("move docs");
    std::os::unix::fs::symlink(&outside, fx.path("docs")).expect("swap");

    let result = delete_tree(
        &fx.path("docs/sub"),
        &fx.root,
        &no_bounds(),
        None,
        &CancellationToken::new(),
    );
    assert_eq!(
        refused(result),
        Refusal::Symlink {
            at: fx.path("docs")
        }
    );
    assert!(outside.join("sub/keep.txt").exists());
    // The link itself, as the target, is still removable as a link.
    delete_tree(
        &fx.path("docs"),
        &fx.root,
        &no_bounds(),
        None,
        &CancellationToken::new(),
    )
    .expect("link");
    assert!(outside.join("sub/keep.txt").exists());
    drop(guard);
}

#[test]
fn delete_tree_refuses_an_entry_whose_identity_changed() {
    let fx = fixture(false);
    let tree = scan_ok(&fx.root);
    let docs = tree.node(child(&tree, tree.root, "docs")).expect("docs");
    let token = CancellationToken::new();
    assert_eq!(
        refused(delete_tree(
            &fx.path("docs"),
            &fx.root,
            &no_bounds(),
            Some((docs.dev, docs.ino.wrapping_add(1))),
            &token
        )),
        Refusal::Changed
    );
    assert!(fx.path("docs/a.txt").exists());
    let removed = delete_tree(
        &fx.path("docs"),
        &fx.root,
        &no_bounds(),
        Some((docs.dev, docs.ino)),
        &token,
    )
    .expect("the scanned identity is accepted");
    assert_eq!(removed.entries, 4);
}

/// Restores a directory's mode on drop, so a failing test never leaves a
/// directory the fixture's own cleanup cannot enter.
struct ModeGuard {
    path: PathBuf,
}

impl Drop for ModeGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn a_deletion_stopped_by_an_unreadable_folder_reports_what_it_removed() {
    if running_as_root() {
        eprintln!("skipped: root reads through a mode-000 directory");
        return;
    }
    let fx = fixture(false);
    let inner = fx.path("docs/inner");
    fs::create_dir_all(&inner).expect("inner");
    fs::write(inner.join("kept.txt"), b"kept").expect("kept");
    let guard = ModeGuard {
        path: inner.clone(),
    };
    fs::set_permissions(&inner, fs::Permissions::from_mode(0o000)).expect("chmod");
    let files = allocated(&fx.path("docs/a.txt")) + allocated(&fx.path("docs/b.txt"));

    let result = delete_tree(
        &fx.path("docs"),
        &fx.root,
        &no_bounds(),
        None,
        &CancellationToken::new(),
    );
    // A directory's files go before its subdirectories: a.txt, b.txt and the
    // hard link are gone when inner/ refuses to open.
    match result {
        Err(Failure {
            error: RemoveError::Io { path, .. },
            removed,
        }) => {
            assert_eq!(path, inner);
            assert_eq!(removed.entries, 3);
            assert_eq!(removed.bytes, files);
        }
        other => panic!("expected an IO failure with a partial result, got {other:?}"),
    }
    assert!(!fx.path("docs/a.txt").exists());
    assert!(fs::symlink_metadata(&inner).is_ok());
    drop(guard);
    assert!(inner.join("kept.txt").exists());
}

#[test]
fn scan_subtree_equals_scan_on_the_same_folder() {
    let fx = fixture(true);
    let full = scan_ok(&fx.path("many"));
    let sub = scan_subtree(&fx.path("many"), &no_bounds(), &CancellationToken::new()).expect("sub");
    assert_eq!(sub.nodes.len(), full.nodes.len());
    assert_eq!(sub.path, full.path);
    let (a, b) = (
        sub.node(sub.root).expect("root"),
        full.node(full.root).expect("root"),
    );
    assert_eq!(
        (a.allocated, a.apparent, a.files_below),
        (b.allocated, b.apparent, b.files_below)
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        scan_subtree(&fx.root, &no_bounds(), &cancel),
        Err(ScanError::Cancelled)
    ));
}

#[test]
fn only_a_second_name_of_a_linked_file_is_counted_as_a_hard_link() {
    let fx = fixture(false);
    assert_eq!(scan_ok(&fx.root).hard_link_names, 1);
    fs::remove_file(fx.path("docs/link-to-a.txt")).expect("unlink");
    let tree = scan_ok(&fx.root);
    assert_eq!(tree.hard_link_names, 0);
    assert_eq!(tree.node(tree.root).expect("root").files_below, 5);
}

fn hand_node(name: &str, kind: Kind, parent: Option<u32>, allocated: u64, files: u64) -> Node {
    Node {
        name: name.into(),
        kind,
        parent: parent.map(NodeId),
        allocated,
        apparent: allocated,
        files_below: files,
        others_below: 0,
        unreadable: false,
        other_device: false,
        dev: 1,
        ino: 0,
        children: Vec::new(),
    }
}

fn hand_tree(path: &str, nodes: Vec<(Node, Vec<u32>)>) -> Tree {
    Tree {
        root: NodeId(0),
        path: PathBuf::from(path),
        device: 1,
        nodes: nodes
            .into_iter()
            .map(|(mut node, children)| {
                node.children = children.into_iter().map(NodeId).collect();
                node
            })
            .collect(),
        unreadable_dirs: 0,
        hard_link_names: 0,
    }
}

#[test]
fn a_graft_replaces_a_subtree_and_updates_its_ancestors() {
    // root(100) ─ { top(90) ─ { old(80) ─ { x(50), y(30) }, keep(10) }, side(0) }
    let mut tree = hand_tree(
        "/scan",
        vec![
            (hand_node("scan", Kind::Dir, None, 100, 3), vec![1, 6]),
            (hand_node("top", Kind::Dir, Some(0), 90, 3), vec![2, 5]),
            (hand_node("old", Kind::Dir, Some(1), 80, 2), vec![3, 4]),
            (hand_node("x", Kind::File, Some(2), 50, 1), vec![]),
            (hand_node("y", Kind::File, Some(2), 30, 1), vec![]),
            (hand_node("keep", Kind::File, Some(1), 10, 1), vec![]),
            (hand_node("side", Kind::Dir, Some(0), 0, 0), vec![]),
        ],
    );
    // What is left of old/ after a stopped deletion: itself and y.
    let fresh = hand_tree(
        "/scan/top/old",
        vec![
            (hand_node("elsewhere", Kind::Dir, None, 30, 1), vec![1]),
            (hand_node("y", Kind::File, Some(0), 30, 1), vec![]),
        ],
    );
    assert!(tree.graft(NodeId(2), fresh));

    for id in [0, 1] {
        let n = tree.node(NodeId(id)).expect("ancestor");
        assert_eq!(n.files_below, 2, "{id}");
    }
    assert_eq!(tree.node(NodeId(0)).expect("root").allocated, 50);
    assert_eq!(tree.node(NodeId(1)).expect("top").allocated, 40);
    // Untouched ids keep their meaning.
    assert_eq!(name(&tree, NodeId(5)), "keep");
    assert_eq!(name(&tree, NodeId(6)), "side");
    for id in [0, 1, 5, 6] {
        assert!(tree.is_live(NodeId(id)), "{id}");
    }
    for id in [2, 3, 4] {
        assert!(!tree.is_live(NodeId(id)), "{id}");
    }
    // The graft keeps the old name and place, children first in order.
    let grafted = child(&tree, NodeId(1), "old");
    assert!(tree.is_live(grafted));
    assert_eq!(tree.node(NodeId(1)).expect("top").children[0], grafted);
    let y = child(&tree, grafted, "y");
    assert_eq!(tree.path_of(y), PathBuf::from("/scan/top/old/y"));
    assert_eq!(tree.node(y).expect("y").allocated, 30);

    // A dead id and an unknown one are refused.
    let empty = || hand_tree("/x", vec![(hand_node("x", Kind::Dir, None, 0, 0), vec![])]);
    assert!(!tree.graft(NodeId(3), empty()));
    assert!(!tree.graft(NodeId(99), empty()));
    assert!(!tree.is_live(NodeId(99)));
    assert_eq!(tree.node(NodeId(0)).expect("root").allocated, 50);
}
