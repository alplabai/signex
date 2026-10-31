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

fn typed(app: &mut Signex, id: uuid::Uuid, key: &str, text: &str) {
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: id,
        key: key.into(),
        value: text.into(),
    }));
}

fn id_of(app: &Signex) -> uuid::Uuid {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols[0]
        .uuid
}

fn seeded() -> (Signex, uuid::Uuid) {
    let id = uuid::Uuid::now_v7();
    let (mut app, _path) = app_with(vec![symbol(id, "R1", 0.0)]);
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: id,
        key: "Tolerance".into(),
        value: "5%".into(),
    }));
    // Enter ends the seeding edit's typing run.
    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ));
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
fn typing_several_characters_is_one_undo_step_and_is_in_the_document_at_once() {
    let (mut app, _id) = seeded();
    let path = std::path::PathBuf::from("dirty-gateway.snxsch");

    for text in ["1", "1%", "10", "10%"] {
        let id = id_of(&app);
        typed(&mut app, id, "Tolerance", text);
    }
    // No Enter yet: the value is already in the document, so Save, Save All
    // and quit see it and the sheet is dirty.
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("10%")
    );
    assert!(app.document_state.is_document_dirty(&path));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("5%"),
        "one Undo must restore the value from before the whole edit"
    );
}

#[test]
fn enter_or_another_cell_starts_a_new_undo_step() {
    let (mut app, id) = seeded();
    typed(&mut app, id, "Rating", "0.1W");

    typed(&mut app, id, "Tolerance", "1%");
    typed(&mut app, id, "Tolerance", "12%");
    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::CellSubmitted,
    ));
    typed(&mut app, id, "Tolerance", "2%");
    typed(&mut app, id, "Rating", "0.25W");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(fields(&app).get("Rating").map(String::as_str), Some("0.1W"));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("12%")
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        fields(&app).get("Tolerance").map(String::as_str),
        Some("5%")
    );
}

#[test]
fn an_edit_naming_a_symbol_the_sheet_lacks_changes_nothing() {
    let (mut app, _id) = seeded();
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let before = fields(&app);

    typed(&mut app, uuid::Uuid::now_v7(), "Tolerance", "99%");

    assert_eq!(
        fields(&app),
        before,
        "a foreign symbol must not land on this sheet"
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

#[test]
fn deleting_a_column_leaves_power_symbols_alone() {
    let part = uuid::Uuid::now_v7();
    let power = uuid::Uuid::now_v7();
    let mut pwr = symbol(power, "#PWR01", 10.0);
    pwr.is_power = true;
    pwr.fields.insert("Tolerance".into(), "keep".into());
    let (mut app, _path) = app_with(vec![symbol(part, "R1", 0.0), pwr]);
    typed(&mut app, part, "Tolerance", "5%");

    let _ = app.update(Message::ParameterManager(
        ParameterManagerMsg::ParameterRemoved {
            key: "Tolerance".into(),
        },
    ));

    let doc = app.document_state.active_engine().unwrap().document();
    assert!(!doc.symbols[0].fields.contains_key("Tolerance"));
    assert_eq!(
        doc.symbols[1].fields.get("Tolerance").map(String::as_str),
        Some("keep"),
        "the table hides power symbols, so a delete must not reach them"
    );
}
