//! #1783 - the Parameter Manager table: clearing a cell keeps the
//! parameter, a typed cell is one undo step, deleting is explicit.

use signex_app::app::{EditMsg, Message, ParameterManagerMsg, Signex};

use crate::dirty_paths_gateway::{app_with, symbol};

fn fields(app: &Signex) -> std::collections::HashMap<String, String> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .fields
        .clone()
}

fn drafted(app: &mut Signex, id: uuid::Uuid, key: &str, text: &str) {
    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::CellDrafted {
            symbol_uuid: id,
            key: key.into(),
            text: text.into(),
        },
    ));
}

fn seeded() -> (Signex, uuid::Uuid) {
    let id = uuid::Uuid::now_v7();
    let (mut app, _path) = app_with(vec![symbol(id, "R1", 0.0)]);
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: id,
        key: "Tolerance".into(),
        value: "5%".into(),
    }));
    (app, id)
}

#[test]
fn clearing_a_cell_keeps_the_parameter_with_an_empty_value() {
    let (mut app, id) = seeded();

    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: id,
        key: "Tolerance".into(),
        value: String::new(),
    }));

    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some(""),
        "an emptied cell must stay a parameter, not vanish with its column"
    );
}

#[test]
fn typing_several_characters_then_enter_is_one_undo_step() {
    let (mut app, id) = seeded();

    for text in ["1", "1%", "10", "10%"] {
        drafted(&mut app, id, "Tolerance", text);
    }
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("5%"),
        "nothing is written while the cell is still being typed in"
    );
    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ));
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("10%")
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("5%"),
        "one Undo must restore the value from before the whole edit"
    );
}

#[test]
fn moving_to_another_cell_commits_the_previous_draft() {
    let (mut app, id) = seeded();
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: id,
        key: "Rating".into(),
        value: "0.1W".into(),
    }));

    drafted(&mut app, id, "Tolerance", "2%");
    drafted(&mut app, id, "Rating", "0.25W");

    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("2%")
    );
}

#[test]
fn the_explicit_delete_removes_the_parameter_and_is_undoable() {
    let (mut app, _id) = seeded();

    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::ParameterRemoved {
            key: "Tolerance".into(),
        },
    ));
    assert!(!fields(&app).contains_key("Tolerance"));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("5%")
    );
}

#[test]
fn a_new_column_lands_empty_on_every_symbol_in_one_undo_step() {
    let a = uuid::Uuid::now_v7();
    let b = uuid::Uuid::now_v7();
    let (mut app, _path) = app_with(vec![symbol(a, "R1", 0.0), symbol(b, "R2", 10.0)]);

    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::NewKeyChanged("MPN".into()),
    ));
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::ColumnAdded));
    let has_mpn = |app: &Signex| {
        app.document_state
            .active_engine()
            .unwrap()
            .document()
            .symbols
            .iter()
            .filter(|s| s.fields.get("MPN").is_some_and(String::is_empty))
            .count()
    };
    assert_eq!(has_mpn(&app), 2);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(has_mpn(&app), 0);
}
