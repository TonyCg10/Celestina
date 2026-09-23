//! The scanned tree: an indexed arena of nodes whose sizes are aggregates of
//! their subtree.
//!
//! Nodes live in one `Vec` addressed by [`NodeId`], so an id handed to the
//! interface stays valid for the life of the scan, including after
//! [`Tree::prune`] (a pruned node is emptied and unlinked, never moved).

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
    /// The directory could not be listed; what it holds is not in the tree.
    pub unreadable: bool,
    /// A directory on another device than the scanned root: listed as a leaf
    /// with no size, because a mount is analysed from its own root.
    pub other_device: bool,
    pub children: Vec<NodeId>,
}

#[derive(Clone, Debug)]
pub struct Tree {
    pub root: NodeId,
    pub path: PathBuf,
    pub device: u64,
    pub nodes: Vec<Node>,
    pub unreadable_dirs: u32,
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

    /// Removes `id`'s subtree from the aggregates of every ancestor and
    /// unlinks it from its parent; answers the allocated bytes removed.
    /// `None` for the root or an unknown id.
    pub fn prune(&mut self, id: NodeId) -> Option<u64> {
        if id == self.root {
            return None;
        }
        let node = self.nodes.get_mut(id.0 as usize)?;
        let (allocated, apparent, files) = (node.allocated, node.apparent, node.files_below);
        let parent = node.parent;
        node.allocated = 0;
        node.apparent = 0;
        node.files_below = 0;
        node.children.clear();
        if let Some(parent) = parent.and_then(|p| self.nodes.get_mut(p.0 as usize)) {
            parent.children.retain(|child| *child != id);
        }
        let mut cursor = parent;
        while let Some(ancestor) = cursor.and_then(|a| self.nodes.get_mut(a.0 as usize)) {
            ancestor.allocated = ancestor.allocated.saturating_sub(allocated);
            ancestor.apparent = ancestor.apparent.saturating_sub(apparent);
            ancestor.files_below = ancestor.files_below.saturating_sub(files);
            cursor = ancestor.parent;
        }
        Some(allocated)
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
            unreadable: false,
            other_device: false,
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
    fn prune_refuses_the_root_and_unknown_ids() {
        let mut tree = three_levels();
        assert_eq!(tree.prune(NodeId(0)), None);
        assert_eq!(tree.prune(NodeId(99)), None);
        assert_eq!(tree.node(NodeId(0)).expect("root").allocated, 70);
    }
}
