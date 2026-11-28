//! The document area's panes (mockup row 207, #1307), namespaced under
//! `Message::Window(WindowMsg::Pane(..))` and routed to
//! `dispatch_pane_message`.

use crate::document_panes::DropEdge;
use crate::keymap::AppCommandId;

#[derive(Debug, Clone)]
pub enum PaneMsg {
    /// View ▸ Window ▸ Split Right / Split Down / Open Second View: the
    /// active document moves into a new pane on `edge`, or with
    /// `second_view` opens there as well.
    SplitRequested { edge: DropEdge, second_view: bool },
    /// The kept panes' `prefs.json` write finished; an error goes to Messages.
    Kept(Result<(), String>),
    /// A tab on a pane's own strip was pressed: the pane takes the focus and
    /// shows it. `pane` is its place among the panes, `tab` the tab index.
    TabPressed { pane: usize, tab: usize },
    /// A pane's ⋯ button was pressed: its menu opens, or closes when open.
    MenuToggled(usize),
    /// The pane menu was dismissed by a press outside it.
    MenuDismissed,
    /// A row of pane `pane`'s menu was picked: the pane takes the focus and
    /// the command runs, as from the palette.
    MenuCommandPicked { pane: usize, command: AppCommandId },
    /// Move Tab to Other Pane (`move_tab_to_other_pane`).
    MoveTabRequested,
    /// Close Pane (`close_pane`): the focused pane's tabs join the next one.
    CloseRequested,
    /// Sync Zoom with Other Pane (`sync_pane_zoom`).
    SyncZoomRequested,
    /// Equal Pane Sizes (`equalize_panes`), also a double click on a
    /// splitter.
    EqualizeRequested,
    /// A splitter between panes was pressed: dragging it resizes them.
    SplitterPressed(usize),
    /// A dragged document tab was let go in this window over a widget that
    /// took the release (the canvas): over the document area the drag ends
    /// there, on a pane edge with a split.
    TabReleased(iced::window::Id),
}

impl PaneMsg {
    /// The message the pane command `id` sends from a menu row, a key or
    /// the palette; `None` for any other command.
    pub fn for_command(id: &str) -> Option<Self> {
        let split = |edge, second_view| Self::SplitRequested { edge, second_view };
        Some(match id {
            "split_pane_right" => split(DropEdge::Right, false),
            "split_pane_down" => split(DropEdge::Bottom, false),
            "open_second_view" => split(DropEdge::Right, true),
            "move_tab_to_other_pane" => Self::MoveTabRequested,
            "close_pane" => Self::CloseRequested,
            "sync_pane_zoom" => Self::SyncZoomRequested,
            "equalize_panes" => Self::EqualizeRequested,
            _ => return None,
        })
    }
}
