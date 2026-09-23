//! Empty folders: directories with no file at any depth.
//!
//! A folder whose only content is empty folders is itself empty, so the list
//! is deepest first: trashing in that order never leaves a parent behind. A
//! folder is not called empty when anything below it could not be seen — an
//! unreadable directory or another device's mount — because "nothing found"
//! is then not "nothing there".

use super::tree::{Kind, NodeId, Tree};

/// Every empty folder under the root (never the root itself), deepest first,
/// ties by id.
#[must_use]
pub fn empty_folders(tree: &Tree) -> Vec<NodeId> {
    // Whether anything at or below a node went unseen, propagated bottom-up:
    // a child always sits after its parent in the arena.
    let mut unseen: Vec<bool> = tree
        .nodes
        .iter()
        .map(|n| n.unreadable || n.other_device)
        .collect();
    for index in (0..tree.nodes.len()).rev() {
        if unseen[index] {
            if let Some(parent) = tree.nodes[index].parent {
                if let Some(flag) = unseen.get_mut(parent.0 as usize) {
                    *flag = true;
                }
            }
        }
    }
    let mut empty: Vec<(usize, NodeId)> = tree
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            node.kind == Kind::Dir
                && node.files_below == 0
                && !unseen[*index]
                && node.parent.is_some()
                && *index != tree.root.0 as usize
        })
        .filter_map(|(index, _)| {
            let id = NodeId(u32::try_from(index).ok()?);
            Some((depth(tree, id), id))
        })
        .collect();
    empty.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1 .0.cmp(&b.1 .0)));
    empty.into_iter().map(|(_, id)| id).collect()
}

fn depth(tree: &Tree, id: NodeId) -> usize {
    let mut depth = 0;
    let mut cursor = tree.node(id).and_then(|n| n.parent);
    while let Some(parent) = cursor {
        depth += 1;
        cursor = tree.node(parent).and_then(|n| n.parent);
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::empty_folders;
    use crate::usage::tree::{Kind, Node, NodeId, Tree};
    use std::path::PathBuf;

    fn dir(parent: Option<u32>, children: &[u32]) -> Node {
        Node {
            name: "d".into(),
            kind: Kind::Dir,
            parent: parent.map(NodeId),
            allocated: 0,
            apparent: 0,
            files_below: 0,
            unreadable: false,
            other_device: false,
            children: children.iter().copied().map(NodeId).collect(),
        }
    }

    #[test]
    fn nothing_seen_below_an_unreadable_or_foreign_folder_is_not_empty() {
        // 0 ─ 1 ─ 2 (unreadable)
        //   ─ 3 ─ 4 (another device)
        //   ─ 5 ─ 6
        let mut nodes = vec![
            dir(None, &[1, 3, 5]),
            dir(Some(0), &[2]),
            dir(Some(1), &[]),
            dir(Some(0), &[4]),
            dir(Some(3), &[]),
            dir(Some(0), &[6]),
            dir(Some(5), &[]),
        ];
        nodes[2].unreadable = true;
        nodes[4].other_device = true;
        let tree = Tree {
            root: NodeId(0),
            path: PathBuf::from("/r"),
            device: 1,
            nodes,
            unreadable_dirs: 1,
        };
        assert_eq!(empty_folders(&tree), vec![NodeId(6), NodeId(5)]);
    }
}
