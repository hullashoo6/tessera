//! Treemap layout.
//!
//! * `squarify` implements the squarified algorithm of Bruls, Huizing and van
//!   Wijk (2000): rectangle areas are exactly proportional to weights while
//!   aspect ratios stay close to 1.
//! * `layout` nests containers breadth-first, reserves a header strip for
//!   opened containers, merges items too small to see into one "aggregate"
//!   tile per container, and caps the total number of tiles so rendering
//!   cost is bounded regardless of how many files a folder holds.
//!
//! The crate knows nothing about files: callers supply children by id.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }

    pub fn area(&self) -> f64 {
        self.w * self.h
    }

    /// Shrinks the rectangle by the given margins, never below zero size.
    pub fn inset(&self, left: f64, top: f64, right: f64, bottom: f64) -> Rect {
        let w = (self.w - left - right).max(0.0);
        let h = (self.h - top - bottom).max(0.0);
        Rect::new(self.x + left.min(self.w), self.y + top.min(self.h), w, h)
    }

    /// Whether `other` lies within `self`, allowing for rounding error.
    pub fn contains(&self, other: &Rect) -> bool {
        const E: f64 = 1e-6;
        other.x >= self.x - E
            && other.y >= self.y - E
            && other.x + other.w <= self.x + self.w + E
            && other.y + other.h <= self.y + self.h + E
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        const E: f64 = 1e-6;
        self.x + E < other.x + other.w
            && other.x + E < self.x + self.w
            && self.y + E < other.y + other.h
            && other.y + E < self.y + self.h
    }
}

/// Lays out positive weights, sorted in descending order, inside `bounds`.
/// Returns one rectangle per weight, in input order. Each area equals
/// `bounds.area() * weight / sum(weights)` up to floating-point error.
pub fn squarify(weights: &[f64], bounds: Rect) -> Vec<Rect> {
    let n = weights.len();
    let mut out = vec![Rect::default(); n];
    let total: f64 = weights.iter().sum();
    if n == 0 || total <= 0.0 || bounds.area() <= 0.0 {
        return out;
    }
    debug_assert!(
        weights.windows(2).all(|w| w[0] >= w[1]),
        "weights must be sorted descending"
    );
    let scale = bounds.area() / total;
    let areas: Vec<f64> = weights.iter().map(|w| w * scale).collect();

    let mut free = bounds;
    let mut i = 0;
    while i < n {
        let side = free.w.min(free.h);
        let mut j = i + 1;
        let mut sum = areas[i];
        let mut current = worst(areas[i], areas[i], sum, side);
        while j < n {
            let s = sum + areas[j];
            // Sorted descending: the row's largest is areas[i], smallest areas[j].
            let w = worst(areas[i], areas[j], s, side);
            if w > current {
                break;
            }
            current = w;
            sum = s;
            j += 1;
        }
        place_row(&areas[i..j], sum, &mut free, &mut out[i..j], j == n);
        i = j;
    }
    out
}

/// Worst aspect ratio in a row with the given extreme areas and total.
fn worst(max: f64, min: f64, sum: f64, side: f64) -> f64 {
    if min <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let s2 = sum * sum;
    let w2 = side * side;
    (w2 * max / s2).max(s2 / (w2 * min))
}

fn place_row(row: &[f64], sum: f64, free: &mut Rect, out: &mut [Rect], last: bool) {
    let n = row.len();
    if free.w >= free.h {
        // Column along the left edge.
        let thick = if last {
            free.w
        } else {
            (sum / free.h).min(free.w)
        };
        let mut y = free.y;
        for (k, a) in row.iter().enumerate() {
            let h = if k + 1 == n {
                free.y + free.h - y
            } else {
                a / thick
            };
            out[k] = Rect::new(free.x, y, thick, h.max(0.0));
            y += h;
        }
        free.x += thick;
        free.w = (free.w - thick).max(0.0);
    } else {
        // Row along the top edge.
        let thick = if last {
            free.h
        } else {
            (sum / free.w).min(free.h)
        };
        let mut x = free.x;
        for (k, a) in row.iter().enumerate() {
            let w = if k + 1 == n {
                free.x + free.w - x
            } else {
                a / thick
            };
            out[k] = Rect::new(x, free.y, w.max(0.0), thick);
            x += w;
        }
        free.y += thick;
        free.h = (free.h - thick).max(0.0);
    }
}

/// A child supplied by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: u32,
    pub weight: u64,
    /// May have children of its own (a folder).
    pub container: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileKind {
    Leaf,
    Container,
    /// Several items too small to draw, merged. `Tile::id` is their parent.
    Aggregate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub id: u32,
    pub kind: TileKind,
    /// 0 for direct children of the layout root.
    pub depth: u16,
    pub rect: Rect,
    pub weight: u64,
    /// Items represented (more than 1 only for aggregates).
    pub count: u32,
    /// Position of the depth-0 ancestor among the root's children, for
    /// colouring a whole branch consistently.
    pub branch: u32,
    /// Container whose contents are drawn inside it (below its header).
    pub opened: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Levels of nesting drawn (1 = only the root's children).
    pub max_depth: u16,
    /// Items whose tile would be smaller than this area are aggregated.
    pub min_area: f64,
    /// Height of the label strip at the top of an opened container.
    pub header: f64,
    /// Inset between an opened container and its contents.
    pub gap: f64,
    /// Containers narrower or shorter than this are drawn closed.
    pub min_open_side: f64,
    /// Upper bound on the number of tiles produced.
    pub max_tiles: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            max_depth: 4,
            min_area: 36.0,
            header: 18.0,
            gap: 2.0,
            min_open_side: 44.0,
            max_tiles: 5000,
        }
    }
}

/// Lays out the subtree under `root` within `bounds`. Tiles are returned
/// parents-first, so painting in order draws children on top.
pub fn layout(
    root: u32,
    bounds: Rect,
    params: &Params,
    mut children: impl FnMut(u32) -> Vec<Item>,
) -> Vec<Tile> {
    let mut tiles: Vec<Tile> = Vec::new();
    // (container id, rect for its contents, depth, branch, index of its tile)
    let mut queue: VecDeque<(u32, Rect, u16, u32, Option<usize>)> = VecDeque::new();
    queue.push_back((root, bounds, 0, 0, None));

    while let Some((id, rect, depth, branch, tile_index)) = queue.pop_front() {
        let budget = params.max_tiles.saturating_sub(tiles.len());
        if budget == 0 || rect.area() <= 0.0 {
            if let Some(t) = tile_index {
                tiles[t].opened = false;
            }
            continue;
        }
        let mut items: Vec<Item> = children(id).into_iter().filter(|i| i.weight > 0).collect();
        if items.is_empty() {
            if let Some(t) = tile_index {
                tiles[t].opened = false;
            }
            continue;
        }
        // Stable sort: callers' order is kept for equal weights, so a list
        // sorted the same way shows items (and colours) in the same order.
        items.sort_by_key(|i| std::cmp::Reverse(i.weight));
        let total: u64 = items.iter().map(|i| i.weight).sum();
        let scale = rect.area() / total as f64;

        // Visible items: large enough to see, and within the tile budget.
        let mut cut = items
            .iter()
            .position(|it| (it.weight as f64) * scale < params.min_area)
            .unwrap_or(items.len());
        if items.len() - cut == 1 {
            cut = items.len(); // a single small item is shown, not merged
        }
        if cut > budget || (cut < items.len() && cut + 1 > budget) {
            cut = budget.saturating_sub(1);
        }
        // (item or aggregate, weight, count, position in sorted order)
        let mut entries: Vec<(Option<Item>, u64, u32, u32)> = items[..cut]
            .iter()
            .enumerate()
            .map(|(k, it)| (Some(*it), it.weight, 1, k as u32))
            .collect();
        let rest = &items[cut..];
        if !rest.is_empty() {
            let w: u64 = rest.iter().map(|i| i.weight).sum();
            entries.push((None, w, rest.len() as u32, cut as u32));
            entries.sort_by_key(|e| std::cmp::Reverse(e.1));
        }

        let weights: Vec<f64> = entries.iter().map(|e| e.1 as f64).collect();
        let rects = squarify(&weights, rect);
        for (entry, r) in entries.iter().zip(rects) {
            let br = if depth == 0 { entry.3 } else { branch };
            match entry.0 {
                None => tiles.push(Tile {
                    id,
                    kind: TileKind::Aggregate,
                    depth,
                    rect: r,
                    weight: entry.1,
                    count: entry.2,
                    branch: br,
                    opened: false,
                }),
                Some(it) => {
                    let open = it.container
                        && depth + 1 < params.max_depth
                        && r.w >= params.min_open_side
                        && r.h >= params.min_open_side + params.header;
                    tiles.push(Tile {
                        id: it.id,
                        kind: if it.container {
                            TileKind::Container
                        } else {
                            TileKind::Leaf
                        },
                        depth,
                        rect: r,
                        weight: it.weight,
                        count: 1,
                        branch: br,
                        opened: open,
                    });
                    if open {
                        let g = params.gap;
                        let inner = r.inset(g, params.header, g, g);
                        queue.push_back((it.id, inner, depth + 1, br, Some(tiles.len() - 1)));
                    }
                }
            }
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed >> 33
    }

    fn weights(n: usize, seed: u64) -> Vec<f64> {
        let mut s = seed;
        let mut w: Vec<f64> = (0..n).map(|_| (lcg(&mut s) % 10_000 + 1) as f64).collect();
        w.sort_by(|a, b| b.partial_cmp(a).unwrap());
        w
    }

    #[test]
    fn areas_are_proportional_and_inside_bounds() {
        let bounds = Rect::new(10.0, 20.0, 1200.0, 700.0);
        for (n, seed) in [(1, 1), (2, 2), (7, 3), (50, 4), (500, 5)] {
            let w = weights(n, seed);
            let total: f64 = w.iter().sum();
            let rects = squarify(&w, bounds);
            let mut area = 0.0;
            for (wi, r) in w.iter().zip(&rects) {
                let expected = bounds.area() * wi / total;
                assert!(
                    (r.area() - expected).abs() <= expected * 1e-6 + 1e-6,
                    "n={n}"
                );
                assert!(bounds.contains(r), "rect {r:?} escapes bounds");
                area += r.area();
            }
            assert!((area - bounds.area()).abs() < 1e-3);
        }
    }

    #[test]
    fn rectangles_do_not_overlap() {
        let w = weights(80, 9);
        let rects = squarify(&w, Rect::new(0.0, 0.0, 900.0, 600.0));
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                assert!(!rects[i].overlaps(&rects[j]), "{i} overlaps {j}");
            }
        }
    }

    #[test]
    fn equal_weights_in_a_square_are_square() {
        let rects = squarify(&[1.0; 16], Rect::new(0.0, 0.0, 400.0, 400.0));
        for r in rects {
            let ratio = r.w.max(r.h) / r.w.min(r.h);
            assert!(ratio < 1.5, "aspect ratio {ratio}");
        }
    }

    #[test]
    fn squarified_beats_slicing_on_aspect_ratio() {
        let w = weights(40, 11);
        let rects = squarify(&w, Rect::new(0.0, 0.0, 1000.0, 600.0));
        let worst = rects
            .iter()
            .map(|r| r.w.max(r.h) / r.w.min(r.h))
            .fold(0.0, f64::max);
        // Slice-and-dice of 40 items across 1000 px gives ratios far above 20.
        assert!(worst < 20.0, "worst aspect ratio {worst}");
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        assert!(squarify(&[], Rect::new(0.0, 0.0, 10.0, 10.0)).is_empty());
        let r = squarify(&[5.0, 1.0], Rect::new(0.0, 0.0, 0.0, 10.0));
        assert_eq!(r.len(), 2);
    }

    /// A synthetic tree: node 0 has 3 folders (1..=3) and 2000 tiny files;
    /// each folder has 20 files.
    fn tree(id: u32) -> Vec<Item> {
        match id {
            0 => {
                let mut v = vec![
                    Item {
                        id: 1,
                        weight: 5_000_000,
                        container: true,
                    },
                    Item {
                        id: 2,
                        weight: 3_000_000,
                        container: true,
                    },
                    Item {
                        id: 3,
                        weight: 1_000_000,
                        container: true,
                    },
                ];
                v.extend((0..2000).map(|k| Item {
                    id: 1000 + k,
                    weight: 10,
                    container: false,
                }));
                v
            }
            1..=3 => (0..20)
                .map(|k| Item {
                    id: 100 * id + k,
                    weight: 1 + k as u64 * 1000,
                    container: false,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn nested_layout_aggregates_and_nests() {
        let bounds = Rect::new(0.0, 0.0, 1000.0, 700.0);
        let tiles = layout(0, bounds, &Params::default(), tree);
        let aggregates: Vec<_> = tiles
            .iter()
            .filter(|t| t.kind == TileKind::Aggregate)
            .collect();
        assert_eq!(aggregates.iter().filter(|t| t.depth == 0).count(), 1);
        assert_eq!(
            aggregates.iter().find(|t| t.depth == 0).unwrap().count,
            2000
        );
        // Every child tile lies inside its parent's tile.
        for t in tiles.iter().filter(|t| t.depth == 1) {
            let parent_id = if t.kind == TileKind::Aggregate {
                t.id
            } else {
                t.id / 100
            };
            let parent = tiles
                .iter()
                .find(|p| p.id == parent_id && p.depth == 0)
                .unwrap();
            assert!(parent.opened);
            assert!(parent.rect.contains(&t.rect));
            assert_eq!(parent.branch, t.branch);
        }
        // Depth-0 areas are proportional to weights.
        let total: u64 = tree(0).iter().map(|i| i.weight).sum();
        for t in tiles.iter().filter(|t| t.depth == 0) {
            let expected = bounds.area() * t.weight as f64 / total as f64;
            assert!((t.rect.area() - expected).abs() < expected * 1e-6 + 1e-6);
        }
    }

    #[test]
    fn tile_budget_is_respected() {
        let params = Params {
            max_tiles: 50,
            min_area: 0.0,
            ..Params::default()
        };
        let tiles = layout(0, Rect::new(0.0, 0.0, 4000.0, 4000.0), &params, tree);
        assert!(tiles.len() <= 50, "{} tiles", tiles.len());
        // Containers whose contents were cut by the budget are not marked opened.
        for t in tiles.iter().filter(|t| t.opened) {
            assert!(
                tiles
                    .iter()
                    .any(|c| c.depth == t.depth + 1 && t.rect.contains(&c.rect))
            );
        }
    }

    #[test]
    fn depth_limit_is_respected() {
        let params = Params {
            max_depth: 1,
            ..Params::default()
        };
        let tiles = layout(0, Rect::new(0.0, 0.0, 1000.0, 700.0), &params, tree);
        assert!(tiles.iter().all(|t| t.depth == 0 && !t.opened));
    }
}
