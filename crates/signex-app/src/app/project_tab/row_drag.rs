//! Dragging a Sheets or Libraries row by its grip (mockup rows 31 and 33,
//! decided 2026-11-22). A drop is one General-history step; Move up and Move
//! down stay for the keyboard. Esc or a release on no other row changes
//! nothing.

use std::path::PathBuf;

use iced::Length;
use signex_engine::ProjectSectionCommand;
use signex_i18n::MessageId;

use super::general::tab_message;
use super::table::{RowGrip, TS};
use super::{ProjectTabMsg, ProjectTabState};
use crate::app::Signex;
use crate::app::state::ProjectId;

/// Width of the grip column (24 px).
pub(super) const GRIP_W: f32 = TS.body * 2.0;

/// The row being dragged, by what the project stores for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKey {
    Sheet(String),
    Library(PathBuf),
}

/// A row held by its grip, and the row the pointer is over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowDrag {
    key: RowKey,
    over: Option<usize>,
}

/// What happened to a row drag.
#[derive(Debug, Clone)]
pub enum RowDragMsg {
    /// A grip was pressed.
    Started(RowKey),
    /// The pointer entered the row at this index.
    Entered(usize),
    /// The button was let go: the row drops on the last row entered.
    Released,
    /// Esc: the drag ends and nothing moves.
    Cancelled,
}

/// The grip column of a grid table: no caption.
pub(super) fn grip_column() -> (Option<MessageId>, Length) {
    (None, Length::Fixed(GRIP_W))
}

/// The grip of the row at `index`.
pub(super) fn grip_of(
    project_id: ProjectId,
    key: RowKey,
    index: usize,
    drag: Option<&RowDrag>,
) -> RowGrip {
    let message = |msg| tab_message(ProjectTabMsg::RowDragged { project_id, msg });
    RowGrip {
        on_press: message(RowDragMsg::Started(key)),
        on_enter: message(RowDragMsg::Entered(index)),
        is_drop_target: drag.is_some_and(|drag| drag.over == Some(index)),
    }
}

/// The command a release makes of `drag`; `None` when it entered no row.
fn drop_command(drag: RowDrag) -> Option<ProjectSectionCommand> {
    let to = drag.over?;
    Some(match drag.key {
        RowKey::Sheet(filename) => ProjectSectionCommand::PlaceSheet { filename, to },
        RowKey::Library(path) => ProjectSectionCommand::PlaceLibrary { path, to },
    })
}

impl ProjectTabState {
    /// Whether a row is held by its grip.
    pub(super) fn is_dragging_row(&self) -> bool {
        self.row_drag.is_some()
    }
}

impl Signex {
    pub(super) fn act_on_row_drag(&mut self, project_id: ProjectId, msg: RowDragMsg) {
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return;
        };
        match msg {
            RowDragMsg::Started(key) => tab.row_drag = Some(RowDrag { key, over: None }),
            RowDragMsg::Entered(index) => {
                if let Some(drag) = tab.row_drag.as_mut() {
                    drag.over = Some(index);
                }
            }
            RowDragMsg::Cancelled => tab.row_drag = None,
            RowDragMsg::Released => {
                let Some(command) = tab.row_drag.take().and_then(drop_command) else {
                    return;
                };
                // A drop never refuses; on its own place it changes nothing.
                if let Err(problem) = self.apply_project_section(project_id, &command) {
                    crate::diagnostics::log_warning(format!("Project tab: {problem}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::menu::export::tests::app_workspace;
    use crate::app::{Message, ProjectMsg};

    fn app() -> (Signex, ProjectId) {
        let dir = std::env::temp_dir().join(format!("signex-prj-drag-{}", uuid::Uuid::now_v7()));
        let mut app = app_workspace(
            &dir.to_string_lossy(),
            &["a.snxsch", "b.snxsch", "c.snxsch"],
        );
        app.open_project_tab(0);
        let id = app.document_state.projects[0].id;
        (app, id)
    }

    fn send(app: &mut Signex, project_id: ProjectId, msg: RowDragMsg) {
        let _ = app.update(Message::Project(ProjectMsg::Tab(
            ProjectTabMsg::RowDragged { project_id, msg },
        )));
    }

    fn order(app: &Signex) -> Vec<String> {
        app.document_state.projects[0].data.sheet_order()
    }

    #[test]
    fn a_dropped_row_moves_as_one_undo_step() {
        let (mut app, id) = app();
        let before = order(&app);
        send(
            &mut app,
            id,
            RowDragMsg::Started(RowKey::Sheet("a.snxsch".into())),
        );
        send(&mut app, id, RowDragMsg::Entered(1));
        send(&mut app, id, RowDragMsg::Entered(2));
        send(&mut app, id, RowDragMsg::Released);
        assert_eq!(order(&app), ["b.snxsch", "c.snxsch", "a.snxsch"]);
        assert!(app.step_project_tab_history(id, true));
        assert_eq!(order(&app), before);
    }

    #[test]
    fn esc_and_a_press_without_a_move_change_nothing() {
        let (mut app, id) = app();
        let before = order(&app);
        let start = || RowDragMsg::Started(RowKey::Sheet("a.snxsch".into()));
        send(&mut app, id, start());
        send(&mut app, id, RowDragMsg::Entered(2));
        send(&mut app, id, RowDragMsg::Cancelled);
        send(&mut app, id, RowDragMsg::Released);
        assert_eq!(order(&app), before);
        send(&mut app, id, start());
        send(&mut app, id, RowDragMsg::Released);
        assert_eq!(order(&app), before);
        assert!(!app.document_state.project_tabs[&id].is_dragging_row());
    }
}
