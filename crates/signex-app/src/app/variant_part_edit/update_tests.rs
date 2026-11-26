//! Mockup rows 208 and 209 on the GUI path: the canvas's In variant ▸
//! Alternate… and Edit value for this variant… open the picker and the
//! popover through `Signex::update`; Apply, OK and Reset to base are one
//! undo step of the General history each; an empty value is not accepted;
//! the scope on a channel tab defaults to All channels; Esc cancels.

use std::path::PathBuf;

use signex_types::pcb::SymbolLink;
use signex_types::project::{MPN_PARAMETER, PartRef, PlacedPart, VALUE_PARAMETER, VariantPart};
use signex_types::schematic::{
    ChannelOccurrence, InstancePath, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use uuid::Uuid;

use super::VariantPartEditMsg;
use super::candidates::{Candidate, CandidateSource, CandidateStatus, Candidates};
use super::state::{CandidateRead, EditScope, VariantEditKind, VariantPartEdit};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{
    ContextAction, ContextMenuMsg, ContextMenuState, Message, ProjectId, Signex, TabInfo, TabKind,
};
use crate::panels::compiled_design::{ChannelPartFit, ChannelStripView, ChannelTabView};

pub(super) const R5: Uuid = Uuid::from_u128(0x0209_0005);

/// The fixture's part: R5, 31.6k, with an MPN and a tolerance field.
pub(super) fn r5() -> Symbol {
    Symbol {
        uuid: R5,
        lib_id: "Device:R".to_string(),
        reference: "R5".to_string(),
        value: "31.6k".to_string(),
        footprint: "RES_0603".to_string(),
        fields: [
            (MPN_PARAMETER.to_string(), "RC0603FR-0731K6L".to_string()),
            ("Tolerance".to_string(), "1%".to_string()),
        ]
        .into(),
        ..Default::default()
    }
}

/// A project defining Lite and Pro, Lite active, whose sheet `power.snxsch`
/// places R5, selected, with the canvas menu open on it.
pub(super) fn app() -> (Signex, ProjectId, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["power.snxsch"]);
    // The first-run tour is its own Esc rung above every card.
    app.ui_state.first_run_tour_open = false;
    let path = dir.path().join("power.snxsch");
    let sheet = SchematicSheet {
        symbols: vec![r5()],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
    let id = app.document_state.projects[0].id;
    app.document_state.tabs.push(TabInfo {
        title: "power".to_string(),
        path: path.clone(),
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string(), "Pro".to_string()];
    data.active_variant = Some("Lite".to_string());
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(R5, SelectedKind::Symbol)];
    (app, id, path, dir)
}

/// Channel `i`'s copy of R5.
pub(super) fn copy(i: u32) -> PartRef {
    let path = InstancePath::from_chain(vec![
        Uuid::from_u128(0x0209_0000),
        Uuid::from_u128(0x0209_0000 + u128::from(i)),
    ]);
    PartRef::new(format!("CH{i}_R5"), Some(SymbolLink::new(path, [R5])))
}

/// The strip of `sheet` repeated as CH1 to CH3, `active` shown.
pub(super) fn strip(sheet: PathBuf, active: Option<u32>) -> ChannelStripView {
    let tab = |i: u32| ChannelTabView {
        channel: ChannelOccurrence {
            name: format!("CH{i}"),
            index: i,
            number: i,
            of: 3,
        },
        parts: Some(1),
        nets: Some(0),
        designators: vec![format!("CH{i}_R5")],
        miniature: None,
        fits: vec![ChannelPartFit {
            part: copy(i),
            is_fitted: true,
        }],
    };
    ChannelStripView {
        sheet,
        file: "power.snxsch".into(),
        tabs: (1..=3).map(tab).collect(),
        active,
        designator_map: Vec::new(),
        variant: Some("Lite".into()),
        is_fit_locked: false,
    }
}

/// Pick `kind`'s row of the In variant flyout, as a click on it does.
pub(super) fn open(app: &mut Signex, kind: VariantEditKind) {
    app.interaction_state.context_menu = Some(ContextMenuState {
        x: 320.0,
        y: 180.0,
        world: None,
        hit_object: true,
        window: None,
    });
    let pick = ContextMenuMsg::Action(ContextAction::VariantPartEdit(kind));
    let _ = app.update(Message::ContextMenu(pick));
}

pub(super) fn send(app: &mut Signex, msg: VariantPartEditMsg) {
    let _ = app.update(Message::from(msg));
}

/// What Lite sets on the part `designator` names.
fn lite(app: &Signex, designator: &str) -> Option<VariantPart> {
    app.document_state.projects[0]
        .data
        .variant_settings
        .part(Some("Lite"), PlacedPart::from(designator))
        .cloned()
}

fn settings_empty(app: &Signex) -> bool {
    app.document_state.projects[0]
        .data
        .variant_settings
        .is_empty()
}

/// A library alternate of R5: same value and footprint, two pins.
pub(super) fn alternate(mpn: &str) -> Candidate {
    Candidate {
        source: CandidateSource::LibraryAlternate,
        mpn: mpn.to_string(),
        manufacturer: "Yageo".to_string(),
        value: "31.6k".to_string(),
        footprint: "RES_0603".to_string(),
        pins: None,
        stock: Some(1200),
        status: CandidateStatus::Alternate(signex_library::AlternateStatus::Approved),
    }
}

/// The candidates read the picker waits for, landing now.
fn land(app: &mut Signex, candidates: Candidates) {
    let Some(VariantPartEdit::Alternate(picker)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the picker is open");
    };
    let msg = VariantPartEditMsg::CandidatesRead {
        generation: picker.generation,
        part: picker.part.this_part.designator.clone(),
        query: picker.query.trim().to_string(),
        read: Ok(Box::new(candidates)),
    };
    send(app, msg);
}

#[test]
fn edit_value_opens_on_value_and_ok_is_one_undo_step() {
    let (mut app, id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Value);
    let Some(VariantPartEdit::Value(popover)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the popover opens");
    };
    assert_eq!(popover.parameter, VALUE_PARAMETER);
    assert_eq!(popover.text, "31.6k", "the Base value until Lite sets one");
    assert_eq!(popover.part.anchor, (320.0, 180.0), "at the part");
    assert!(
        app.interaction_state.context_menu.is_none(),
        "the menu closes"
    );

    send(&mut app, VariantPartEditMsg::ValueEdited("47k".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    assert!(app.ui_state.variant_part_edit.is_none(), "OK closes it");
    let own = lite(&app, "R5").expect("Lite sets R5");
    assert_eq!(own.parameters[VALUE_PARAMETER], "47k");
    assert!(app.step_project_tab_history(id, true));
    assert!(settings_empty(&app), "one step");
}

/// DECISIONS 2026-11-26: an empty field is not a value; Reset to base is.
#[test]
fn an_empty_value_is_not_accepted_and_reset_to_base_is_one_step() {
    let (mut app, id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Value);
    send(&mut app, VariantPartEditMsg::ValueEdited("47k".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);

    open(&mut app, VariantEditKind::Value);
    let Some(VariantPartEdit::Value(popover)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the popover opens");
    };
    assert_eq!(popover.text, "47k", "the value Lite gives R5");
    send(&mut app, VariantPartEditMsg::ValueEdited("  ".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    assert!(app.ui_state.variant_part_edit.is_some(), "OK does nothing");
    assert_eq!(
        lite(&app, "R5").map(|own| own.parameters[VALUE_PARAMETER].clone()),
        Some("47k".to_string())
    );

    send(&mut app, VariantPartEditMsg::ResetPressed);
    assert!(app.ui_state.variant_part_edit.is_none());
    assert!(settings_empty(&app), "Reset to base removes the override");
    assert!(app.step_project_tab_history(id, true));
    assert!(lite(&app, "R5").is_some(), "Reset was one step");
}

/// DECISIONS 2026-11-26: the parameter list holds the part's Base
/// parameters; picking one edits it from its Base value.
#[test]
fn a_parameter_of_the_list_is_edited_from_its_base_value() {
    let (mut app, _id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Value);
    let Some(VariantPartEdit::Value(popover)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the popover opens");
    };
    let names: Vec<&str> = popover
        .part
        .base
        .parameters
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, [VALUE_PARAMETER, MPN_PARAMETER, "Tolerance"]);
    send(&mut app, VariantPartEditMsg::ParameterListToggled);
    send(
        &mut app,
        VariantPartEditMsg::ParameterPicked("Tolerance".into()),
    );
    let Some(VariantPartEdit::Value(popover)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("still open");
    };
    assert_eq!(popover.text, "1%");
    assert!(!popover.parameters_open, "picking closes the list");
    send(&mut app, VariantPartEditMsg::ValueEdited("0.5%".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    let own = lite(&app, "R5").expect("Lite sets R5");
    assert_eq!(own.parameters["Tolerance"], "0.5%");
}

#[test]
fn alternate_opens_the_picker_reads_and_apply_is_one_undo_step() {
    let (mut app, id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Alternate);
    let Some(VariantPartEdit::Alternate(picker)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the picker opens");
    };
    assert_eq!(picker.read, CandidateRead::Reading);
    assert_eq!(picker.part.base.mpn, "RC0603FR-0731K6L");
    assert_eq!(picker.query, "31.6k", "the search starts from the value");

    land(
        &mut app,
        Candidates {
            library: Some("passives.snxlib".into()),
            alternates: vec![alternate("ERJ-3EKF3162V")],
            ..Candidates::default()
        },
    );
    send(&mut app, VariantPartEditMsg::CandidatePicked(0));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    assert!(app.ui_state.variant_part_edit.is_none(), "Apply closes it");
    let own = lite(&app, "R5").expect("Lite sets R5");
    assert_eq!(own.parameters[MPN_PARAMETER], "ERJ-3EKF3162V");
    assert!(
        !own.parameters.contains_key(VALUE_PARAMETER),
        "the same value stays the part's own"
    );
    assert!(app.step_project_tab_history(id, true));
    assert!(settings_empty(&app), "one step");
}

/// A read the picker no longer waits for (an older query, an older picker)
/// is dropped.
#[test]
fn a_stale_candidate_read_is_dropped() {
    let (mut app, _id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Alternate);
    send(
        &mut app,
        VariantPartEditMsg::CandidatesRead {
            generation: 1,
            part: "R5".into(),
            query: "an older query".into(),
            read: Ok(Box::default()),
        },
    );
    let Some(VariantPartEdit::Alternate(picker)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the picker is open");
    };
    assert_eq!(picker.read, CandidateRead::Reading);
}

/// DECISIONS 2026-11-26: pins or footprint that differ warn, and Apply
/// still applies, with the alternate's footprint.
#[test]
fn a_part_whose_footprint_differs_still_applies() {
    let (mut app, _id, _path, _dir) = app();
    open(&mut app, VariantEditKind::Alternate);
    let mut other = alternate("RC1206FR-0747KL");
    other.source = CandidateSource::Search;
    other.value = "47k".into();
    other.footprint = "RES_1206".into();
    land(
        &mut app,
        Candidates {
            found: vec![other],
            found_total: 1,
            ..Candidates::default()
        },
    );
    send(&mut app, VariantPartEditMsg::CandidatePicked(0));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    let own = lite(&app, "R5").expect("Lite sets R5");
    assert_eq!(own.parameters[MPN_PARAMETER], "RC1206FR-0747KL");
    assert_eq!(own.parameters[VALUE_PARAMETER], "47k");
    assert_eq!(own.parameters["Footprint"], "RES_1206");
}

/// DECISIONS 2026-11-26: on a channel tab the scope defaults to All
/// channels; This channel only sets the shown copy alone.
#[test]
fn on_a_channel_tab_all_channels_is_the_default_scope() {
    let (mut app, id, path, _dir) = app();
    app.document_view.panel_ctx.compiled_design.strip = Some(strip(path.clone(), Some(2)));
    open(&mut app, VariantEditKind::Value);
    let Some(VariantPartEdit::Value(popover)) = app.ui_state.variant_part_edit.as_deref() else {
        panic!("the popover opens");
    };
    assert_eq!(popover.scope, EditScope::AllChannels);
    assert_eq!(popover.part.channel.as_deref(), Some("CH2"));
    assert_eq!(
        popover.part.channels,
        Some(("CH1".to_string(), "CH3".to_string()))
    );
    send(&mut app, VariantPartEditMsg::ValueEdited("47k".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    let settings = &app.document_state.projects[0].data.variant_settings;
    assert_eq!(settings.parts.len(), 3);
    for i in 1..=3 {
        let own = settings.part(Some("Lite"), copy(i).placed());
        assert_eq!(
            own.map(|own| own.parameters[VALUE_PARAMETER].as_str()),
            Some("47k")
        );
    }
    assert!(app.step_project_tab_history(id, true));
    assert!(settings_empty(&app), "one step takes every channel");
    // The undo refreshes the panels, which drop the test's compiled strip.
    app.document_view.panel_ctx.compiled_design.strip = Some(strip(path.clone(), Some(2)));

    open(&mut app, VariantEditKind::Value);
    send(
        &mut app,
        VariantPartEditMsg::ScopePicked(EditScope::ThisChannel),
    );
    send(&mut app, VariantPartEditMsg::ValueEdited("47k".into()));
    send(&mut app, VariantPartEditMsg::ApplyPressed);
    let settings = &app.document_state.projects[0].data.variant_settings;
    assert_eq!(settings.parts.len(), 1);
    assert!(settings.part(Some("Lite"), copy(2).placed()).is_some());
}

/// Nothing opens in Base, and Esc and Cancel close an open card.
#[test]
fn base_opens_nothing_and_esc_cancels() {
    let (mut app, _id, _path, _dir) = app();
    app.document_state.projects[0].data.active_variant = None;
    open(&mut app, VariantEditKind::Alternate);
    assert!(app.ui_state.variant_part_edit.is_none());

    app.document_state.projects[0].data.active_variant = Some("Lite".into());
    open(&mut app, VariantEditKind::Value);
    let esc = app.escape_overlay_message().expect("Esc has a rung");
    assert!(matches!(
        esc,
        Message::Project(crate::app::ProjectMsg::VariantPartEdit(
            VariantPartEditMsg::CancelPressed
        ))
    ));
    let _ = app.update(esc);
    assert!(app.ui_state.variant_part_edit.is_none());
    assert!(settings_empty(&app), "Cancel changes nothing");
}
