//! View ▸ Window ▸ Tile Horizontally / Tile Vertically (#2247): the main window and
//! every undocked document window, arranged side by side or stacked on the
//! main window's monitor.
//!
//! iced reports a window's position relative to its monitor and the monitor's
//! full logical size, not its work area, so the tiles cover the whole monitor;
//! the OS keeps windows clear of its own taskbar. The geometry comes from the
//! last settled readings (`window_geometry_events`); nothing is read here.

use iced::Task;

use crate::app::state::WindowKind;
use crate::app::{Message, Signex};

/// How Tile arranges the windows and the main window's document panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileLayout {
    /// Tile Vertically: one column per window or pane, left to right.
    SideBySide,
    /// Tile Horizontally: one row per window or pane, top to bottom
    /// (Caner 2026-11-10).
    Stacked,
    /// Tile Grid (mockup row 207): rows and columns, filled row by row.
    Grid,
}

/// One window's tile: position relative to the monitor, and size, both in
/// logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    pub position: (f32, f32),
    pub size: (f32, f32),
}

/// `count` equal tiles filling `area` in `layout`. The last tile takes the
/// rounding remainder, so the tiles cover the area exactly.
#[must_use]
pub fn tiles(count: usize, area: (f32, f32), layout: TileLayout) -> Vec<Tile> {
    if count == 0 {
        return Vec::new();
    }
    let along = match layout {
        TileLayout::SideBySide => area.0,
        TileLayout::Stacked => area.1,
        TileLayout::Grid => return grid_tiles(count, area, None),
    };
    let step = (along / count as f32).floor();
    let extents: Vec<f32> = (0..count)
        .map(|index| {
            if index + 1 == count {
                along - step * index as f32
            } else {
                step
            }
        })
        .collect();
    linear_tiles(&extents, area, layout == TileLayout::Stacked)
}

/// Tiles of `extents` one after the other along the width of `area`, or
/// along its height when `stacked`.
#[must_use]
pub fn linear_tiles(extents: &[f32], area: (f32, f32), stacked: bool) -> Vec<Tile> {
    let mut start = 0.0;
    extents
        .iter()
        .map(|&extent| {
            let tile = if stacked {
                Tile {
                    position: (0.0, start),
                    size: (area.0, extent),
                }
            } else {
                Tile {
                    position: (start, 0.0),
                    size: (extent, area.1),
                }
            };
            start += extent;
            tile
        })
        .collect()
}

/// `count` tiles in a grid over `area`: as many columns as the square root
/// of `count` rounds up to, filled row by row; the last tile stretches to
/// the right edge of a short last row. `split` is the share of the first
/// column and of the first row when there are two; `None` makes them equal.
#[must_use]
pub fn grid_tiles(count: usize, area: (f32, f32), split: Option<(f32, f32)>) -> Vec<Tile> {
    if count == 0 {
        return Vec::new();
    }
    let columns = (count as f32).sqrt().ceil().max(1.0) as usize;
    let rows = count.div_ceil(columns);
    let xs = cuts(area.0, columns, split.map(|s| s.0));
    let ys = cuts(area.1, rows, split.map(|s| s.1));
    (0..count)
        .map(|index| {
            let (row, column) = (index / columns, index % columns);
            let right = if index + 1 == count {
                area.0
            } else {
                xs[column + 1]
            };
            Tile {
                position: (xs[column], ys[row]),
                size: (right - xs[column], ys[row + 1] - ys[row]),
            }
        })
        .collect()
}

/// The `parts + 1` cut positions along `extent`: equal whole-pixel steps,
/// the last part taking the remainder. With two parts and a `first` share,
/// the first part is that share of the extent.
fn cuts(extent: f32, parts: usize, first: Option<f32>) -> Vec<f32> {
    let step = (extent / parts as f32).floor();
    let mut at: Vec<f32> = (0..parts).map(|index| step * index as f32).collect();
    if parts == 2
        && let Some(share) = first.filter(|s| s.is_finite() && *s > 0.0 && *s < 1.0)
    {
        at[1] = (extent * share).floor();
    }
    at.push(extent);
    at
}

/// Where each window goes and on which monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct TilePlan {
    pub monitor_index: usize,
    /// The monitor's logical size, which the tiles fill.
    pub area: (f32, f32),
    /// The main window first, then the undocked document windows by path.
    pub windows: Vec<(iced::window::Id, Tile)>,
}

impl Signex {
    /// The windows Tile arranges: the main window, then every undocked
    /// document window in path order (stable whatever order they opened in).
    fn tiled_windows(&self) -> Vec<iced::window::Id> {
        let mut undocked: Vec<(&std::path::Path, iced::window::Id)> = self
            .ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match kind {
                WindowKind::UndockedTab { path, .. } => Some((path.as_path(), *id)),
                _ => None,
            })
            .collect();
        undocked.sort_by(|a, b| a.0.cmp(b.0));
        self.ui_state
            .main_window_id
            .into_iter()
            .chain(undocked.into_iter().map(|(_, id)| id))
            .collect()
    }

    /// The plan for `layout`, or `None` when the main window's monitor has not
    /// been read yet or there is no undocked window to tile with it.
    #[must_use]
    pub fn tile_plan(&self, layout: TileLayout) -> Option<TilePlan> {
        let main = self.ui_state.main_window_id?;
        let (monitor_index, area) = self.ui_state.window_geometry.monitor_of(main)?;
        let windows = self.tiled_windows();
        if windows.len() < 2 {
            return None;
        }
        let placed = tiles(windows.len(), area, layout);
        Some(TilePlan {
            monitor_index,
            area,
            windows: windows.into_iter().zip(placed).collect(),
        })
    }

    /// Run Tile: un-maximize, resize and move every window of the plan.
    pub(crate) fn tile_windows(&mut self, layout: TileLayout) -> Task<Message> {
        let Some(plan) = self.tile_plan(layout) else {
            crate::diagnostics::log_warning(
                "Tile: needs an undocked document window and a main window whose screen is \
                 known; nothing was moved.",
            );
            return Task::none();
        };
        let (width, height) = plan.area;
        crate::diagnostics::log_info(format!(
            "Tile: arranged {} windows {} on the {width} x {height} screen",
            plan.windows.len(),
            match layout {
                TileLayout::SideBySide => "side by side",
                TileLayout::Stacked => "one above the other",
                TileLayout::Grid => "in a grid",
            }
        ));
        let monitor = Some(iced::window::MonitorIndex(plan.monitor_index));
        Task::batch(plan.windows.into_iter().flat_map(|(id, tile)| {
            [
                iced::window::maximize(id, false),
                iced::window::resize(id, iced::Size::new(tile.size.0, tile.size.1)),
                iced::window::move_to(
                    id,
                    iced::window::PositionOnMonitor {
                        monitor_index: monitor,
                        position: iced::Point::new(tile.position.0, tile.position.1),
                    },
                ),
            ]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_splits_the_width_and_covers_it_exactly() {
        let placed = tiles(3, (1000.0, 600.0), TileLayout::SideBySide);
        assert_eq!(
            placed,
            vec![
                Tile {
                    position: (0.0, 0.0),
                    size: (333.0, 600.0)
                },
                Tile {
                    position: (333.0, 0.0),
                    size: (333.0, 600.0)
                },
                Tile {
                    position: (666.0, 0.0),
                    size: (334.0, 600.0)
                },
            ]
        );
    }

    #[test]
    fn stacked_splits_the_height() {
        let placed = tiles(2, (1920.0, 1080.0), TileLayout::Stacked);
        assert_eq!(placed[0].size, (1920.0, 540.0));
        assert_eq!(placed[1].position, (0.0, 540.0));
        assert!(tiles(0, (10.0, 10.0), TileLayout::Stacked).is_empty());
    }

    /// Mockup row 207, Tile Grid: four panes in two rows of two, three with
    /// the last one across the bottom row, two side by side.
    #[test]
    fn a_grid_fills_rows_of_two_and_the_last_tile_stretches() {
        let at = |tile: &Tile| (tile.position, tile.size);
        let four = tiles(4, (1000.0, 600.0), TileLayout::Grid);
        assert_eq!(
            four.iter().map(at).collect::<Vec<_>>(),
            [
                ((0.0, 0.0), (500.0, 300.0)),
                ((500.0, 0.0), (500.0, 300.0)),
                ((0.0, 300.0), (500.0, 300.0)),
                ((500.0, 300.0), (500.0, 300.0)),
            ]
        );
        let three = tiles(3, (1000.0, 600.0), TileLayout::Grid);
        assert_eq!(at(&three[2]), ((0.0, 300.0), (1000.0, 300.0)));
        let two = tiles(2, (1001.0, 600.0), TileLayout::Grid);
        assert_eq!(at(&two[1]), ((500.0, 0.0), (501.0, 600.0)));
        assert!(tiles(0, (10.0, 10.0), TileLayout::Grid).is_empty());
    }

    /// The first column and row take their share of a dragged grid.
    #[test]
    fn a_grid_split_moves_the_first_column_and_row() {
        let placed = grid_tiles(4, (1000.0, 600.0), Some((0.3, 0.75)));
        assert_eq!(placed[0].size, (300.0, 450.0));
        assert_eq!(placed[3].position, (300.0, 450.0));
        assert_eq!(placed[3].size, (700.0, 150.0));
    }
}
