//! #1784 - Revert changes puts every cell edited since the Parameter
//! Manager opened back, as one undo step.

use signex_app::app::{EditMsg, Message, ParameterManagerMsg, Signex};

use crate::dirty_paths_gateway::{app_with, symbol};

fn tolerance(app: &Signex, id: uuid::Uuid) -> Option<String> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == id)
        .and_then(|s| s.fields.get("Tolerance").cloned())
}

fn typed(app: &mut Signex, id: uuid::Uuid, text: &str) {
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        sheet: "dirty-gateway.snxsch".into(),
        symbol_uuid: id,
        key: "Tolerance".into(),
        value: text.into(),
    }));
    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ));
}

#[test]
fn revert_changes_is_one_undo_step_over_the_session_edits() {
    let (r1, r2) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let (mut app, _path) = app_with(vec![symbol(r1, "R1", 0.0), symbol(r2, "R2", 10.0)]);
    // Before the dialog opens: not part of its session.
    typed(&mut app, r1, "5%");

    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Open));
    typed(&mut app, r1, "1%");
    typed(&mut app, r2, "10%");

    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::SessionReverted,
    ));
    assert_eq!(tolerance(&app, r1).as_deref(), Some("5%"));
    assert_eq!(
        tolerance(&app, r2),
        None,
        "a parameter the session added is gone again"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(tolerance(&app, r1).as_deref(), Some("1%"));
    assert_eq!(
        tolerance(&app, r2).as_deref(),
        Some("10%"),
        "one Undo takes the whole revert back"
    );
}
