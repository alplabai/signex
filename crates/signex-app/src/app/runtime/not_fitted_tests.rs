//! #1294 — the canvas crosses out the parts that are DNP or not fitted in
//! the project's active variant, follows the variant and its settings, and
//! stops while "Cross out not-fitted parts" is off. An unfocused tile pane
//! follows them too.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Message, PreferencesMsg, Signex, TabInfo, TabKind};
use crate::canvas::CanvasSlot;
use crate::preferences::PrefMsg;
use signex_types::project::{VariantPart, VariantSettings};
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

const U1: Uuid = Uuid::from_u128(1);
const U2: Uuid = Uuid::from_u128(2);
const U3: Uuid = Uuid::from_u128(3);

fn part(uuid: Uuid, reference: &str) -> Symbol {
    Symbol {
        uuid,
        lib_id: "Device:U".to_string(),
        reference: reference.to_string(),
        ..Default::default()
    }
}

/// A project defining Lite whose one sheet, open in the active tab, places
/// U1 (DNP), U2 (not fitted in Lite on the sheet) and U3.
fn app() -> (Signex, tempfile::TempDir) {
    app_listing(&["a.snxsch"])
}

/// [`app`] with `listed` as the project's sheets.
fn app_listing(listed: &[&str]) -> (Signex, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), listed);
    let path = dir.path().join("a.snxsch");
    let mut u1 = part(U1, "U1");
    u1.dnp = true;
    let mut u2 = part(U2, "U2");
    u2.fields.insert("Fitted@Lite".into(), "no".into());
    let sheet = SchematicSheet {
        symbols: vec![u1, u2, part(U3, "U3")],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
    let id = app.document_state.projects[0].id;
    app.document_state.tabs.push(TabInfo {
        title: "a".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".to_string()];
    (app, dir)
}

fn crossed_out(app: &mut Signex) -> HashSet<Uuid> {
    app.refresh_canvas_not_fitted();
    app.interaction_state.active_canvas().not_fitted.clone()
}

fn set_active(app: &mut Signex, variant: Option<&str>) {
    app.document_state.projects[0].data.active_variant = variant.map(str::to_string);
}

fn send(app: &mut Signex, msg: PrefMsg) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
}

#[test]
fn the_preference_is_on_by_default_and_reads_a_saved_off() {
    assert!(crate::fonts::read_cross_out_not_fitted_pref_from(None));
    let saved = serde_json::json!({ crate::fonts::CROSS_OUT_NOT_FITTED_KEY: false });
    assert!(!crate::fonts::read_cross_out_not_fitted_pref_from(Some(
        &saved
    )));
    let bad = serde_json::json!({ crate::fonts::CROSS_OUT_NOT_FITTED_KEY: "no" });
    assert!(crate::fonts::read_cross_out_not_fitted_pref_from(Some(
        &bad
    )));
}

#[test]
fn base_crosses_out_the_dnp_part_and_the_active_variant_its_own() {
    let (mut app, _dir) = app();
    assert!(app.ui_state.cross_out_not_fitted, "on by default");
    assert_eq!(crossed_out(&mut app), HashSet::from([U1]));
    set_active(&mut app, Some("Lite"));
    assert_eq!(crossed_out(&mut app), HashSet::from([U1, U2]));
    set_active(&mut app, None);
    assert_eq!(crossed_out(&mut app), HashSet::from([U1]));
}

#[test]
fn a_variant_manager_setting_reaches_the_canvas() {
    let (mut app, _dir) = app();
    set_active(&mut app, Some("Lite"));
    assert_eq!(crossed_out(&mut app), HashSet::from([U1, U2]));
    app.document_state.projects[0].data.variant_settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "U3".into(),
            fitted: Some(false),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    assert_eq!(crossed_out(&mut app), HashSet::from([U1, U2, U3]));
}

#[test]
fn turning_the_preference_off_and_saving_draws_every_part_as_fitted() {
    let (mut app, _dir) = app();
    assert_eq!(crossed_out(&mut app), HashSet::from([U1]));
    send(&mut app, PrefMsg::DraftCrossOutNotFitted(false));
    assert!(app.ui_state.cross_out_not_fitted, "a draft until Save");
    assert!(app.ui_state.preferences_draft_differs());
    send(&mut app, PrefMsg::Save);
    assert!(!app.ui_state.cross_out_not_fitted);
    assert!(crossed_out(&mut app).is_empty());
    send(&mut app, PrefMsg::DraftCrossOutNotFitted(true));
    send(&mut app, PrefMsg::Save);
    assert_eq!(crossed_out(&mut app), HashSet::from([U1]));
}

#[test]
fn the_crossed_out_parts_follow_an_edit_of_the_sheet() {
    let (mut app, _dir) = app();
    assert_eq!(crossed_out(&mut app), HashSet::from([U1]));
    let path = app.document_state.active_path.clone().expect("a sheet");
    let engine = app.document_state.engines.get(&path).expect("an engine");
    let mut sheet = engine.document().clone();
    sheet.symbols[2].dnp = true;
    open_with(&mut app.document_state, &path, sheet);
    assert_eq!(crossed_out(&mut app), HashSet::from([U1, U3]));
}

const U4: Uuid = Uuid::from_u128(4);

/// [`app`] whose project also lists `b.snxsch`, shown in an unfocused tile
/// pane and placing U4, not fitted in Lite on the sheet. Returns the pane's
/// sheet too.
fn app_with_pane() -> (Signex, tempfile::TempDir, PathBuf) {
    let (mut app, dir) = app_listing(&["a.snxsch", "b.snxsch"]);
    let path = dir.path().join("b.snxsch");
    let mut u4 = part(U4, "U4");
    u4.fields.insert("Fitted@Lite".into(), "no".into());
    let sheet = SchematicSheet {
        symbols: vec![u4],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
    app.interaction_state
        .tile_panes
        .schematic
        .insert(path.clone(), CanvasSlot::default());
    (app, dir, path)
}

fn pane_not_fitted(app: &Signex, pane: &Path) -> HashSet<Uuid> {
    app.interaction_state.tile_panes.schematic[pane]
        .not_fitted
        .clone()
}

#[test]
fn an_unfocused_tile_pane_follows_the_active_variant_and_an_edit_of_its_sheet() {
    let (mut app, _dir, pane) = app_with_pane();
    app.refresh_panel_ctx();
    assert!(pane_not_fitted(&app, &pane).is_empty(), "Base fits U4");
    set_active(&mut app, Some("Lite"));
    app.refresh_panel_ctx();
    assert_eq!(pane_not_fitted(&app, &pane), HashSet::from([U4]));
    set_active(&mut app, None);
    app.refresh_panel_ctx();
    assert!(pane_not_fitted(&app, &pane).is_empty());
    let mut sheet = app.document_state.engines[&pane].document().clone();
    sheet.symbols[0].dnp = true;
    open_with(&mut app.document_state, &pane, sheet);
    app.refresh_panel_ctx();
    assert_eq!(pane_not_fitted(&app, &pane), HashSet::from([U4]));
}

#[test]
fn a_pane_is_not_recomputed_while_nothing_its_parts_depend_on_moved() {
    let (mut app, _dir, pane) = app_with_pane();
    set_active(&mut app, Some("Lite"));
    app.refresh_window_not_fitted();
    assert_eq!(pane_not_fitted(&app, &pane), HashSet::from([U4]));
    // Emptied behind the refresh's back: only a recompute would refill it.
    let slot = app.interaction_state.tile_panes.schematic.get_mut(&pane);
    slot.expect("the pane").not_fitted.clear();
    app.refresh_window_not_fitted();
    assert!(
        pane_not_fitted(&app, &pane).is_empty(),
        "kept, not recomputed"
    );
    app.document_state.projects[0].data.variant_settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "U4".into(),
            fitted: Some(false),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    app.refresh_window_not_fitted();
    assert_eq!(
        pane_not_fitted(&app, &pane),
        HashSet::from([U4]),
        "new settings recompute"
    );
}
