//! What the board's Measure tool and net highlight hold (#2403).

use std::path::{Path, PathBuf};

use pcb_geom::{ItemRef, Span};
use signex_types::pcb::Point;

/// The board Measure tool and the highlighted board net.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PcbQueryState {
    /// Measure, armed on one board in one window.
    pub measure: Option<BoardMeasure>,
    /// The net the board shows bright.
    pub net_highlight: Option<BoardNetHighlight>,
}

/// Measure armed on a board: the first click once it has one, and the last
/// measurement until the next first click.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardMeasure {
    pub path: PathBuf,
    /// The undocked window it was armed in (`None`: the main window).
    pub window: Option<iced::window::Id>,
    pub first: Option<MeasureMark>,
    /// Where the pointer is after the first click, snapped: the ruler's
    /// other end until the second click.
    pub cursor: Option<Point>,
    pub result: Option<BoardMeasurement>,
}

impl BoardMeasure {
    /// Whether the tool belongs to the board at `path` shown in `window`.
    pub fn is_for(&self, path: Option<&Path>, window: Option<iced::window::Id>) -> bool {
        path == Some(self.path.as_path()) && window == self.window
    }
}

/// One Measure click: the snapped point, and the object under it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureMark {
    pub at: Point,
    pub item: Option<ItemRef>,
}

/// What two Measure clicks read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardMeasurement {
    /// From the first point to the second.
    pub span: Span,
    /// Edge to edge, when both clicks landed on an object.
    pub gap: Option<Span>,
}

/// A board net drawn bright.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardNetHighlight {
    pub path: PathBuf,
    pub net: u32,
    pub name: String,
}
