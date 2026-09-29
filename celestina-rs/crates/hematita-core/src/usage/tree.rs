//! The scanned tree: an indexed arena of nodes whose sizes are aggregates of
//! their subtree.
//!
//! Nodes live in one `Vec` addressed by [`NodeId`], so an id handed to the
//! interface stays valid for the life of the scan, including after
//! [`Tree::prune_many`] (a pruned node is emptied and unlinked, never moved) and
//! [`Tree::graft`] (a replaced subtree is unlinked and the fresh one
//! appended). [`Tree::is_live`] tells an id still in the tree from one of a
//! subtree that was pruned or replaced.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    Other,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub name: OsString,
    pub kind: Kind,
    pub parent: Option<NodeId>,
    /// Bytes on disk (`st_blocks × 512`) of this node and everything below.
    pub allocated: u64,
    /// Bytes as `st_size` reports them, of this node and everything below.
    pub apparent: u64,
    /// Regular files counted at or below this node (a hard link once).
    pub files_below: u64,
    /// Entries that are neither a file nor a directory (symbolic links,
    /// sockets, FIFOs, devices) at or below this node. A folder holding one
    /// is not empty, even with no file below it.
    pub others_below: u64,
    /// The directory could not be listed; what it holds is not in the tree.
    pub unreadable: bool,
    /// A directory on another mount than the scanned root (another device,
    /// another mount id, or a mount table target): listed as a leaf with no
    /// size, because a mount is analysed from its own root.
    pub other_device: bool,
    /// The device and inode the walk read for the entry, without following
    /// it, so an action can tell the same entry from a replacement.
    pub dev: u64,
    pub ino: u64,
    pub children: Vec<NodeId>,
}

#[derive(Clone, Debug)]
pub struct Tree {
    pub root: NodeId,
    /// The scanned folder, every link on the way to it resolved: the path
    /// the mount table and every path below it agree on.
    pub path: PathBuf,
    pub device: u64,
    pub nodes: Vec<Node>,
    pub unreadable_dirs: u32,
    /// Second and later names of a hard-linked file met by the walk that
    /// built the tree; a graft does not change it.
    pub hard_link_names: u64,
}

impl Tree {
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0 as usize)
    }

    /// The root path joined with every ancestor's name down to `id`.
    #[must_use]
    pub fn path_of(&self, id: NodeId) -> PathBuf {
        let mut names = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            if current == self.root {
                break;
            }
            let Some(node) = self.node(current) else {
                break;
            };
            names.push(node.name.as_os_str());
            cursor = node.parent;
        }
        let mut path = self.path.clone();
        for name in names.into_iter().rev() {
            path.push(name);
        }
        path
    }

    /// `id`'s children, biggest allocation first, ties by name.
    #[must_use]
    pub fn children_by_size(&self, id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let mut children = node.children.clone();
        children.sort_by(|a, b| match (self.node(*a), self.node(*b)) {
            (Some(x), Some(y)) => y
                .allocated
                .cmp(&x.allocated)
                .then_with(|| x.name.cmp(&y.name)),
            _ => a.0.cmp(&b.0),
        });
        children
    }

    /// [`Tree::prune_many`] for one id: `None` for the root or an unknown
    /// id, otherwise the allocated bytes removed (zero for an id no longer
    /// in the tree).
    pub fn prune(&mut self, id: NodeId) -> Option<u64> {
        if id == self.root || self.node(id).is_none() {
            return None;
        }
        Some(self.prune_many(&[id]))
    }

    /// Removes every subtree in `ids` from the aggregates of every ancestor
    /// and unlinks it from its parent; answers the allocated bytes removed.
    ///
    /// The root, unknown ids, repeated ids, ids no longer in the tree and ids
    /// below another one of `ids` are skipped — the one above takes them
    /// out — so nothing is subtracted twice. Each parent's children are
    /// filtered once against a set, so pruning k of n siblings costs
    /// O(n + k·depth) instead of O(k·n).
    pub fn prune_many(&mut self, ids: &[NodeId]) -> u64 {
        let chosen: HashSet<NodeId> = ids
            .iter()
            .copied()
            .filter(|id| *id != self.root && self.node(*id).is_some())
            .collect();
        let mut by_parent: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
        for id in &chosen {
            if self.has_ancestor_in(*id, &chosen) {
                continue;
            }
            if let Some(parent) = self.node(*id).and_then(|node| node.parent) {
                by_parent.entry(parent).or_default().insert(*id);
            }
        }
        let mut freed = 0u64;
        for (parent, members) in by_parent {
            if !self.is_live(parent) {
                continue;
            }
            let Some(holder) = self.nodes.get_mut(parent.0 as usize) else {
                continue;
            };
            let mut unlinked = Vec::new();
            holder.children.retain(|child| {
                let gone = members.contains(child);
                if gone {
                    unlinked.push(*child);
                }
                !gone
            });
            for id in unlinked {
                freed = freed.saturating_add(self.detach(id));
            }
        }
        freed
    }

    /// Whether an ancestor of `id` is in `set`.
    fn has_ancestor_in(&self, id: NodeId, set: &HashSet<NodeId>) -> bool {
        let mut cursor = self.node(id).and_then(|node| node.parent);
        // A parent chain longer than the arena would be a cycle.
        for _ in 0..=self.nodes.len() {
            let Some(current) = cursor else {
                return false;
            };
            if set.contains(&current) {
                return true;
            }
            cursor = self.node(current).and_then(|node| node.parent);
        }
        false
    }

    /// Empties `id`, already unlinked from its parent, and subtracts what it
    /// held from every ancestor; answers its allocated bytes.
    fn detach(&mut self, id: NodeId) -> u64 {
        let Some(node) = self.nodes.get_mut(id.0 as usize) else {
            return 0;
        };
        let (allocated, apparent, files, others) = (
            node.allocated,
            node.apparent,
            node.files_below,
            node.others_below,
        );
        let parent = node.parent;
        node.allocated = 0;
        node.apparent = 0;
        node.files_below = 0;
        node.others_below = 0;
        node.children.clear();
        let mut cursor = parent;
        while let Some(ancestor) = cursor.and_then(|a| self.nodes.get_mut(a.0 as usize)) {
            ancestor.allocated = ancestor.allocated.saturating_sub(allocated);
            ancestor.apparent = ancestor.apparent.saturating_sub(apparent);
            ancestor.files_below = ancestor.files_below.saturating_sub(files);
            ancestor.others_below = ancestor.others_below.saturating_sub(others);
            cursor = ancestor.parent;
        }
        allocated
    }

    /// Whether `id` is reachable from the root through `children`: false
    /// for an unknown id and for every node of a pruned or replaced subtree.
    #[must_use]
    pub fn is_live(&self, id: NodeId) -> bool {
        let mut cursor = id;
        // A parent chain longer than the arena would be a cycle.
        for _ in 0..=self.nodes.len() {
            if cursor == self.root {
                return self.node(cursor).is_some();
            }
            let Some(parent) = self.node(cursor).and_then(|node| node.parent) else {
                return false;
            };
            let Some(holder) = self.node(parent) else {
                return false;
            };
            if !holder.children.contains(&cursor) {
                return false;
            }
            cursor = parent;
        }
        false
    }

    /// Replaces the subtree at `id` with `fresh` (a scan of the same path):
    /// the old root is unlinked from its parent and zeroed like
    /// [`Tree::prune_many`]'s, the nodes below it stay in the arena unreachable
    /// (and [`Tree::is_live`] false), the fresh ones are appended with new ids, the fresh root taking `id`'s name
    /// and place among its siblings, and every ancestor's totals moved by
    /// the difference. Ids outside the subtree keep their meaning.
    ///
    /// A hard-linked file is counted once inside `fresh`, as the sub-scan
    /// saw it; a name of it elsewhere in the tree is not known there.
    ///
    /// Answers false, changing nothing, for the root, an id that is not
    /// live, an empty `fresh`, or one the arena could not address.
    pub fn graft(&mut self, id: NodeId, fresh: Tree) -> bool {
        if id == self.root || !self.is_live(id) {
            return false;
        }
        let Some(parent) = self.node(id).and_then(|node| node.parent) else {
            return false;
        };
        let Ok(offset) = u32::try_from(self.nodes.len()) else {
            return false;
        };
        let fresh_count = fresh.nodes.len();
        if fresh.node(fresh.root).is_none()
            || u32::try_from(self.nodes.len().saturating_add(fresh_count)).is_err()
        {
            return false;
        }
        let new_id = NodeId(offset + fresh.root.0);
        let old_unreadable = self.unreadable_below(id);

        let Some(old) = self.nodes.get_mut(id.0 as usize) else {
            return false;
        };
        let name = old.name.clone();
        let before = (
            old.allocated,
            old.apparent,
            old.files_below,
            old.others_below,
        );
        old.allocated = 0;
        old.apparent = 0;
        old.files_below = 0;
        old.others_below = 0;
        old.children.clear();

        let fresh_root = fresh.root;
        let mut after = (0, 0, 0, 0);
        for (index, mut node) in fresh.nodes.into_iter().enumerate() {
            let is_root = index == fresh_root.0 as usize;
            node.parent = if is_root {
                Some(parent)
            } else {
                node.parent.map(|p| NodeId(p.0 + offset))
            };
            for child in &mut node.children {
                child.0 += offset;
            }
            if is_root {
                node.name = name.clone();
                after = (
                    node.allocated,
                    node.apparent,
                    node.files_below,
                    node.others_below,
                );
            }
            self.nodes.push(node);
        }
        if let Some(holder) = self.nodes.get_mut(parent.0 as usize) {
            for child in &mut holder.children {
                if *child == id {
                    *child = new_id;
                }
            }
        }
        let mut cursor = Some(parent);
        while let Some(ancestor) = cursor.and_then(|a| self.nodes.get_mut(a.0 as usize)) {
            ancestor.allocated = ancestor
                .allocated
                .saturating_sub(before.0)
                .saturating_add(after.0);
            ancestor.apparent = ancestor
                .apparent
                .saturating_sub(before.1)
                .saturating_add(after.1);
            ancestor.files_below = ancestor
                .files_below
                .saturating_sub(before.2)
                .saturating_add(after.2);
            ancestor.others_below = ancestor
                .others_below
                .saturating_sub(before.3)
                .saturating_add(after.3);
            cursor = ancestor.parent;
        }
        self.unreadable_dirs = self
            .unreadable_dirs
            .saturating_sub(old_unreadable)
            .saturating_add(fresh.unreadable_dirs);
        true
    }

    /// Unreadable folders at or below `id`.
    fn unreadable_below(&self, id: NodeId) -> u32 {
        let mut count = 0u32;
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let Some(node) = self.node(current) else {
                continue;
            };
            if node.unreadable {
                count = count.saturating_add(1);
            }
            stack.extend(node.children.iter().copied());
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Node, NodeId, Tree};
    use std::path::PathBuf;

    fn node(name: &str, kind: Kind, parent: Option<u32>, allocated: u64, files: u64) -> Node {
        Node {
            name: name.into(),
            kind,
            parent: parent.map(NodeId),
            allocated,
            apparent: allocated / 2,
            files_below: files,
            others_below: 0,
            unreadable: false,
            other_device: false,
            dev: 0,
            ino: 0,
            children: Vec::new(),
        }
    }

    /// root(70) ─ dir(70) ─ { c(30), a(20), b(20) }
    fn three_levels() -> Tree {
        let mut nodes = vec![
            node("root", Kind::Dir, None, 70, 3),
            node("dir", Kind::Dir, Some(0), 70, 3),
            node("a", Kind::File, Some(1), 20, 1),
            node("b", Kind::File, Some(1), 20, 1),
            node("c", Kind::File, Some(1), 30, 1),
        ];
        nodes[0].children = vec![NodeId(1)];
        nodes[1].children = vec![NodeId(2), NodeId(3), NodeId(4)];
        Tree {
            root: NodeId(0),
            path: PathBuf::from("/scan/root"),
            device: 1,
            nodes,
            unreadable_dirs: 0,
            hard_link_names: 0,
        }
    }

    #[test]
    fn path_of_joins_the_root_with_the_ancestors() {
        let tree = three_levels();
        assert_eq!(tree.path_of(NodeId(0)), PathBuf::from("/scan/root"));
        assert_eq!(tree.path_of(NodeId(4)), PathBuf::from("/scan/root/dir/c"));
    }

    #[test]
    fn children_by_size_orders_descending_then_by_name() {
        let tree = three_levels();
        assert_eq!(
            tree.children_by_size(NodeId(1)),
            vec![NodeId(4), NodeId(2), NodeId(3)]
        );
        assert!(tree.children_by_size(NodeId(99)).is_empty());
    }

    #[test]
    fn prune_subtracts_from_every_ancestor_and_unlinks() {
        let mut tree = three_levels();
        assert_eq!(tree.prune(NodeId(4)), Some(30));
        for id in [0, 1] {
            let n = tree.node(NodeId(id)).expect("ancestor");
            assert_eq!(n.allocated, 40);
            assert_eq!(n.apparent, 20);
            assert_eq!(n.files_below, 2);
        }
        assert_eq!(
            tree.node(NodeId(1)).expect("dir").children,
            vec![NodeId(2), NodeId(3)]
        );
        let pruned = tree.node(NodeId(4)).expect("ids are stable");
        assert_eq!((pruned.allocated, pruned.files_below), (0, 0));
        assert_eq!(tree.prune(NodeId(1)), Some(40));
        assert_eq!(tree.node(NodeId(0)).expect("root").allocated, 0);
    }

    #[test]
    fn prune_many_unlinks_each_parent_once_and_counts_nothing_twice() {
        let mut tree = three_levels();
        // c and a, a twice, the root and an unknown id: 50 bytes, once.
        assert_eq!(
            tree.prune_many(&[NodeId(4), NodeId(2), NodeId(2), NodeId(0), NodeId(99)]),
            50
        );
        assert_eq!(tree.node(NodeId(1)).expect("dir").children, vec![NodeId(3)]);
        for id in [0, 1] {
            let n = tree.node(NodeId(id)).expect("ancestor");
            assert_eq!((n.allocated, n.apparent, n.files_below), (20, 10, 1));
        }
        assert!(!tree.is_live(NodeId(2)) && !tree.is_live(NodeId(4)));

        // A folder and a file inside it: the folder already carries the file.
        let mut nested = three_levels();
        nested.nodes.push(node("side", Kind::File, Some(0), 5, 1));
        nested.nodes[0].children.push(NodeId(5));
        for ancestor in &mut nested.nodes[..1] {
            ancestor.allocated += 5;
            ancestor.files_below += 1;
        }
        assert_eq!(nested.prune_many(&[NodeId(3), NodeId(1)]), 70);
        let root = nested.node(NodeId(0)).expect("root");
        assert_eq!((root.allocated, root.files_below), (5, 1));
        // An id below a pruned folder is no longer in the tree: nothing moves.
        assert_eq!(nested.prune_many(&[NodeId(2)]), 0);
        assert_eq!(nested.prune(NodeId(2)), Some(0));
        assert_eq!(nested.node(NodeId(0)).expect("root").allocated, 5);
    }

    #[test]
    fn prune_refuses_the_root_and_unknown_ids() {
        let mut tree = three_levels();
        assert_eq!(tree.prune(NodeId(0)), None);
        assert_eq!(tree.prune(NodeId(99)), None);
        assert_eq!(tree.node(NodeId(0)).expect("root").allocated, 70);
    }
}
