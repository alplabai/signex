//! Smart Paste placed at once (#2665, mockup row 155): the dialog's checks
//! and copies, numbered from the project as read, as one undo step. The
//! fixtures are shared with `command::smart_paste_operand_tests`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::schematic::{
    Label, LabelType, LibSymbol, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use uuid::Uuid;

use super::PasteSeed;
use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use crate::smart_paste::plan::NetAfter;
use crate::smart_paste::{PlaceMode, SmartPasteState};

/// The sheet the fixtures open.
pub(crate) const SHEET: &str = "smart-paste-2665.snxsch";

/// R1 with the signal label D0 and the power label GND beside it.
pub(crate) fn sheet_to_copy() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_owned(),
        ..Default::default()
    };
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).expect("a body");
    sheet.lib_symbols.insert(body.id.clone(), body);
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".to_owned(),
        reference: "R1".to_owned(),
        position: Point::new(10.16, 10.16),
        unit: 1,
        ..Default::default()
    });
    sheet
        .labels
        .push(Label::new("D0", Point::new(20.32, 10.16), LabelType::Net));
    sheet.labels.push(Label::new(
        "GND",
        Point::new(10.16, 20.32),
        LabelType::Power,
    ));
    sheet
}

/// An app with `sheet` open and active at `path`, its R1 and labels copied.
pub(crate) fn app_with_copy_at(path: &Path, sheet: SchematicSheet) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.to_path_buf(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "smart-paste".to_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.to_path_buf());
    copy_everything(&mut app);
    app
}

/// [`app_with_copy_at`] on a loose sheet at [`SHEET`].
pub(crate) fn app_with_copy(sheet: SchematicSheet) -> Signex {
    app_with_copy_at(&PathBuf::from(SHEET), sheet)
}

/// Select the active sheet's parts and labels and copy them.
pub(crate) fn copy_everything(app: &mut Signex) {
    let doc = app
        .document_state
        .active_engine()
        .expect("a sheet")
        .document();
    let items: Vec<SelectedItem> = doc
        .symbols
        .iter()
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .chain(
            doc.labels
                .iter()
                .map(|l| SelectedItem::new(l.uuid, SelectedKind::Label)),
        )
        .collect();
    app.interaction_state.active_canvas_mut().selected = items;
    let _ = app.update(Message::Edit(EditMsg::Copy));
    assert!(
        app.interaction_state.clipboard.is_some(),
        "the copy is made"
    );
}

/// What landed on the active sheet: each part's designator and place, each
/// label's text and place, in a stable order (the ids differ per paste).
pub(crate) fn landed(app: &Signex) -> (Vec<(String, i64, i64)>, Vec<(String, i64, i64)>) {
    let nm = |mm: f64| (mm * 1e6).round() as i64;
    let doc = app
        .document_state
        .active_engine()
        .expect("a sheet")
        .document();
    let mut parts: Vec<_> = doc
        .symbols
        .iter()
        .map(|s| (s.reference.clone(), nm(s.position.x), nm(s.position.y)))
        .collect();
    let mut labels: Vec<_> = doc
        .labels
        .iter()
        .map(|l| (l.text.clone(), nm(l.position.x), nm(l.position.y)))
        .collect();
    parts.sort();
    labels.sort();
    (parts, labels)
}

/// The dialog's state for the active sheet: an array of `copies`, 10.16 mm
/// apart on X.
fn array(app: &Signex, copies: u32) -> SmartPasteState {
    let target = app.document_state.active_path.clone().expect("a sheet");
    let mut state = SmartPasteState::new(target, 10.16);
    state.mode = PlaceMode::Array;
    state.copies = copies.to_string();
    state
}

fn undo_position(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .expect("a sheet")
        .undo_position()
}

#[test]
fn an_array_lands_as_one_undo_step_numbered_from_the_sheet() {
    let mut app = app_with_copy(sheet_to_copy());
    let (before, _) = landed(&app);
    let steps = undo_position(&app);
    let mut state = array(&app, 2);
    let placed = app
        .smart_paste_at_once(
            &mut state,
            &PasteSeed::default(),
            Some(Point::new(50.8, 50.8)),
        )
        .expect("placed");
    assert_eq!(placed.copies, 2);
    assert_eq!(
        placed.designators,
        [("R1".to_owned(), vec!["R2".to_owned(), "R3".to_owned()])]
    );
    assert_eq!(
        placed.nets[0],
        (
            "D0".to_owned(),
            NetAfter::Renamed(vec!["D0_2".into(), "D0_3".into()])
        )
    );
    assert_eq!(placed.nets[1], ("GND".to_owned(), NetAfter::Power));
    assert_eq!(undo_position(&app), steps + 1, "one undo step");
    let (parts, _) = landed(&app);
    assert_eq!(parts.len(), before.len() + 2);
    app.document_state
        .active_engine_mut()
        .expect("a sheet")
        .undo()
        .expect("undo");
    assert_eq!(
        landed(&app).0,
        before,
        "one undo takes the whole array back"
    );
}

/// The project's numbers as read count: R9 in use elsewhere makes the copy
/// R10.
#[test]
fn the_projects_designators_as_read_number_the_copies() {
    let mut app = app_with_copy(sheet_to_copy());
    let seed = PasteSeed {
        counters: HashMap::from([("R".to_owned(), 9)]),
        ..PasteSeed::default()
    };
    let mut state = array(&app, 1);
    let placed = app
        .smart_paste_at_once(&mut state, &seed, None)
        .expect("placed");
    assert_eq!(placed.designators[0].1, ["R10"]);
}

/// #2744 for the operand form: a rewritten name the project writes stops
/// the paste with the dialog's words; keeping the nets goes through.
#[test]
fn a_rewritten_name_in_use_stops_the_paste_and_keep_goes_through() {
    let mut app = app_with_copy(sheet_to_copy());
    let before = landed(&app);
    let seed = PasteSeed {
        net_names: ["D0_2".to_owned()].into_iter().collect(),
        ..PasteSeed::default()
    };
    let mut state = array(&app, 1);
    let reason = app
        .smart_paste_at_once(&mut state, &seed, None)
        .expect_err("refused")
        .render_en_us();
    assert!(reason.contains("D0_2 already exists"), "{reason}");
    assert_eq!(landed(&app), before, "nothing placed");
    state.nets = crate::smart_paste::NetChoice::Keep;
    assert!(app.smart_paste_at_once(&mut state, &seed, None).is_ok());
}

/// #2744: an unread project sheet stops a paste that rewrites a net.
#[test]
fn an_unread_sheet_stops_a_rewrite_and_is_named() {
    let mut app = app_with_copy(sheet_to_copy());
    let seed = PasteSeed {
        unreadable: vec![(PathBuf::from("bad.snxsch"), "garbled".into())],
        ..PasteSeed::default()
    };
    let mut state = array(&app, 1);
    let reason = app
        .smart_paste_at_once(&mut state, &seed, None)
        .expect_err("refused")
        .render_en_us();
    assert!(reason.contains("bad.snxsch"), "{reason}");
}

/// Without `at` the copies are laid from the copy's own place: copy 1 one
/// offset from the original.
#[test]
fn without_an_anchor_copy_one_sits_one_offset_from_the_original() {
    let mut app = app_with_copy(sheet_to_copy());
    let mut state = array(&app, 1);
    let placed = app
        .smart_paste_at_once(&mut state, &PasteSeed::default(), None)
        .expect("placed");
    let origin = app
        .interaction_state
        .clipboard
        .as_ref()
        .expect("copy")
        .origin;
    assert_eq!(placed.at, origin);
    let (parts, _) = landed(&app);
    let nm = |mm: f64| (mm * 1e6).round() as i64;
    assert!(
        parts.contains(&("R2".to_owned(), nm(10.16 + 10.16), nm(10.16))),
        "{parts:?}"
    );
}

#[test]
fn a_read_only_sheet_is_refused_and_nothing_is_placed() {
    let mut sheet = sheet_to_copy();
    sheet.unknown.newer_minor_source = Some("snxsch/9.9".into());
    let mut app = app_with_copy(sheet);
    let before = landed(&app);
    let mut state = array(&app, 1);
    assert!(
        app.smart_paste_at_once(&mut state, &PasteSeed::default(), None)
            .is_err()
    );
    assert_eq!(landed(&app), before);
}
