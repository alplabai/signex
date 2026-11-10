//! Window ▸ Tile Horizontally / Tile Vertically (#2247): the main window and
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

/// How Tile arranges the windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileLayout {
    /// Tile Vertically: one column per window, left to right.
    SideBySide,
    /// Tile Horizontally: one row per window, top to bottom (the Windows
    /// MDI convention, Caner 2026-11-10).
    Stacked,
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
    let (width, height) = area;
    let along = match layout {
        TileLayout::SideBySide => width,
        TileLayout::Stacked => height,
    };
    let step = (along / count as f32).floor();
    (0..count)
        .map(|index| {
            let start = step * index as f32;
            let extent = if index + 1 == count {
                along - start
            } else {
                step
            };
            match layout {
                TileLayout::SideBySide => Tile {
                    position: (start, 0.0),
                    size: (extent, height),
                },
                TileLayout::Stacked => Tile {
                    position: (0.0, start),
                    size: (width, extent),
                },
            }
        })
        .collect()
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
}
