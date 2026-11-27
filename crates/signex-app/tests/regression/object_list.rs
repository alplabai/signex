//! #1259 - View ▸ Panels ▸ Object List lists the objects of the sheet in
//! view; clicking a row selects that object on the canvas, on its own sheet
//! when it is on another one.
//!
//! The project of `net_nav`: `a.snxsch` and `b.snxsch`, each a wire with the
//! global label `VCC`, both open with `a` focused.

use signex_app::app::{Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{ObjectListMsg, ObjectListStatus, ObjectRef, PanelKind, PanelMsg};
use signex_app::task_driver::TaskDriver;
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use tempfile::TempDir;
use uuid::Uuid;

use crate::net_nav::flows::open_project;

const COMMAND: &str = "toggle_schematic_list_panel";

/// Run `message` and what it leads to, dropping the dock layout save so the
/// shared config folder does not keep the panel open for the next test.
fn settle(app: &mut Signex, message: Message) {
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([app.update(message)]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for message in driver.outputs(task).expect("the task chain settled") {
            steps += 1;
            assert!(steps < 1_000, "the task chain settles");
            if !matches!(message, Message::DockLayout(_)) {
                queue.push_back(app.update(message));
            }
        }
    }
}

fn list(app: &mut Signex, msg: ObjectListMsg) {
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::ObjectList(msg))),
    );
}

#[test]
fn clicking_an_object_list_row_selects_that_object_on_the_canvas() {
    let tmp = TempDir::new().unwrap();
    let (mut app, a, b) = open_project(tmp.path());
    settle(&mut app, crate::menu_support::menu_row(COMMAND));
    assert!(
        app.document_view
            .dock
            .locate(PanelKind::ObjectList)
            .is_some()
    );

    let view = &app.document_view.panel_ctx.object_list;
    assert_eq!(view.status, ObjectListStatus::Ready);
    assert_eq!(
        view.counts.total(),
        2,
        "a wire and a label: {:?}",
        view.rows
    );
    let label = view
        .rows
        .iter()
        .find(|r| r.target.item.kind == SelectedKind::Label)
        .expect("the VCC label is listed")
        .target
        .clone();
    assert_eq!(label.sheet, a);

    list(&mut app, ObjectListMsg::RowClicked(label.clone()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![label.item]
    );
    let view = &app.document_view.panel_ctx.object_list;
    assert_eq!(view.selected_count, 1, "the row mirrors the selection");

    // A row of another sheet opens that sheet and selects there.
    let on_b = ObjectRef {
        sheet: b.clone(),
        item: SelectedItem::new(Uuid::from_u128(2), SelectedKind::Label),
        at: Point::new(0.0, 0.0),
    };
    list(&mut app, ObjectListMsg::RowClicked(on_b.clone()));
    assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![on_b.item]
    );
}
