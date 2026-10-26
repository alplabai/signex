//! #1080 — the sheet's title block is edited from the Properties panel: a
//! field commits on Enter as one undoable edit, a draft never reaches the
//! document, and a draft typed on one sheet is never written into another.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, TitleBlockField};
use signex_types::schematic::SchematicSheet;
use std::path::PathBuf;

fn open_sheet(app: &mut Signex, name: &str) -> PathBuf {
    let path = PathBuf::from(format!("{name}.snxsch"));
    let engine = signex_engine::Engine::new(SchematicSheet::default()).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: name.to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    path
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn type_field(app: &mut Signex, field: TitleBlockField, text: &str) {
    panel(app, PanelMsg::TitleBlockTyping(field, text.to_string()));
}

fn stored(app: &Signex, path: &PathBuf, key: &str) -> Option<String> {
    app.document_state.engines[path]
        .document()
        .title_block
        .get(key)
        .cloned()
}

#[test]
fn a_field_commits_on_enter_as_one_undo_step() {
    let (mut app, _t) = Signex::new();
    let sheet = open_sheet(&mut app, "power");

    type_field(&mut app, TitleBlockField::DrawnBy, "A. Eng");
    type_field(&mut app, TitleBlockField::DrawnBy, "  A. Engineer ");
    assert_eq!(
        stored(&app, &sheet, "drawn_by"),
        None,
        "a draft is not saved"
    );

    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::DrawnBy),
    );
    assert_eq!(
        stored(&app, &sheet, "drawn_by").as_deref(),
        Some("A. Engineer")
    );
    assert_eq!(
        app.document_state
            .panel_ctx
            .title_block
            .get("drawn_by")
            .map(String::as_str),
        Some("A. Engineer"),
        "the panel shows the stored value"
    );
    assert!(app.document_state.panel_ctx.title_block_drafts.is_empty());
    assert!(
        app.document_state.tabs[0].dirty,
        "a commit dirties the sheet"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(stored(&app, &sheet, "drawn_by"), None, "one undo step");
}

#[test]
fn an_empty_field_clears_the_value() {
    let (mut app, _t) = Signex::new();
    let sheet = open_sheet(&mut app, "power");
    type_field(&mut app, TitleBlockField::Title, "Power supply");
    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Title),
    );
    type_field(&mut app, TitleBlockField::Title, "   ");
    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Title),
    );
    assert_eq!(stored(&app, &sheet, "title"), None);
}

#[test]
fn a_draft_typed_on_one_sheet_is_never_written_into_another() {
    let (mut app, _t) = Signex::new();
    let first = open_sheet(&mut app, "first");
    type_field(&mut app, TitleBlockField::Revision, "C");

    let second = open_sheet(&mut app, "second");
    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Revision),
    );
    assert_eq!(stored(&app, &second, "rev"), None);
    assert_eq!(stored(&app, &first, "rev"), None);

    // Back on the first sheet the draft is still there and commits there.
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(first.clone());
    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Revision),
    );
    assert_eq!(stored(&app, &first, "rev").as_deref(), Some("C"));
    assert_eq!(stored(&app, &second, "rev"), None);
}

/// Moving to another field before pressing Enter keeps the first field's
/// draft; each commits on its own Enter.
#[test]
fn typing_in_a_second_field_keeps_the_first_fields_draft() {
    let (mut app, _t) = Signex::new();
    let sheet = open_sheet(&mut app, "power");
    type_field(&mut app, TitleBlockField::Title, "Power supply");
    type_field(&mut app, TitleBlockField::Revision, "B");

    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Revision),
    );
    assert_eq!(stored(&app, &sheet, "rev").as_deref(), Some("B"));
    assert_eq!(stored(&app, &sheet, "title"), None, "still a draft");

    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Title),
    );
    assert_eq!(
        stored(&app, &sheet, "title").as_deref(),
        Some("Power supply")
    );
}

/// The document number and the sign-off values typed here appear in their
/// cells of the sheet's title block on the canvas (and so in the PDF, which
/// lowers the same table) (#1080, #1201).
#[test]
fn the_sign_off_fields_appear_in_the_drawn_title_block() {
    let (mut app, _t) = Signex::new();
    open_sheet(&mut app, "power");
    for (field, text) in [
        (TitleBlockField::DocumentNumber, "DOC-42"),
        (TitleBlockField::DrawnBy, "A. Engineer"),
        (TitleBlockField::CheckedBy, "B. Reviewer"),
        (TitleBlockField::ApprovedBy, "C. Lead"),
    ] {
        type_field(&mut app, field, text);
        panel(&mut app, PanelMsg::CommitTitleBlockField(field));
    }

    let drawn: Vec<&str> = app
        .interaction_state
        .active_canvas()
        .sheet_frame
        .iter()
        .filter_map(|p| match p {
            signex_output::template::FramePrimitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for wanted in [
        "Doc No",
        "DOC-42",
        "Drawn",
        "A. Engineer",
        "Checked",
        "B. Reviewer",
        "Approved",
        "C. Lead",
    ] {
        assert!(drawn.contains(&wanted), "{wanted} not in {drawn:?}");
    }
}
