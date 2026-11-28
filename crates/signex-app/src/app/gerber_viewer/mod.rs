//! The Gerber Viewer document tab (#2539, mockup rows 193 and 193f) over the
//! UI-free `signex-gerber` reader: the layer list with its order, colours
//! and eyes, the board view, the read-only Properties of the selected
//! object, the toolbar (Open, Flip, Drills, mm / mil, Highlight net) and the
//! empty state with its recent packages. Read only: nothing here writes a
//! file. Rules: `docs/GERBER_VIEWER.md`, "The viewer tab".
//!
//! Open paths this round: a double-click on a `.gbr`, `.drl` or `.zip` row
//! in Projects, files dropped on a window, the tab's own Open menu (Ctrl+O),
//! a recent package and the catalog command `open_gerber_viewer`
//! (`command.rs`). Every read runs in a task (`read.rs`). The states of
//! mockup row 193 (a pad or a plane selected, flipped, a net highlighted,
//! the Open menu) are set by catalog commands too (`view_commands.rs`).

mod canvas;
mod command;
mod from_run;
mod read;
mod recent;
mod state;
mod tab;
mod update;
mod view;
mod view_commands;
mod view_update;

#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_command_tests;

use std::path::PathBuf;
use std::sync::Arc;

use pcb_geom::PointNm;

pub(crate) use command::open_gerber_viewer;
pub use from_run::RunOrigin;
pub use read::{PackageRead, ReadFailure};
pub use recent::{RecentPackage, RecentPackages};
pub use state::{
    Camera, GerberViewerState, GripDrag, LayerLook, LengthUnit, ObjectRef, OpenPackage,
};
pub(crate) use view_commands::{
    flip_gerber_view, highlight_gerber_net, select_gerber_object, show_gerber_drills_on_top,
    show_gerber_open_menu,
};

/// Whether a file named `file_name` opens in the viewer from elsewhere: a
/// Projects row (mockup row 193) or a drop on another tab.
pub fn opens_by_name(file_name: &str) -> bool {
    read::is_fabrication_name(std::path::Path::new(file_name))
}

impl From<GerberViewerMsg> for crate::app::Message {
    fn from(msg: GerberViewerMsg) -> Self {
        Self::GerberViewer(msg)
    }
}

/// Which picker an Open action asks with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    Folder,
    Zip,
    Files,
}

/// What happened on the Gerber Viewer tab, or to files dropped on a window.
/// Namespaced under `Message::GerberViewer`, routed to
/// `dispatch_gerber_viewer_message`.
#[derive(Debug, Clone)]
pub enum GerberViewerMsg {
    /// `open_gerber_viewer` without a path (the palette, a key): the tab
    /// opens, or is focused, showing what it holds.
    ViewerRequested,
    /// The Open button or Ctrl+O on the tab.
    OpenMenuToggled,
    /// Open folder…, Open zip package… or Open files… was pressed.
    OpenPressed(OpenKind),
    /// The picker closed (`None`: cancelled).
    PathsPicked(Option<Vec<PathBuf>>),
    /// Paths to open from elsewhere in the app (a Projects row).
    PathsOpened(Vec<PathBuf>),
    /// A recent package row was pressed.
    RecentPressed(usize),
    /// Which recent packages are still there, by their paths.
    RecentChecked {
        found: Vec<(Vec<PathBuf>, bool)>,
        /// The clock when the check ran (`None`: no clock was read).
        now: Option<i64>,
    },
    /// The check of the recent packages could not run, and why.
    RecentCheckFailed(String),
    /// Read `job` of `paths` finished. `waiting` is the `open_gerber_viewer`
    /// call answered once it lands (#2896); `None` for a person's open.
    PackageRead {
        job: u64,
        paths: Vec<PathBuf>,
        result: Result<Arc<PackageRead>, ReadFailure>,
        waiting: Option<crate::app::agent::WaitingCommand>,
    },
    /// A file is dragged over a window.
    FileHovered(PathBuf),
    /// The drag left the window without a drop.
    FilesHoverLeft,
    /// A file landed on `window`.
    FileDropped {
        window: iced::window::Id,
        path: PathBuf,
    },
    /// The drops `window` reported with no hover before them are in.
    DropSettled {
        window: iced::window::Id,
    },
    /// A layer row's eye was pressed, by layer index.
    LayerVisibilityToggled(usize),
    /// A layer row's name was pressed.
    LayerPicked(usize),
    /// A layer's swatch was pressed: its colour row opens or closes.
    SwatchPressed(usize),
    ColourPicked {
        layer: usize,
        colour: [u8; 3],
    },
    /// A row's grip was pressed, by its place in the drawing order.
    GripPressed(usize),
    /// The pointer came over a row, by its place in the drawing order.
    RowEntered(usize),
    /// The button was let go anywhere on the tab.
    GripReleased,
    /// Esc on the tab: it closes the Open menu, else lets go of a held
    /// row, else clears the selection.
    EscapePressed,
    /// `select_gerber_object` without operands: nothing is selected.
    SelectionCleared,
    FlipToggled,
    DrillsToggled,
    UnitPicked(LengthUnit),
    NetHighlightToggled,
    /// The board view was zoomed or panned.
    ViewChanged(Camera),
    /// A click on the board, in the file frame, with how far from an
    /// object it may land.
    BoardClicked {
        at: PointNm,
        tolerance: i64,
    },
    /// The Export Job run banner's close (mockup row 193b).
    RunBannerClosed,
    /// The run banner's Back to run summary.
    BackToRunPressed,
}
