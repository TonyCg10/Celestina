//! What the analysed mode shows of a scanned tree, as plain values.
//!
//! The hub owns the tree; these functions decide which children a filter
//! keeps, which rows a verified group becomes, and how the treemap crosses to
//! QML, without a QObject — so each rule is tested on a hand-built tree.

use hematita_core::usage::duplicates::{self, Group, Verified};
use hematita_core::usage::empty::empty_folders;
use hematita_core::usage::layout::Tile;
use hematita_core::usage::tree::{NodeId, Tree};

/// The id the treemap's merged remainder carries.
pub const REMAINDER_ID: f64 = -1.0;

/// The duplicate candidates a scan offers, biggest size first. A home folder
/// holds tens of thousands of equal-sized files; the page lists, and the
/// content check reads, only the groups that free the most.
pub const SHOWN_GROUPS: usize = 500;

/// What the scan thread derives from a finished tree, so the Qt thread never
/// walks the whole arena.
#[derive(Clone, Debug, Default)]
pub struct Findings {
    pub candidates: Vec<Group>,
    pub empty: Vec<NodeId>,
    pub unreadable: Vec<u32>,
}

#[must_use]
pub fn findings(tree: &Tree) -> Findings {
    let mut candidates = duplicates::candidates(tree);
    candidates.truncate(SHOWN_GROUPS);
    Findings {
        candidates,
        empty: empty_folders(tree),
        unreadable: unreadable_below(tree),
    }
}

/// Per node: whether it is in `set` or has something of `set` below it. Each
/// member climbs its ancestors until it meets one already marked, so the cost
/// follows the set, not the tree.
#[must_use]
pub fn marks_below(tree: &Tree, set: impl IntoIterator<Item = NodeId>) -> Vec<bool> {
    let mut marks = vec![false; tree.nodes.len()];
    for id in set {
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            match marks.get_mut(current.0 as usize) {
                Some(mark) if !*mark => *mark = true,
                _ => break,
            }
            cursor = tree.node(current).and_then(|n| n.parent);
        }
    }
    marks
}

/// Per node: whether it is exactly one of `set`.
#[must_use]
pub fn marks_exact(len: usize, set: impl IntoIterator<Item = NodeId>) -> Vec<bool> {
    let mut marks = vec![false; len];
    for id in set {
        if let Some(mark) = marks.get_mut(id.0 as usize) {
            *mark = true;
        }
    }
    marks
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

/// The children a filter keeps: all of them when no filter is on, otherwise
/// those marked by every active filter.
#[must_use]
pub fn project(children: &[NodeId], filters: &[&[bool]]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|id| {
            filters
                .iter()
                .all(|marks| marks.get(id.0 as usize).copied().unwrap_or(false))
        })
        .collect()
}

/// A selection that keeps one copy: every member but the lowest id.
#[must_use]
pub fn all_but_one(nodes: &[NodeId]) -> Vec<NodeId> {
    let Some(keep) = nodes.iter().map(|id| id.0).min() else {
        return Vec::new();
    };
    nodes.iter().copied().filter(|id| id.0 != keep).collect()
}

/// One published duplicate row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRow {
    pub size: u64,
    pub nodes: Vec<NodeId>,
    pub verified: bool,
}

/// The rows the page lists: each candidate as it is, or, once checked, the
/// sets it split into (none when no two files matched).
#[must_use]
pub fn group_rows(candidates: &[Group], verdicts: &[Option<Vec<Verified>>]) -> Vec<GroupRow> {
    let mut rows = Vec::new();
    for (index, group) in candidates.iter().enumerate() {
        match verdicts.get(index).and_then(Option::as_ref) {
            Some(sets) => rows.extend(sets.iter().map(|set| GroupRow {
                size: set.size,
                nodes: set.nodes.clone(),
                verified: true,
            })),
            None => rows.push(GroupRow {
                size: group.size,
                nodes: group.nodes.clone(),
                verified: false,
            }),
        }
    }
    rows
}

/// The treemap as QML reads it: `[id, x, y, w, h]` per tile, the remainder
/// with [`REMAINDER_ID`]. `children` are the ids whose sizes were laid out,
/// in the same order.
#[must_use]
pub fn flat_rects(children: &[NodeId], tiles: &[Tile]) -> Vec<f64> {
    let mut flat = Vec::with_capacity(tiles.len() * 5);
    for tile in tiles {
        let id = match tile.index {
            Some(index) => match children.get(index) {
                Some(id) => f64::from(id.0),
                None => continue,
            },
            None => REMAINDER_ID,
        };
        flat.extend([id, tile.rect.x, tile.rect.y, tile.rect.w, tile.rect.h]);
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;
    use hematita_core::usage::layout::{squarify, Rect};
    use hematita_core::usage::tree::{Kind, Node};
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
            children: Vec::new(),
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
        Tree {
            root: NodeId(0),
            path: PathBuf::from("/data"),
            device: 1,
            nodes,
            unreadable_dirs: 1,
        }
    }

    #[test]
    fn a_filter_keeps_the_children_holding_a_match() {
        let tree = tree();
        let duplicates = marks_below(&tree, [NodeId(3), NodeId(4)]);
        let empty = marks_below(&tree, [NodeId(5)]);
        let children = tree.children_by_size(tree.root);
        assert_eq!(project(&children, &[]), vec![NodeId(1), NodeId(2)]);
        assert_eq!(project(&children, &[&duplicates]), vec![NodeId(1)]);
        assert_eq!(project(&children, &[&empty]), vec![NodeId(2)]);
        assert!(project(&children, &[&duplicates, &empty]).is_empty());
        assert_eq!(
            project(&tree.children_by_size(NodeId(1)), &[&duplicates]),
            vec![NodeId(3), NodeId(4)]
        );
    }

    #[test]
    fn exact_marks_name_only_the_members() {
        let marks = marks_exact(4, [NodeId(1), NodeId(9)]);
        assert_eq!(marks, vec![false, true, false, false]);
    }

    #[test]
    fn unreadable_folders_are_counted_up_to_the_root() {
        let counts = unreadable_below(&tree());
        assert_eq!(counts[0], 1);
        assert_eq!(counts[2], 1);
        assert_eq!(counts[1], 0);
    }

    #[test]
    fn all_but_one_keeps_the_lowest_id() {
        assert_eq!(
            all_but_one(&[NodeId(9), NodeId(4), NodeId(7)]),
            vec![NodeId(9), NodeId(7)]
        );
        assert!(all_but_one(&[]).is_empty());
    }

    #[test]
    fn a_checked_group_becomes_its_verified_sets() {
        let candidates = vec![
            Group {
                size: 8,
                nodes: vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)],
            },
            Group {
                size: 4,
                nodes: vec![NodeId(5), NodeId(6)],
            },
            Group {
                size: 2,
                nodes: vec![NodeId(7), NodeId(8)],
            },
        ];
        let verdicts = vec![
            Some(vec![
                Verified {
                    size: 8,
                    nodes: vec![NodeId(1), NodeId(3)],
                },
                Verified {
                    size: 8,
                    nodes: vec![NodeId(2), NodeId(4)],
                },
            ]),
            Some(Vec::new()),
            None,
        ];
        let rows = group_rows(&candidates, &verdicts);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].verified && rows[1].verified && !rows[2].verified);
        assert_eq!(rows[1].nodes, vec![NodeId(2), NodeId(4)]);
        assert_eq!(rows[2].nodes, vec![NodeId(7), NodeId(8)]);
    }

    #[test]
    fn the_flat_rects_round_trip_to_ids_and_tiles() {
        let children = vec![NodeId(11), NodeId(12), NodeId(13), NodeId(14)];
        let sizes = [600, 399, 1, 0];
        let unit = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        let tiles = squarify(&sizes, unit);
        let flat = flat_rects(&children, &tiles);
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
}
