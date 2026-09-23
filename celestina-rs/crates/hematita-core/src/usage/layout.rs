//! The squarified treemap (Bruls, Huizing and van Wijk, 2000) of one folder's
//! children.
//!
//! Pure arithmetic: sizes in, rectangles out, each tile's area proportional
//! to its size. Children below [`MIN_TILE_SHARE`] of the folder would be
//! slivers nobody can point at, so they merge into one remainder tile placed
//! last.

/// The share of the total below which a child joins the remainder tile.
pub const MIN_TILE_SHARE: f64 = 0.005;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One placed tile: `index` into the input sizes, or `None` for the merged
/// remainder.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub index: Option<usize>,
    pub rect: Rect,
}

/// Lays `sizes` out inside `rect`, biggest first (equal sizes by index), the
/// remainder last. Empty when the total or the rectangle's area is zero.
#[must_use]
pub fn squarify(sizes: &[u64], rect: Rect) -> Vec<Tile> {
    let total: f64 = sizes.iter().map(|&s| s as f64).sum();
    let whole = rect.w * rect.h;
    if total <= 0.0 || whole.is_nan() || whole <= 0.0 {
        return Vec::new();
    }
    let floor = total * MIN_TILE_SHARE;
    let mut kept: Vec<usize> = (0..sizes.len())
        .filter(|&i| sizes[i] as f64 >= floor && sizes[i] > 0)
        .collect();
    kept.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]).then_with(|| a.cmp(&b)));
    let remainder: f64 = (0..sizes.len())
        .filter(|&i| (sizes[i] as f64) < floor)
        .map(|i| sizes[i] as f64)
        .sum();

    let mut items: Vec<(Option<usize>, f64)> = kept
        .into_iter()
        .map(|i| (Some(i), sizes[i] as f64 / total * whole))
        .collect();
    if remainder > 0.0 {
        items.push((None, remainder / total * whole));
    }
    place(&items, rect)
}

/// The squarify loop: grow a row along the free rectangle's shorter side
/// while its worst aspect ratio improves, lay it down, repeat on what is left.
fn place(items: &[(Option<usize>, f64)], rect: Rect) -> Vec<Tile> {
    let mut tiles = Vec::with_capacity(items.len());
    let mut free = rect;
    let mut start = 0;
    while start < items.len() {
        let side = free.w.min(free.h);
        if side <= 0.0 {
            break;
        }
        let mut end = start + 1;
        let mut best = worst(&items[start..end], side);
        while end < items.len() {
            let next = worst(&items[start..=end], side);
            if next > best {
                break;
            }
            best = next;
            end += 1;
        }
        let row = &items[start..end];
        let row_area: f64 = row.iter().map(|(_, a)| a).sum();
        let last = start + row.len() == items.len();
        if free.w >= free.h {
            // A column against the left edge, as tall as the free space.
            let width = if last { free.w } else { row_area / free.h };
            let mut y = free.y;
            for (index, area) in row {
                let h = area / width;
                tiles.push(Tile {
                    index: *index,
                    rect: Rect {
                        x: free.x,
                        y,
                        w: width,
                        h,
                    },
                });
                y += h;
            }
            free = Rect {
                x: free.x + width,
                w: (free.w - width).max(0.0),
                ..free
            };
        } else {
            // A row against the top edge, as wide as the free space.
            let height = if last { free.h } else { row_area / free.w };
            let mut x = free.x;
            for (index, area) in row {
                let w = area / height;
                tiles.push(Tile {
                    index: *index,
                    rect: Rect {
                        x,
                        y: free.y,
                        w,
                        h: height,
                    },
                });
                x += w;
            }
            free = Rect {
                y: free.y + height,
                h: (free.h - height).max(0.0),
                ..free
            };
        }
        start = end;
    }
    tiles
}

/// The worst aspect ratio of `row` laid along a side of length `side`.
fn worst(row: &[(Option<usize>, f64)], side: f64) -> f64 {
    let sum: f64 = row.iter().map(|(_, a)| a).sum();
    let side2 = side * side;
    let sum2 = sum * sum;
    row.iter()
        .map(|(_, a)| (side2 * a / sum2).max(sum2 / (side2 * a)))
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::{squarify, Rect, Tile, MIN_TILE_SHARE};

    const EPS: f64 = 1e-9;
    const UNIT: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    fn area(r: &Rect) -> f64 {
        r.w * r.h
    }

    fn overlap(a: &Rect, b: &Rect) -> f64 {
        let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        if w > 0.0 && h > 0.0 {
            w * h
        } else {
            0.0
        }
    }

    /// Area proportional, pairwise disjoint, inside `rect`, each index once.
    fn check(sizes: &[u64], rect: Rect, tiles: &[Tile]) {
        let total: f64 = sizes.iter().map(|&s| s as f64).sum();
        let whole = area(&rect);
        let mut remainder = 0.0;
        let mut seen = vec![false; sizes.len()];
        for tile in tiles {
            match tile.index {
                Some(i) => {
                    assert!(!seen[i], "index {i} placed twice");
                    seen[i] = true;
                    let expected = sizes[i] as f64 / total * whole;
                    assert!((area(&tile.rect) - expected).abs() < EPS, "tile {i}");
                }
                None => remainder = area(&tile.rect),
            }
            let r = tile.rect;
            assert!(r.w >= 0.0 && r.h >= 0.0);
            assert!(r.x >= rect.x - EPS && r.y >= rect.y - EPS);
            assert!(r.x + r.w <= rect.x + rect.w + EPS);
            assert!(r.y + r.h <= rect.y + rect.h + EPS);
        }
        let merged: f64 = sizes
            .iter()
            .enumerate()
            .filter(|(i, _)| !seen[*i])
            .map(|(_, &s)| s as f64 / total * whole)
            .sum();
        assert!((remainder - merged).abs() < EPS);
        for (i, a) in tiles.iter().enumerate() {
            for b in &tiles[i + 1..] {
                assert!(overlap(&a.rect, &b.rect) < EPS, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn nothing_to_lay_out_gives_no_tiles() {
        assert!(squarify(&[], UNIT).is_empty());
        assert!(squarify(&[0, 0, 0], UNIT).is_empty());
        let flat = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 0.0,
        };
        assert!(squarify(&[5], flat).is_empty());
    }

    #[test]
    fn a_single_item_fills_the_rectangle() {
        let rect = Rect {
            x: 2.0,
            y: 3.0,
            w: 4.0,
            h: 1.5,
        };
        assert_eq!(
            squarify(&[7], rect),
            vec![Tile {
                index: Some(0),
                rect
            }]
        );
    }

    #[test]
    fn equal_sizes_keep_index_order_and_proportion() {
        let sizes = [10; 7];
        let tiles = squarify(&sizes, UNIT);
        let order: Vec<_> = tiles.iter().map(|t| t.index).collect();
        assert_eq!(order, (0..7).map(Some).collect::<Vec<_>>());
        check(&sizes, UNIT, &tiles);
    }

    #[test]
    fn mixed_sizes_are_proportional_and_disjoint() {
        let sizes = [6, 6, 4, 3, 2, 2, 1];
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 6.0,
            h: 4.0,
        };
        let tiles = squarify(&sizes, rect);
        assert_eq!(tiles.first().and_then(|t| t.index), Some(0));
        check(&sizes, rect, &tiles);
    }

    #[test]
    fn slivers_merge_into_one_remainder_placed_last() {
        let mut sizes = vec![1_000_000];
        sizes.extend(std::iter::repeat_n(100, 50));
        sizes.push(0);
        let total: u64 = sizes.iter().sum();
        assert!((sizes[1] as f64) < total as f64 * MIN_TILE_SHARE);
        let tiles = squarify(&sizes, UNIT);
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].index, Some(0));
        assert_eq!(tiles[1].index, None);
        assert_eq!(tiles.iter().filter(|t| t.index.is_none()).count(), 1);
        check(&sizes, UNIT, &tiles);
    }
}
