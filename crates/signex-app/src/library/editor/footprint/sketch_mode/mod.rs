//! Sketch-mode tooling shared by the footprint editor and the board sketch
//! (#2518): the Active Bar, its Create / Modify menus, the role picker's
//! labels, the way out of the mode and the edit messages. Which host the
//! editor serves is decided once, by [`FootprintEditorState::sketch_host`].
//!
//! [`FootprintEditorState::sketch_host`]: crate::library::editor::footprint::state::FootprintEditorState::sketch_host

pub mod active_bar;
pub mod constraint_menu;
pub mod exit;
pub mod linetype;
pub mod menus;
pub mod messages;
pub mod roles;
pub mod value_edit;

pub use exit::{BakeOutcome, BakePreview, ExitOutcome, SketchExit};
pub use linetype::SketchLinetype;
pub use messages::{ActiveTool, SketchEdit, SketchModeMsg, ToolEvent};
pub use value_edit::SketchCanvasMsg;

use signex_types::sketch::SketchHost;

use crate::library::editor::footprint::state::FootprintEditorState;

impl FootprintEditorState {
    /// The document this editor sketches for. A board sketch carries the
    /// board drawn under it; nothing else does. Every "am I on a board"
    /// decision asks this, never the backdrop itself.
    pub fn sketch_host(&self) -> SketchHost {
        if self.board_backdrop.is_some() {
            SketchHost::Board
        } else {
            SketchHost::Footprint
        }
    }
}
