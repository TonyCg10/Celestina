//! What a scanned folder looks like to a page, as plain values: its children
//! as rows biggest first with a merged remainder, the treemap in the flat
//! shape QML reads, and the unreadable folders counted up the tree.
//!
//! Nothing here knows Qt; each consumer turns these values into its own
//! properties and words.

use std::ffi::OsString;

use crate::usage::layout::Tile;
use crate::usage::tree::{Kind, NodeId, Tree};

/// The id the merged remainder carries, in rows and in the treemap.
pub const REMAINDER_ID: f64 = -1.0;
/// Children listed before the rest merge into one remainder row.
pub const MAX_ROWS: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// `None` is the merged remainder.
    pub id: Option<NodeId>,
    pub name: OsString,
    pub kind: Kind,
    pub allocated: u64,
    pub apparent: u64,
    /// `allocated` over the parent's `allocated`; 0 when the parent is empty.
    pub share: f64,
    pub files_below: u64,
    pub unreadable_below: u32,
    pub other_device: bool,
    /// Children merged into this row; 0 for a real child.
    pub merged: usize,
}

/// `current`'s children biggest first; beyond `limit` rows the rest become
/// one remainder row that keeps their sums. `unreadable` is
/// [`unreadable_below`]'s output, indexed by node.
#[must_use]
pub fn children_rows(tree: &Tree, current: NodeId, unreadable: &[u32], limit: usize) -> Vec<Row> {
    let Some(parent) = tree.node(current) else {
        return Vec::new();
    };
    let total = parent.allocated;
    let share = |allocated: u64| {
        if total > 0 {
            allocated as f64 / total as f64
        } else {
            0.0
        }
    };
    let below = |id: NodeId| unreadable.get(id.0 as usize).copied().unwrap_or(0);
    let children = tree.children_by_size(current);
    let (listed, rest) = if children.len() > limit && limit > 0 {
        children.split_at(limit - 1)
    } else {
        (children.as_slice(), &[][..])
    };
    let mut rows: Vec<Row> = listed
        .iter()
        .filter_map(|id| {
            tree.node(*id).map(|node| Row {
                id: Some(*id),
                name: node.name.clone(),
                kind: node.kind,
                allocated: node.allocated,
                apparent: node.apparent,
                share: share(node.allocated),
                files_below: node.files_below,
                unreadable_below: below(*id),
                other_device: node.other_device,
                merged: 0,
            })
        })
        .collect();
    if !rest.is_empty() {
        let mut remainder = Row {
            id: None,
            name: OsString::new(),
            kind: Kind::Other,
            allocated: 0,
            apparent: 0,
            share: 0.0,
            files_below: 0,
            unreadable_below: 0,
            other_device: false,
            merged: rest.len(),
        };
        for id in rest {
            if let Some(node) = tree.node(*id) {
                remainder.allocated = remainder.allocated.saturating_add(node.allocated);
                remainder.apparent = remainder.apparent.saturating_add(node.apparent);
                remainder.files_below = remainder.files_below.saturating_add(node.files_below);
                remainder.unreadable_below = remainder.unreadable_below.saturating_add(below(*id));
            }
        }
        remainder.share = share(remainder.allocated);
        rows.push(remainder);
    }
    rows
}

/// The treemap as QML reads it: `[id, x, y, w, h]` per tile; a `None` id and
/// squarify's own remainder both carry [`REMAINDER_ID`]. `ids` name the sizes
/// that were laid out, in the same order.
#[must_use]
pub fn flat_rects(ids: &[Option<NodeId>], tiles: &[Tile]) -> Vec<f64> {
    let mut flat = Vec::with_capacity(tiles.len() * 5);
    for tile in tiles {
        let id = match tile.index {
            Some(index) => match ids.get(index) {
                Some(Some(id)) => f64::from(id.0),
                Some(None) => REMAINDER_ID,
                None => continue,
            },
            None => REMAINDER_ID,
        };
        flat.extend([id, tile.rect.x, tile.rect.y, tile.rect.w, tile.rect.h]);
    }
    flat
}

/// Per node: how many unreadable directories sit at or below it.
#[must_use]
pub fn unreadable_below(tree: &Tree) -> Vec<u32> {
    let mut counts: Vec<u32> = tree.nodes.iter().map(|n| u32::from(n.unreadable)).collect();
    for index in (0..tree.nodes.len()).rev() {
        let count = counts[index];
        if count > 0 {
            if let Some(parent) = tree.nodes[index].parent {
                if let Some(total) = counts.get_mut(parent.0 as usize) {
                    *total = total.saturating_add(count);
                }
            }
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::layout::{squarify, Rect};
    use crate::usage::tree::Node;
    use std::path::PathBuf;

    fn node(name: &str, kind: Kind, parent: Option<u32>, allocated: u64) -> Node {
        Node {
            name: name.into(),
            kind,
            parent: parent.map(NodeId),
            allocated,
            apparent: allocated,
            files_below: u64::from(kind == Kind::File),
            others_below: 0,
            unreadable: false,
            other_device: false,
            dev: 0,
            ino: 0,
            children: Vec::new(),
        }
    }

    fn tree_of(nodes: Vec<Node>, unreadable_dirs: u32) -> Tree {
        Tree {
            root: NodeId(0),
            path: PathBuf::from("/data"),
            device: 1,
            nodes,
            unreadable_dirs,
            hard_link_names: 0,
        }
    }

    /// root(0) ─ a(1) ─ x(3), y(4); b(2) ─ c(5, empty dir), locked(6)
    fn tree() -> Tree {
        let mut nodes = vec![
            node("root", Kind::Dir, None, 40),
            node("a", Kind::Dir, Some(0), 30),
            node("b", Kind::Dir, Some(0), 10),
            node("x", Kind::File, Some(1), 15),
            node("y", Kind::File, Some(1), 15),
            node("c", Kind::Dir, Some(2), 0),
            node("locked", Kind::Dir, Some(2), 0),
        ];
        nodes[0].children = vec![NodeId(1), NodeId(2)];
        nodes[1].children = vec![NodeId(3), NodeId(4)];
        nodes[2].children = vec![NodeId(5), NodeId(6)];
        nodes[6].unreadable = true;
        tree_of(nodes, 1)
    }

    /// root(0) with four file children of 10, 40, 20 and 30 bytes.
    fn four_children() -> Tree {
        let mut nodes = vec![
            node("root", Kind::Dir, None, 100),
            node("d", Kind::File, Some(0), 10),
            node("a", Kind::File, Some(0), 40),
            node("c", Kind::File, Some(0), 20),
            node("b", Kind::File, Some(0), 30),
        ];
        nodes[0].children = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)];
        tree_of(nodes, 0)
    }

    /// An empty root folder.
    fn leaf_only() -> Tree {
        tree_of(vec![node("root", Kind::Dir, None, 0)], 0)
    }

    #[test]
    fn unreadable_folders_are_counted_up_to_the_root() {
        let counts = unreadable_below(&tree());
        assert_eq!(counts[0], 1);
        assert_eq!(counts[2], 1);
        assert_eq!(counts[1], 0);
    }

    #[test]
    fn the_flat_rects_round_trip_to_ids_and_tiles() {
        let children = [NodeId(11), NodeId(12), NodeId(13), NodeId(14)];
        let ids: Vec<Option<NodeId>> = children.iter().map(|id| Some(*id)).collect();
        let sizes = [600, 399, 1, 0];
        let unit = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        let tiles = squarify(&sizes, unit);
        let flat = flat_rects(&ids, &tiles);
        assert_eq!(flat.len(), tiles.len() * 5);
        for (chunk, tile) in flat.chunks(5).zip(&tiles) {
            let expected = tile
                .index
                .map_or(REMAINDER_ID, |i| f64::from(children[i].0));
            assert_eq!(chunk[0], expected);
            assert_eq!(
                (chunk[1], chunk[2], chunk[3], chunk[4]),
                (tile.rect.x, tile.rect.y, tile.rect.w, tile.rect.h)
            );
        }
        assert!(flat.chunks(5).any(|c| c[0] == REMAINDER_ID));
        let area: f64 = flat.chunks(5).map(|c| c[3] * c[4]).sum();
        assert!((area - 1.0).abs() < 1e-9);
    }

    #[test]
    fn children_rows_orders_by_size_and_merges_the_rest() {
        let tree = four_children();
        let unreadable = unreadable_below(&tree);
        let rows = children_rows(&tree, tree.root, &unreadable, 3);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].allocated, 40);
        assert_eq!(rows[1].allocated, 30);
        let rest = &rows[2];
        assert_eq!(rest.id, None);
        assert_eq!(rest.allocated, 30); // 20 + 10
        assert_eq!(rest.merged, 2);
        assert!((rows.iter().map(|r| r.share).sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn children_rows_under_the_limit_have_no_remainder() {
        let tree = four_children();
        let rows = children_rows(&tree, tree.root, &unreadable_below(&tree), 40);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| r.id.is_some() && r.merged == 0));
    }

    #[test]
    fn children_rows_of_an_empty_folder_have_zero_shares() {
        let tree = leaf_only();
        let rows = children_rows(&tree, tree.root, &unreadable_below(&tree), 40);
        assert!(rows.is_empty());
    }

    #[test]
    fn flat_rects_names_a_none_id_as_the_remainder() {
        let ids = [Some(NodeId(3)), None];
        let tiles = squarify(
            &[70, 30],
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
        );
        let flat = flat_rects(&ids, &tiles);
        assert_eq!(flat[0], 3.0);
        assert_eq!(flat[5], REMAINDER_ID);
    }
}
