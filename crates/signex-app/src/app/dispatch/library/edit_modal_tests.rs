//! #1500 — the Edit Component Details modal is reached from the browser
//! action row's "Edit Details" button (`BrowserOpenEditModal`), and an edit
//! applied there persists through the adapter's `update_row`.

use std::path::{Path, PathBuf};

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef, RowId,
};
use uuid::Uuid;

use crate::app::{Message, Signex};
use crate::library::messages::{BrowserEditMsg, LibraryMessage};
use crate::library::state::{LibraryBrowserState, LibraryDisplaySettings, OpenLibrary};
use crate::library::test_adapters::RevisionedRowAdapter;
use crate::library::writes::test_support::run_write;

const TABLE: &str = "R";

fn row() -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-1500"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R-1500"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: Some(1),
    }
}

fn app_with_browser() -> (Signex, PathBuf, RowId) {
    let (mut app, _task) = Signex::new();
    let library_id = Uuid::now_v7();
    let root = std::env::temp_dir()
        .join(format!("snx-edit-modal-{}", Uuid::now_v7().simple()))
        .join("lib.snxlib");
    app.library.open_libraries.clear();
    let row = row();
    let row_id = RowId::from_uuid(row.row_id);
    let adapter = RevisionedRowAdapter::new(library_id, 1);
    adapter.store(row.clone());
    app.library
        .set
        .mount(Box::new(adapter))
        .expect("mount stub adapter");
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "EditModalLib".into(),
        library_id,
        tables: std::collections::HashMap::from([(TABLE.to_string(), vec![row])]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        pricing: Default::default(),
    });
    app.library
        .library_browsers
        .insert(root.clone(), LibraryBrowserState::new(root.clone()));
    (app, root, row_id)
}

#[test]
fn edit_details_opens_the_modal_for_that_row_and_save_persists() {
    let (mut app, root, row_id) = app_with_browser();
    assert!(app.library.library_browsers[&root].edit_modal.is_none());

    let _ = app.update(Message::Library(LibraryMessage::BrowserOpenEditModal {
        library_path: root.clone(),
        table: TABLE.into(),
        row_id,
    }));
    let modal = app.library.library_browsers[&root]
        .edit_modal
        .clone()
        .expect("the modal opens");
    assert_eq!(modal.address.row_id, row_id);
    assert_eq!(modal.draft.internal_pn.as_str(), "R-1500");

    let edit = |msg| {
        Message::Library(LibraryMessage::BrowserEdit {
            library_path: root.clone(),
            msg,
        })
    };
    let _ = app.update(edit(BrowserEditMsg::SetInternalPn("R-1500-B".into())));
    let task = app.update(edit(BrowserEditMsg::Save));
    run_write(&mut app, task);

    assert!(
        app.library.library_browsers[&root].edit_modal.is_none(),
        "a successful save closes the modal"
    );
    // The write is synchronous; the cached grid catches up when the
    // off-thread refresh (#1374) lands.
    drop(app.land_pending_refresh(&root));
    let stored = app
        .library
        .library_at(&root)
        .and_then(|lib| lib.tables.get(TABLE))
        .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
        .expect("row still cached");
    assert_eq!(stored.internal_pn.as_str(), "R-1500-B");
}

fn open_modal(app: &mut Signex, root: &Path, row_id: RowId) {
    let _ = app.update(Message::Library(LibraryMessage::BrowserOpenEditModal {
        library_path: root.to_path_buf(),
        table: TABLE.into(),
        row_id,
    }));
}

fn edit(root: &Path, msg: BrowserEditMsg) -> Message {
    Message::Library(LibraryMessage::BrowserEdit {
        library_path: root.to_path_buf(),
        msg,
    })
}

fn stored(app: &Signex, root: &Path, row_id: RowId) -> ComponentRow {
    app.library
        .library_at(root)
        .and_then(|lib| lib.tables.get(TABLE))
        .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
        .cloned()
        .expect("row cached")
}

#[test]
fn escape_resolves_to_cancel_for_the_open_edit_modal() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);

    let esc = app
        .escape_overlay_message()
        .expect("the open edit modal claims Esc");
    assert!(
        matches!(
            &esc,
            Message::Library(LibraryMessage::BrowserEdit {
                library_path,
                msg: BrowserEditMsg::Cancel,
            }) if *library_path == root
        ),
        "Esc must resolve to the edit modal's Cancel"
    );
    let _ = app.update(esc);
    assert!(app.library.library_browsers[&root].edit_modal.is_none());
}

#[test]
fn save_commits_a_parameter_typed_without_pressing_enter() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "param_1".into(),
            value: "4.7".into(),
        },
    ));
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);

    assert!(app.library.library_browsers[&root].edit_modal.is_none());
    drop(app.land_pending_refresh(&root));
    assert_eq!(
        stored(&app, &root, row_id).parameters.get("param_1"),
        Some(&signex_library::ParamValue::Number(4.7))
    );
}

#[test]
fn save_refuses_an_invalid_uncommitted_parameter_and_keeps_the_modal_open() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "param_1".into(),
            value: "nan".into(),
        },
    ));
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);

    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .expect("modal stays open");
    assert!(
        modal
            .error
            .as_deref()
            .is_some_and(|e| e.contains("param_1"))
    );
    assert!(stored(&app, &root, row_id).parameters.is_empty());
}

#[test]
fn a_hash_pinned_datasheet_survives_editing_another_field_and_saving() {
    let (mut app, root, row_id) = app_with_browser();
    let pinned = DatasheetRef::hash_pinned("ab".repeat(32), "r1500.pdf");
    {
        let lib = app
            .library
            .open_libraries
            .iter_mut()
            .find(|l| l.root == root)
            .unwrap();
        lib.tables.get_mut(TABLE).unwrap()[0].datasheet = pinned.clone();
    }
    open_modal(&mut app, &root, row_id);
    // A keystroke routed at the URL setter must not replace the pin.
    let _ = app.update(edit(&root, BrowserEditMsg::SetDatasheetUrl("x".into())));
    let _ = app.update(edit(&root, BrowserEditMsg::SetMpn("R-1500-B".into())));
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);

    assert!(app.library.library_browsers[&root].edit_modal.is_none());
    drop(app.land_pending_refresh(&root));
    let row = stored(&app, &root, row_id);
    assert_eq!(row.primary_mpn.mpn, "R-1500-B");
    assert_eq!(row.datasheet, pinned);
}

#[test]
fn editing_the_description_updates_the_draft_and_saves_it() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetDescription("100 nF 50 V X7R".into()),
    ));
    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .unwrap();
    assert_eq!(modal.draft.description, "100 nF 50 V X7R");

    let task = app.update(edit(&root, BrowserEditMsg::Save));

    run_write(&mut app, task);
    drop(app.land_pending_refresh(&root));
    assert_eq!(stored(&app, &root, row_id).description, "100 nF 50 V X7R");
}

#[test]
fn a_pasted_multi_line_description_becomes_one_line() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetDescription("100 nF\n50 V\t\tX7R".into()),
    ));
    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .unwrap();
    assert_eq!(modal.draft.description, "100 nF 50 V X7R");
}

#[test]
fn a_description_with_triple_quotes_is_refused_at_input() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::SetDescription("ok".into())));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetDescription("a ''' b".into()),
    ));
    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .unwrap();
    assert_eq!(modal.draft.description, "ok");
    assert!(modal.error.is_some());
}

/// A "generic" class template with a measurement, a number and a text
/// slot (#1680).
fn with_slot_template(app: &mut Signex, root: &Path) {
    use signex_library::{ParamKind, ParamSlot, ParameterTemplate};
    let slot = |name: &str, kind, unit: Option<&str>| ParamSlot {
        name: name.into(),
        kind,
        unit: unit.map(str::to_string),
        allow_zero: false,
        allow_negative: false,
        display_plus_minus: false,
    };
    let template = ParameterTemplate {
        class: "generic".into(),
        description: String::new(),
        required_params: vec![slot("capacitance", ParamKind::Measurement, Some("F"))],
        optional_params: vec![
            slot("count", ParamKind::Number, None),
            slot("note", ParamKind::Text, None),
        ],
    };
    let library_id = app.library.library_at(root).expect("library").library_id;
    std::sync::Arc::make_mut(&mut app.library.template_registry)
        .insert_for_library(library_id, template);
}

fn add_slot(app: &mut Signex, root: &Path, name: &str) {
    let _ = app.update(edit(
        root,
        BrowserEditMsg::AddSlotParam { name: name.into() },
    ));
}

fn draft_param(app: &Signex, root: &Path, key: &str) -> Option<signex_library::ParamValue> {
    app.library.library_browsers[root]
        .edit_modal
        .as_ref()
        .and_then(|m| m.draft.parameters.get(key).cloned())
}

#[test]
fn adding_a_template_slot_creates_its_kind_and_unit() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    for name in ["capacitance", "count", "note"] {
        add_slot(&mut app, &root, name);
    }
    assert!(matches!(
        draft_param(&app, &root, "capacitance"),
        Some(signex_library::ParamValue::Measurement { unit, .. }) if unit == "F"
    ));
    assert!(matches!(
        draft_param(&app, &root, "count"),
        Some(signex_library::ParamValue::Number(_))
    ));
    assert!(matches!(
        draft_param(&app, &root, "note"),
        Some(signex_library::ParamValue::Text(_))
    ));
}

#[test]
fn a_new_slot_row_stores_the_typed_number_in_the_slots_unit() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    add_slot(&mut app, &root, "capacitance");
    // The row opens with an empty number field and the slot's unit.
    assert_eq!(
        app.library.library_browsers[&root]
            .edit_modal
            .as_ref()
            .and_then(|m| m.param_buf.get("capacitance").cloned()),
        Some((String::new(), "F".to_string()))
    );
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "capacitance".into(),
            value: "100".into(),
        },
    ));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::ParamPrefixPicked {
            key: "capacitance".into(),
            prefix: signex_types::quantity::SiPrefix::Nano,
        },
    ));
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);
    drop(app.land_pending_refresh(&root));
    match stored(&app, &root, row_id).parameters.get("capacitance") {
        Some(signex_library::ParamValue::Measurement { value, unit }) => {
            assert_eq!(unit, "F");
            assert!((value - 1.0e-7).abs() < 1.0e-15, "value = {value}");
        }
        other => panic!("expected a farad measurement, got {other:?}"),
    }
}

#[test]
fn saving_a_slot_row_left_empty_is_refused() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    add_slot(&mut app, &root, "capacitance");
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);
    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .expect("modal stays open");
    assert!(
        modal
            .error
            .as_deref()
            .is_some_and(|e| e.contains("capacitance"))
    );
    assert!(stored(&app, &root, row_id).parameters.is_empty());
}

#[test]
fn saving_a_number_slot_row_left_empty_or_non_numeric_is_refused() {
    for typed in [None, Some("abc")] {
        let (mut app, root, row_id) = app_with_browser();
        with_slot_template(&mut app, &root);
        open_modal(&mut app, &root, row_id);
        add_slot(&mut app, &root, "count");
        if let Some(v) = typed {
            let _ = app.update(edit(
                &root,
                BrowserEditMsg::SetParamValue {
                    key: "count".into(),
                    value: v.into(),
                },
            ));
        }
        let task = app.update(edit(&root, BrowserEditMsg::Save));
        run_write(&mut app, task);
        let modal = app.library.library_browsers[&root]
            .edit_modal
            .as_ref()
            .expect("modal stays open");
        assert!(modal.error.as_deref().is_some_and(|e| e.contains("count")));
        assert!(stored(&app, &root, row_id).parameters.is_empty());
    }
}

#[test]
fn a_number_slot_row_takes_a_typed_number() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    add_slot(&mut app, &root, "count");
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "count".into(),
            value: "4".into(),
        },
    ));
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);
    drop(app.land_pending_refresh(&root));
    assert_eq!(
        stored(&app, &root, row_id).parameters.get("count"),
        Some(&signex_library::ParamValue::Number(4.0))
    );
}

#[test]
fn adding_a_slot_shrinks_the_missing_slot_offer() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    let missing = |app: &Signex| -> Vec<String> {
        let t = app
            .library
            .template_registry
            .resolve(
                app.library.library_at(&root).expect("library").library_id,
                "generic",
            )
            .expect("template");
        let m = app.library.library_browsers[&root]
            .edit_modal
            .as_ref()
            .expect("modal");
        t.missing_slots(&m.draft.parameters)
            .map(|s| s.name.clone())
            .collect()
    };
    assert_eq!(missing(&app), ["capacitance", "count", "note"]);
    add_slot(&mut app, &root, "count");
    assert_eq!(missing(&app), ["capacitance", "note"]);
}

#[test]
fn a_slot_the_row_holds_or_the_template_lacks_is_not_added() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    add_slot(&mut app, &root, "nonesuch");
    assert!(draft_param(&app, &root, "nonesuch").is_none());
    add_slot(&mut app, &root, "note");
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamValue {
            key: "note".into(),
            value: "kept".into(),
        },
    ));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::CommitParam { key: "note".into() },
    ));
    add_slot(&mut app, &root, "note");
    assert_eq!(
        draft_param(&app, &root, "note"),
        Some(signex_library::ParamValue::Text("kept".into()))
    );
}

// ─── mockup row 130: the Internal PN conflict (lib-07) ───────────────

/// A second row holding `pn`, cached in `table` of the library at `root`.
fn cache_row(app: &mut Signex, root: &Path, table: &str, pn: &str) {
    let mut other = row();
    other.row_id = Uuid::now_v7();
    other.internal_pn = InternalPn::new(pn);
    app.library
        .library_at_mut(root)
        .expect("library")
        .tables
        .entry(table.to_string())
        .or_default()
        .push(other);
}

fn conflict(app: &Signex, root: &Path, pn: &str, row_id: RowId) -> Option<String> {
    let lib = app.library.library_at(root).expect("library");
    crate::library::edit_row_modal::pn_conflict(lib, pn, row_id)
}

#[test]
fn a_row_never_collides_with_itself() {
    let (app, root, row_id) = app_with_browser();
    assert_eq!(conflict(&app, &root, "R-1500", row_id), None);
    assert_eq!(conflict(&app, &root, " R-1500 ", row_id), None);
}

#[test]
fn a_pn_another_table_holds_is_a_conflict_naming_that_table() {
    let (mut app, root, row_id) = app_with_browser();
    cache_row(&mut app, &root, "C", "C-1");
    assert_eq!(conflict(&app, &root, "C-1", row_id), Some("C".to_string()));
    assert_eq!(conflict(&app, &root, "C-2", row_id), None);
}

/// The modal asks the backends' own rule: trimmed, case-sensitive, a blank
/// PN never clashes.
#[test]
fn the_conflict_compares_the_way_the_backends_refuse_a_write() {
    let (mut app, root, row_id) = app_with_browser();
    cache_row(&mut app, &root, "C", "C-1");
    cache_row(&mut app, &root, "C", "");
    for wanted in ["C-1", " C-1", "C-1 ", "c-1", "C-10", "", "  "] {
        assert_eq!(
            conflict(&app, &root, wanted, row_id).is_some(),
            InternalPn::clashes(wanted, "C-1") || InternalPn::clashes(wanted, ""),
            "{wanted:?}"
        );
    }
    assert_eq!(conflict(&app, &root, " C-1", row_id), Some("C".to_string()));
    assert_eq!(conflict(&app, &root, "c-1", row_id), None);
    assert_eq!(conflict(&app, &root, "", row_id), None);
}

/// Keeping the stored PN is no conflict even where another row already has
/// it: the backends only check a changed PN, so such a row stays editable.
#[test]
fn keeping_a_pn_the_library_already_holds_twice_is_no_conflict() {
    let (mut app, root, row_id) = app_with_browser();
    cache_row(&mut app, &root, "C", "R-1500");
    assert_eq!(conflict(&app, &root, "R-1500", row_id), None);
}

/// The texts of the open edit modal laid out at its card size, without
/// Fluent's isolation marks, and what a click on `click` publishes.
fn edit_modal_texts(app: &Signex, root: &Path, click: &str) -> (Vec<String>, Vec<LibraryMessage>) {
    use crate::library::edit_row_modal::{EditRowContext, view};
    let edit = app.library.library_browsers[root]
        .edit_modal
        .as_ref()
        .expect("the modal is open");
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let i18n = signex_i18n::I18n::en_us_ref();
    let size = iced::Size::new(860.0, 640.0);
    let build = || {
        let context = EditRowContext::resolve(&app.library, root, edit, Vec::new());
        view(root, edit, &tokens, i18n, false, context)
    };
    let texts = crate::layout_probe::lay_out_texts(build(), size)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    (
        texts,
        crate::layout_probe::messages_on_click(build(), size, click),
    )
}

#[test]
fn a_pn_conflict_shows_under_the_field_and_in_the_footer_and_holds_save() {
    let (mut app, root, row_id) = app_with_browser();
    cache_row(&mut app, &root, "C", "C-1");
    open_modal(&mut app, &root, row_id);
    let (_, clicked) = edit_modal_texts(&app, &root, "Save");
    assert!(
        clicked.iter().any(|m| matches!(
            m,
            LibraryMessage::BrowserEdit {
                msg: BrowserEditMsg::Save,
                ..
            }
        )),
        "without a conflict Save is pressable: {clicked:?}"
    );

    let _ = app.update(edit(&root, BrowserEditMsg::SetInternalPn("C-1".into())));
    let (texts, clicked) = edit_modal_texts(&app, &root, "Save");
    assert!(
        texts
            .iter()
            .any(|t| t == "Internal PN \u{201C}C-1\u{201D} is already used in C."),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t == "1 problem: Internal PN"),
        "{texts:?}"
    );
    assert!(clicked.is_empty(), "Save waits: {clicked:?}");
}

/// Save sent through update while another row holds the draft's Internal
/// PN is refused there too, not only by the disabled button: no save
/// starts and the stored row is unchanged.
#[test]
fn save_with_a_conflicting_pn_through_update_writes_nothing() {
    let (mut app, root, row_id) = app_with_browser();
    cache_row(&mut app, &root, "C", "C-1");
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::SetInternalPn("C-1".into())));
    let task = app.update(edit(&root, BrowserEditMsg::Save));
    run_write(&mut app, task);

    let modal = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .expect("the modal stays open");
    assert!(modal.saving.is_none(), "no save started");
    assert_eq!(modal.draft.internal_pn.as_str(), "C-1", "the draft is kept");
    let library_id = app.library.library_at(&root).expect("library").library_id;
    let written = app
        .library
        .set
        .get(library_id)
        .expect("adapter")
        .read_row(TABLE, row_id)
        .expect("the row");
    assert_eq!(written.internal_pn.as_str(), "R-1500", "nothing is written");
}

#[test]
fn a_failed_save_shows_in_the_footer_summary() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    app.library
        .library_browsers
        .get_mut(&root)
        .and_then(|b| b.edit_modal.as_mut())
        .expect("modal")
        .error = Some("the backend refused".into());
    let (texts, _) = edit_modal_texts(&app, &root, "Save");
    assert!(
        texts.iter().any(|t| t == "the backend refused"),
        "{texts:?}"
    );
}

/// Template rows carry the "template" pill and no ✕; a custom row keeps
/// its ✕.
#[test]
fn template_rows_have_a_pill_and_no_remove_and_custom_rows_keep_it() {
    let (mut app, root, row_id) = app_with_browser();
    with_slot_template(&mut app, &root);
    open_modal(&mut app, &root, row_id);
    add_slot(&mut app, &root, "count");
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let (texts, clicked) = edit_modal_texts(&app, &root, "\u{2715}");
    let count = |label: &str| texts.iter().filter(|t| *t == label).count();
    assert_eq!(count("template"), 1, "{texts:?}");
    assert_eq!(count("\u{2715}"), 1, "{texts:?}");
    assert!(
        clicked.iter().any(|m| matches!(
            m,
            LibraryMessage::BrowserEdit {
                msg: BrowserEditMsg::DeleteParam { key },
                ..
            } if key.starts_with("param_")
        )),
        "the custom row's ✕ removes it: {clicked:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Template parameters come from the ")),
        "{texts:?}"
    );
}

// ─── mockup row 137: the delete-row confirm ──────────────────────────

#[test]
fn the_delete_confirm_names_the_row_and_its_close_cancels() {
    let (_app, root, row_id) = app_with_browser();
    let confirm = crate::library::state::DeleteConfirmState {
        table: TABLE.into(),
        row_id,
        internal_pn: "R-1500".into(),
    };
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
    let i18n = signex_i18n::I18n::en_us_ref();
    let size = iced::Size::new(480.0, 400.0);
    let build = || {
        crate::library::edit_row_modal::view_delete_confirm(&root, &confirm, &tokens, i18n, false)
    };
    let texts: Vec<String> = crate::layout_probe::lay_out_texts(build(), size)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    for wanted in [
        "Delete Component?",
        "Delete R-1500?",
        "Delete row R-1500 from R? This cannot be undone from the UI.",
        "Placed parts keep their embedded copy.",
    ] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted}: {texts:?}");
    }
    let clicked = crate::layout_probe::messages_on_click(build(), size, "\u{00D7}");
    assert!(
        clicked
            .iter()
            .any(|m| matches!(m, LibraryMessage::BrowserDeleteRowCancel { .. })),
        "{clicked:?}"
    );
}

fn open_unit_menu(app: &Signex, root: &Path) -> Option<String> {
    app.library.library_browsers[root]
        .edit_modal
        .as_ref()
        .and_then(|m| m.unit_menu_open.clone())
}

/// Mockup row 139: the unit menu opens on its face, a press outside closes
/// it, and a pick closes it and applies the unit.
#[test]
fn the_unit_menu_opens_closes_and_applies_a_pick() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let key = "param_1".to_string();
    let opened = || BrowserEditMsg::UnitMenuOpened { key: key.clone() };
    let _ = app.update(edit(&root, opened()));
    assert_eq!(open_unit_menu(&app, &root), Some(key.clone()));
    let _ = app.update(edit(&root, BrowserEditMsg::UnitMenuDismissed));
    assert_eq!(open_unit_menu(&app, &root), None);
    let _ = app.update(edit(&root, opened()));
    let _ = app.update(edit(
        &root,
        BrowserEditMsg::SetParamUnit {
            key: key.clone(),
            unit: "ohm".into(),
        },
    ));
    assert_eq!(open_unit_menu(&app, &root), None);
    let buffered = app.library.library_browsers[&root]
        .edit_modal
        .as_ref()
        .and_then(|m| m.param_buf.get(&key).map(|(_, unit)| unit.clone()));
    assert_eq!(buffered.as_deref(), Some("ohm"));
}

/// Esc with the unit menu open closes the menu and keeps the dialog and its
/// draft; the next Esc cancels the dialog.
#[test]
fn escape_closes_the_unit_menu_before_the_dialog() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let key = "param_1".to_string();
    let _ = app.update(edit(&root, BrowserEditMsg::UnitMenuOpened { key }));

    let _ = app.handle_escape_pressed(None);
    assert_eq!(open_unit_menu(&app, &root), None);
    assert!(draft_param(&app, &root, "param_1").is_some());

    let _ = app.handle_escape_pressed(None);
    assert!(app.library.library_browsers[&root].edit_modal.is_none());
}

/// Esc typed in a detached panel window leaves a main-window unit menu open.
#[test]
fn escape_in_another_window_leaves_the_unit_menu_open() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let key = "param_1".to_string();
    let opened = BrowserEditMsg::UnitMenuOpened { key: key.clone() };
    let _ = app.update(edit(&root, opened));
    let panel = iced::window::Id::unique();
    app.ui_state.windows.insert(
        panel,
        crate::app::WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
    );

    assert!(!app.close_unit_menu_in(Some(panel)));
    assert_eq!(open_unit_menu(&app, &root), Some(key));
}
