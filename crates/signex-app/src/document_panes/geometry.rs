//! Where the panes of the document area sit (mockup row 207): their sizes
//! after a splitter drag, the splitters between them, and Tile Grid's panes.
//! Plain numbers, so the rules test without a window. Design:
//! docs/MENU_BAR.md, "Panes".

use crate::app::{Tile, TileLayout, grid_tiles, linear_tiles, tiles};

use super::model::MAX_PANES;

/// Height of the tab strip on top of each pane.
pub const PANE_STRIP_HEIGHT: f32 = 26.0;

/// The narrowest a pane gets while a splitter is dragged, in pixels.
pub const MIN_PANE_EXTENT: f32 = 80.0;

/// The width of a splitter's grip, in pixels, centred on the pane edge.
pub const SPLITTER_GRIP: f32 = 6.0;

/// How the panes share the document area after a splitter drag: the share
/// of each pane along the split (side by side, stacked), or of the first
/// column and the first row (grid). It holds for the arrangement it was
/// made for only; any other arrangement is equal.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneShares {
    pub layout: TileLayout,
    pub count: usize,
    pub values: Vec<f32>,
}

impl PaneShares {
    /// The shares for `count` panes in `layout`; empty (equal) when these
    /// were made for another arrangement.
    pub fn for_view(&self, layout: TileLayout, count: usize) -> &[f32] {
        if self.layout == layout && self.count == count {
            &self.values
        } else {
            &[]
        }
    }

    /// The shares as whole thousandths, the form they are kept in.
    pub fn per_mille(&self) -> Vec<u16> {
        self.values
            .iter()
            .map(|share| (share * 1000.0).round().clamp(0.0, 1000.0) as u16)
            .collect()
    }

    /// Shares kept as thousandths, for `count` panes in `layout`; `None`
    /// when a value is past a whole.
    pub fn from_per_mille(layout: TileLayout, count: usize, kept: &[u16]) -> Option<Self> {
        let values: Vec<f32> = kept
            .iter()
            .map(|n| (*n <= 1000).then(|| f32::from(*n) / 1000.0))
            .collect::<Option<_>>()?;
        Some(Self {
            layout,
            count,
            values,
        })
    }
}

/// Whether `shares` give each of `count` panes a part of a whole.
fn is_linear(shares: &[f32], count: usize) -> bool {
    shares.len() == count
        && shares.iter().all(|s| s.is_finite() && *s > 0.0)
        && (shares.iter().sum::<f32>() - 1.0).abs() < 0.01
}

/// The share of the first column and row of a grid, when `shares` hold one.
fn grid_split(shares: &[f32]) -> Option<(f32, f32)> {
    match shares {
        [column, row] if [*column, *row].iter().all(|s| *s > 0.0 && *s < 1.0) => {
            Some((*column, *row))
        }
        _ => None,
    }
}

/// The tiles of `count` panes over `area` in `layout`, sized by `shares`
/// (equal when they do not fit the arrangement). The last pane takes the
/// rounding remainder, so the panes cover the area exactly.
pub fn pane_tiles(count: usize, area: (f32, f32), layout: TileLayout, shares: &[f32]) -> Vec<Tile> {
    if layout == TileLayout::Grid {
        return grid_tiles(count, area, grid_split(shares));
    }
    if count == 0 || !is_linear(shares, count) {
        return tiles(count, area, layout);
    }
    let along = match layout {
        TileLayout::Stacked => area.1,
        _ => area.0,
    };
    let mut used = 0.0;
    let extents: Vec<f32> = shares
        .iter()
        .enumerate()
        .map(|(index, share)| {
            let extent = if index + 1 == count {
                along - used
            } else {
                (along * share).floor()
            };
            used += extent;
            extent
        })
        .collect();
    linear_tiles(&extents, area, layout == TileLayout::Stacked)
}

/// A splitter between panes: a vertical one is dragged left and right.
/// `at` is its line, `from` where it starts and `length` how far it runs,
/// all from the area's top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Splitter {
    pub index: usize,
    pub vertical: bool,
    pub at: f32,
    pub from: f32,
    pub length: f32,
}

/// The splitters between the panes of [`pane_tiles`]: one between each two
/// neighbours side by side or stacked; a grid of up to four has the column
/// line (index 0) and, with two rows, the row line (index 1).
pub fn splitters(
    count: usize,
    area: (f32, f32),
    layout: TileLayout,
    shares: &[f32],
) -> Vec<Splitter> {
    let placed = pane_tiles(count, area, layout, shares);
    match layout {
        TileLayout::Grid => grid_splitters(&placed, area),
        TileLayout::SideBySide | TileLayout::Stacked => {
            let vertical = layout == TileLayout::SideBySide;
            placed
                .iter()
                .take(count.saturating_sub(1))
                .enumerate()
                .map(|(index, tile)| Splitter {
                    index,
                    vertical,
                    at: if vertical {
                        tile.position.0 + tile.size.0
                    } else {
                        tile.position.1 + tile.size.1
                    },
                    from: 0.0,
                    length: if vertical { area.1 } else { area.0 },
                })
                .collect()
        }
    }
}

fn grid_splitters(placed: &[Tile], area: (f32, f32)) -> Vec<Splitter> {
    if placed.len() < 2 || placed.len() > MAX_PANES {
        return Vec::new();
    }
    let first = placed[0];
    // Three panes: the column line runs down the top row only.
    let column_length = if placed.len() == 3 {
        first.size.1
    } else {
        area.1
    };
    let mut lines = vec![Splitter {
        index: 0,
        vertical: true,
        at: first.position.0 + first.size.0,
        from: 0.0,
        length: column_length,
    }];
    if placed.len() > 2 {
        lines.push(Splitter {
            index: 1,
            vertical: false,
            at: first.position.1 + first.size.1,
            from: 0.0,
            length: area.0,
        });
    }
    lines
}

/// The shares after splitter `index` was dragged to `at` (pixels from the
/// area's start along the splitter's axis). Each pane keeps at least
/// [`MIN_PANE_EXTENT`] where the area allows. `None` for a splitter the
/// arrangement does not have.
pub fn dragged(
    count: usize,
    area: (f32, f32),
    layout: TileLayout,
    shares: &[f32],
    index: usize,
    at: f32,
) -> Option<Vec<f32>> {
    if !at.is_finite() || area.0 <= 0.0 || area.1 <= 0.0 {
        return None;
    }
    if layout == TileLayout::Grid {
        if count < 2 || count > MAX_PANES || index > 1 || (index == 1 && count < 3) {
            return None;
        }
        let (column, row) = grid_split(shares).unwrap_or((0.5, 0.5));
        let extent = if index == 0 { area.0 } else { area.1 };
        let share = clamp_share(at / extent, extent);
        return Some(if index == 0 {
            vec![share, row]
        } else {
            vec![column, share]
        });
    }
    if index + 1 >= count {
        return None;
    }
    let along = if layout == TileLayout::Stacked {
        area.1
    } else {
        area.0
    };
    let placed = pane_tiles(count, area, layout, shares);
    let extent = |tile: &Tile| {
        if layout == TileLayout::Stacked {
            tile.size.1
        } else {
            tile.size.0
        }
    };
    let start = |tile: &Tile| {
        if layout == TileLayout::Stacked {
            tile.position.1
        } else {
            tile.position.0
        }
    };
    let mut extents: Vec<f32> = placed.iter().map(extent).collect();
    let (left, right) = (placed[index], placed[index + 1]);
    let low = start(&left) + MIN_PANE_EXTENT;
    let high = start(&right) + extent(&right) - MIN_PANE_EXTENT;
    if low > high {
        return None;
    }
    let line = at.clamp(low, high);
    extents[index] = line - start(&left);
    extents[index + 1] = start(&right) + extent(&right) - line;
    Some(extents.iter().map(|e| e / along).collect())
}

/// `share` of an `extent`, kept [`MIN_PANE_EXTENT`] from either end where
/// the extent allows.
fn clamp_share(share: f32, extent: f32) -> f32 {
    let margin = (MIN_PANE_EXTENT / extent).min(0.45);
    share.clamp(margin, 1.0 - margin)
}

/// Tile Grid's panes for the docked tabs of `kinds` (in strip order), as
/// lists of their positions: one tab per pane up to [`MAX_PANES`]; with
/// more, one kind per pane where it can. Tabs are grouped by kind in the
/// order the kinds first appear, the largest group is halved while there
/// are fewer than four panes, and the last groups merge while there are
/// more.
pub fn grid_groups<T: PartialEq>(kinds: &[T]) -> Vec<Vec<usize>> {
    if kinds.len() <= MAX_PANES {
        return (0..kinds.len()).map(|index| vec![index]).collect();
    }
    let mut seen: Vec<&T> = Vec::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, kind) in kinds.iter().enumerate() {
        match seen.iter().position(|known| *known == kind) {
            Some(group) => groups[group].push(index),
            None => {
                seen.push(kind);
                groups.push(vec![index]);
            }
        }
    }
    while groups.len() > MAX_PANES {
        if let Some(last) = groups.pop()
            && let Some(before) = groups.last_mut()
        {
            before.extend(last);
            before.sort_unstable();
        }
    }
    while groups.len() < MAX_PANES {
        let largest = groups
            .iter()
            .enumerate()
            .max_by_key(|(at, group)| (group.len(), std::cmp::Reverse(*at)))
            .map(|(at, _)| at);
        let Some(at) = largest.filter(|at| groups[*at].len() >= 2) else {
            break;
        };
        let half = groups[at].len().div_ceil(2);
        let tail = groups[at].split_off(half);
        groups.insert(at + 1, tail);
    }
    groups
}
