//! Which nodes hold a member of a set, kept current one member at a time.
//!
//! A page's filters ask "does this folder hold a duplicate (or an empty
//! folder) at or below it?" of every child it lists. The answer is counted
//! per node rather than flagged, so taking one member out lowers exactly the
//! ancestors it raised: a verdict that clears one group costs that group's
//! members times the tree's depth, never a pass over the whole tree.

use crate::usage::tree::{NodeId, Tree};

/// A set of member nodes and, per node, how many members lie at or below it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    member: Vec<bool>,
    below: Vec<u32>,
}

impl Marks {
    /// The marks of `members` over `tree`; an id the tree does not hold, or
    /// one given twice, counts once or not at all.
    #[must_use]
    pub fn of(tree: &Tree, members: impl IntoIterator<Item = NodeId>) -> Self {
        let mut marks = Self {
            member: vec![false; tree.nodes.len()],
            below: vec![0; tree.nodes.len()],
        };
        for id in members {
            marks.insert(tree, id);
        }
        marks
    }

    /// Makes `id` a member and counts it up its ancestors; false when it
    /// already was one or the marks were not built for a node of that id.
    pub fn insert(&mut self, tree: &Tree, id: NodeId) -> bool {
        match self.member.get_mut(id.0 as usize) {
            Some(slot) if !*slot => *slot = true,
            _ => return false,
        }
        self.climb(tree, id, |count| count.saturating_add(1));
        true
    }

    /// Takes `id` out of the members and uncounts it up its ancestors; false
    /// when it was not a member.
    pub fn remove(&mut self, tree: &Tree, id: NodeId) -> bool {
        match self.member.get_mut(id.0 as usize) {
            Some(slot) if *slot => *slot = false,
            _ => return false,
        }
        self.climb(tree, id, |count| count.saturating_sub(1));
        true
    }

    /// Whether `id` is itself a member.
    #[must_use]
    pub fn contains(&self, id: NodeId) -> bool {
        self.member.get(id.0 as usize).copied().unwrap_or(false)
    }

    /// Whether `id` is a member or has one below it.
    #[must_use]
    pub fn holds(&self, id: NodeId) -> bool {
        self.below
            .get(id.0 as usize)
            .is_some_and(|count| *count > 0)
    }

    fn climb(&mut self, tree: &Tree, id: NodeId, step: impl Fn(u32) -> u32) {
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            if let Some(count) = self.below.get_mut(current.0 as usize) {
                *count = step(*count);
            }
            cursor = tree.node(current).and_then(|node| node.parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::tree::{Kind, Node, NodeId, Tree};
    use std::path::PathBuf;

    fn node(kind: Kind, parent: Option<u32>, children: &[u32]) -> Node {
        Node {
            name: "n".into(),
            kind,
            parent: parent.map(NodeId),
            allocated: 1,
            apparent: 1,
            files_below: 0,
            others_below: 0,
            unreadable: false,
            other_device: false,
            dev: 0,
            ino: 0,
            children: children.iter().copied().map(NodeId).collect(),
        }
    }

    /// root(0) ─ a(1) ─ x(3), y(4); b(2) ─ c(5) ─ z(6)
    fn tree() -> Tree {
        Tree {
            root: NodeId(0),
            path: PathBuf::from("/data"),
            device: 1,
            nodes: vec![
                node(Kind::Dir, None, &[1, 2]),
                node(Kind::Dir, Some(0), &[3, 4]),
                node(Kind::Dir, Some(0), &[5]),
                node(Kind::File, Some(1), &[]),
                node(Kind::File, Some(1), &[]),
                node(Kind::Dir, Some(2), &[6]),
                node(Kind::File, Some(5), &[]),
            ],
            unreadable_dirs: 0,
            hard_link_names: 0,
        }
    }

    fn held(marks: &Marks, len: u32) -> Vec<u32> {
        (0..len).filter(|id| marks.holds(NodeId(*id))).collect()
    }

    #[test]
    fn a_member_marks_itself_and_every_ancestor() {
        let tree = tree();
        let marks = Marks::of(&tree, [NodeId(3)]);
        assert!(marks.contains(NodeId(3)));
        assert!(
            !marks.contains(NodeId(1)),
            "an ancestor holds, it is not a member"
        );
        assert_eq!(held(&marks, 7), vec![0, 1, 3]);
    }

    #[test]
    fn removing_one_of_two_members_keeps_the_shared_ancestors() {
        let tree = tree();
        let mut marks = Marks::of(&tree, [NodeId(3), NodeId(4), NodeId(6)]);
        assert!(marks.remove(&tree, NodeId(3)));
        assert_eq!(held(&marks, 7), vec![0, 1, 2, 4, 5, 6]);
        assert!(marks.remove(&tree, NodeId(4)));
        assert_eq!(held(&marks, 7), vec![0, 2, 5, 6], "a(1) holds nothing now");
        assert!(!marks.remove(&tree, NodeId(4)), "not a member any more");
        assert!(!marks.insert(&tree, NodeId(6)), "already a member");
        assert!(!marks.insert(&tree, NodeId(99)), "not a node");
    }

    #[test]
    fn incremental_changes_match_a_rebuild_of_the_same_set() {
        let tree = tree();
        let mut marks = Marks::of(&tree, [NodeId(3), NodeId(4), NodeId(5), NodeId(6)]);
        marks.remove(&tree, NodeId(4));
        marks.remove(&tree, NodeId(5));
        marks.insert(&tree, NodeId(2));
        assert_eq!(marks, Marks::of(&tree, [NodeId(3), NodeId(6), NodeId(2)]));
    }

    #[test]
    fn an_empty_set_holds_nothing() {
        let tree = tree();
        let marks = Marks::of(&tree, []);
        assert!(held(&marks, 7).is_empty());
        assert!(!Marks::default().holds(NodeId(0)));
    }
}
