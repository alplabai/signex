//! m127 library row Properties, m128 Component editor, m130 Edit row and
//! m139 prefix and unit pickers.

use signex_app::app::{Message, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{BrowserEditMsg, EditorMsg, LibraryMessage, ParamKindMsg};
use signex_app::library::state::{ComponentPreviewState, EditorAddress, PrimitivePickerTarget};
use signex_app::panels::PanelMsg;
use signex_library::{ParamValue, PrimitiveKind, RowId};
use signex_types::quantity::SiPrefix;

use super::support::{LibRig, RATING, TABLE, library_rig, part_number, read_only_rig};

fn panel(r: &mut LibRig, msg: PanelMsg) {
    let _ = r.app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn open_editor(r: &mut LibRig, row_id: RowId) {
    r.run(LibraryMessage::OpenComponentRow {
        library_path: r.root.clone(),
        table: TABLE.to_string(),
        row_id,
    });
}

fn event(r: &mut LibRig, row_id: RowId, msg: EditorMsg) {
    r.run(LibraryMessage::EditorEvent {
        library_path: r.root.clone(),
        table: TABLE.to_string(),
        row_id,
        msg,
    });
}

fn editor(r: &LibRig, row_id: RowId) -> &ComponentPreviewState {
    let address = EditorAddress::new(r.root.clone(), TABLE.to_string(), row_id);
    &r.app.library.editors[&address]
}

fn draft_rating(r: &LibRig) -> (f64, String) {
    match r.modal().draft.parameters.get(RATING) {
        Some(ParamValue::Measurement { value, unit }) => (*value, unit.clone()),
        other => panic!("the rating is {other:?}"),
    }
}

// ---- m127 ----

#[test]
fn selecting_a_row_fills_the_properties_sheet() {
    let mut r = library_rig(2);
    let id = r.rows[1];

    r.select(id);

    let detail = r
        .app
        .document_state
        .panel_ctx
        .library_row_detail
        .as_ref()
        .expect("Properties shows the row");
    assert_eq!(detail.internal_pn, part_number(1));
    assert_eq!(detail.mpn, "MPN-1");
    assert_eq!(detail.manufacturer, "Yageo");
    assert_eq!(detail.table, TABLE);
    assert_eq!(detail.row_id, id.as_uuid());
    assert_eq!(detail.position, Some(2));
}

#[test]
fn pick_symbol_on_the_sheet_opens_the_picker_for_that_row() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    r.select(id);

    panel(&mut r, PanelMsg::LibraryRowPickSymbol);

    let picker = r.app.library.primitive_picker.as_ref().expect("the picker");
    assert_eq!(picker.kind, PrimitiveKind::Symbol);
    assert!(matches!(
        &picker.target,
        PrimitivePickerTarget::BrowserRow(address) if address.row_id == id
    ));
}

#[test]
fn pick_on_the_sheet_of_a_read_only_library_is_refused() {
    let mut r = read_only_rig(1);
    let id = r.rows[0];
    r.select(id);

    panel(&mut r, PanelMsg::LibraryRowPickFootprint);

    assert!(r.app.library.primitive_picker.is_none());
    assert!(r.app.document_state.error_notice.is_some());
}

// ---- m128 ----

#[test]
fn saving_the_component_editor_writes_the_row_and_clears_unsaved() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    open_editor(&mut r, id);
    let before = r.files();

    event(
        &mut r,
        id,
        EditorMsg::SupplyPrimarySetMpn("RC0402FR-0710KL".to_string()),
    );
    assert!(editor(&r, id).dirty, "the edit shows as unsaved");
    assert_eq!(r.files(), before, "nothing is written before Save");

    event(&mut r, id, EditorMsg::SaveDraft);

    assert!(!editor(&r, id).dirty);
    assert_eq!(r.row(id).expect("row").primary_mpn.mpn, "RC0402FR-0710KL");
    assert_ne!(r.files(), before);
}

#[test]
fn reload_asks_first_and_keep_editing_keeps_the_edit() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    open_editor(&mut r, id);
    event(
        &mut r,
        id,
        EditorMsg::SupplyPrimarySetMpn("typed".to_string()),
    );

    event(&mut r, id, EditorMsg::ReloadRequested);
    assert!(editor(&r, id).reload_confirming);
    assert_eq!(editor(&r, id).row.primary_mpn.mpn, "typed");

    event(&mut r, id, EditorMsg::ReloadCancelled);
    assert!(!editor(&r, id).reload_confirming);
    assert_eq!(editor(&r, id).row.primary_mpn.mpn, "typed");
}

#[test]
fn reload_confirmed_puts_the_saved_row_back() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    open_editor(&mut r, id);
    event(
        &mut r,
        id,
        EditorMsg::SupplyPrimarySetMpn("typed".to_string()),
    );

    event(&mut r, id, EditorMsg::ReloadRequested);
    event(&mut r, id, EditorMsg::ReloadConfirmed);

    assert_eq!(editor(&r, id).row.primary_mpn.mpn, "MPN-0");
    assert!(!editor(&r, id).dirty);
}

#[test]
fn cancel_closes_the_editor_and_writes_nothing() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    open_editor(&mut r, id);
    let before = r.files();
    event(
        &mut r,
        id,
        EditorMsg::SupplyPrimarySetMpn("typed".to_string()),
    );

    event(&mut r, id, EditorMsg::CloseEditor);

    let open = r
        .app
        .document_state
        .tabs
        .iter()
        .any(|tab| matches!(tab.kind, TabKind::ComponentEditor(_)));
    assert!(!open, "the editor tab is closed");
    assert_eq!(r.row(id).expect("row").primary_mpn.mpn, "MPN-0");
    assert_eq!(r.files(), before);
}

#[test]
fn save_is_refused_while_a_number_field_does_not_parse() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    open_editor(&mut r, id);
    event(
        &mut r,
        id,
        EditorMsg::ParamNewNameChanged("Gain".to_string()),
    );
    event(
        &mut r,
        id,
        EditorMsg::ParamAddCustom {
            kind: ParamKindMsg::Number,
        },
    );
    event(
        &mut r,
        id,
        EditorMsg::ParamSetNumberBuf {
            name: "Gain".to_string(),
            buf: "abc".to_string(),
        },
    );
    let before = r.files();

    event(&mut r, id, EditorMsg::SaveDraft);

    assert_eq!(r.files(), before, "nothing is written");
    assert!(!r.row(id).expect("row").parameters.contains_key("Gain"));
    assert!(editor(&r, id).has_unsaved_edits(), "the edit is kept");
    assert!(r.app.document_state.error_notice.is_some());
}

// ---- m130 ----

#[test]
fn edit_row_save_writes_the_fields_and_closes() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    r.open_modal(id);

    r.edit(BrowserEditMsg::SetManufacturer("Vishay".to_string()));
    r.edit(BrowserEditMsg::SetMpn("CRCW040210K0".to_string()));
    r.edit(BrowserEditMsg::SetDescription("10 kΩ 1%".to_string()));
    r.edit(BrowserEditMsg::SetTags("thick film, 0402".to_string()));
    r.edit(BrowserEditMsg::Save);

    assert!(r.browser().edit_modal.is_none(), "a good save closes it");
    let row = r.row(id).expect("row");
    assert_eq!(row.primary_mpn.manufacturer, "Vishay");
    assert_eq!(row.primary_mpn.mpn, "CRCW040210K0");
    assert_eq!(row.description, "10 kΩ 1%");
    assert_eq!(
        row.parameters.get("tags"),
        Some(&ParamValue::Text("thick film, 0402".to_string()))
    );
}

#[test]
fn edit_row_cancel_leaves_the_row_and_files_unchanged() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let before = r.files();
    r.open_modal(id);
    r.edit(BrowserEditMsg::SetMpn("typed".to_string()));

    r.edit(BrowserEditMsg::Cancel);

    assert!(r.browser().edit_modal.is_none());
    assert_eq!(r.row(id).expect("row").primary_mpn.mpn, "MPN-0");
    assert_eq!(r.files(), before);
}

#[test]
fn edit_row_save_waits_while_another_row_holds_the_internal_pn() {
    let mut r = library_rig(2);
    let id = r.rows[1];
    let before = r.files();
    r.open_modal(id);

    r.edit(BrowserEditMsg::SetInternalPn(part_number(0)));
    r.edit(BrowserEditMsg::Save);

    assert!(r.browser().edit_modal.is_some(), "the modal stays open");
    assert_eq!(r.row(id).expect("row").internal_pn.as_str(), part_number(1));
    assert_eq!(r.files(), before);
}

#[test]
fn a_row_keeping_its_own_internal_pn_saves() {
    let mut r = library_rig(2);
    let id = r.rows[1];
    r.open_modal(id);

    r.edit(BrowserEditMsg::SetInternalPn(part_number(1)));
    r.edit(BrowserEditMsg::SetMpn("kept-pn".to_string()));
    r.edit(BrowserEditMsg::Save);

    assert!(r.browser().edit_modal.is_none());
    assert_eq!(r.row(id).expect("row").primary_mpn.mpn, "kept-pn");
}

#[test]
fn a_description_with_triple_quotes_is_refused_with_a_reason() {
    let mut r = library_rig(1);
    r.open_modal(r.rows[0]);

    r.edit(BrowserEditMsg::SetDescription("bad ''' text".to_string()));

    assert!(r.modal().error.is_some());
    assert_eq!(r.modal().draft.description, "");
}

#[test]
fn save_refuses_a_parameter_value_that_is_no_number() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    r.open_modal(id);
    r.edit(BrowserEditMsg::AddParam);
    r.edit(BrowserEditMsg::SetParamValue {
        key: "param_2".to_string(),
        value: "nan".to_string(),
    });
    let before = r.files();

    r.edit(BrowserEditMsg::Save);

    assert!(
        r.modal()
            .error
            .as_deref()
            .is_some_and(|e| e.contains("param_2")),
        "the reason names the parameter"
    );
    assert_eq!(r.files(), before);
}

#[test]
fn add_parameter_then_remove_leaves_the_draft_as_it_was() {
    let mut r = library_rig(1);
    r.open_modal(r.rows[0]);
    let before = r.modal().draft.parameters.clone();

    r.edit(BrowserEditMsg::AddParam);
    assert_eq!(r.modal().draft.parameters.len(), before.len() + 1);
    r.edit(BrowserEditMsg::DeleteParam {
        key: "param_2".to_string(),
    });

    assert_eq!(r.modal().draft.parameters, before);
}

// ---- m139 ----

#[test]
fn a_picked_prefix_rescales_the_measurement_in_the_draft() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    r.open_modal(id);
    assert_eq!(r.modal().param_prefix.get(RATING), Some(&SiPrefix::Kilo));

    r.edit(BrowserEditMsg::ParamPrefixPicked {
        key: RATING.to_string(),
        prefix: SiPrefix::Mega,
    });

    let (value, unit) = draft_rating(&r);
    assert!((value - 4.7e6).abs() < 1e-3, "4.7 MΩ, got {value}");
    assert_eq!(unit, "ohm");
    assert!(
        matches!(
            r.row(id).expect("row").parameters.get(RATING),
            Some(ParamValue::Measurement { value, .. }) if (*value - 4700.0).abs() < 1e-6
        ),
        "the stored row waits for Save"
    );
}

#[test]
fn a_picked_unit_keeps_the_number_and_prefix_shown() {
    let mut r = library_rig(1);
    r.open_modal(r.rows[0]);

    r.edit(BrowserEditMsg::SetParamUnit {
        key: RATING.to_string(),
        unit: "F".to_string(),
    });

    let (value, unit) = draft_rating(&r);
    assert_eq!(unit, "F");
    assert!((value - 4700.0).abs() < 1e-6, "4.7 k stays, got {value}");
}

#[test]
fn a_prefix_typed_into_the_number_is_refused_and_the_draft_is_kept() {
    let mut r = library_rig(1);
    r.open_modal(r.rows[0]);

    r.edit(BrowserEditMsg::SetParamValue {
        key: RATING.to_string(),
        value: "4k7".to_string(),
    });
    assert!(r.modal().error.is_some(), "refused as typed");
    r.edit(BrowserEditMsg::CommitParam {
        key: RATING.to_string(),
    });

    assert!(r.modal().error.is_some(), "still refused on Enter");
    let (value, _) = draft_rating(&r);
    assert!(
        (value - 4700.0).abs() < 1e-6,
        "the draft is kept, got {value}"
    );
}
