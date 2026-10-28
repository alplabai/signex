use std::path::PathBuf;

use signex_engine::{SimilarProperty, SimilarValue};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire};
use uuid::Uuid;

use super::project_search::{SearchInputs, run_search};
use super::*;
use crate::app::{TabInfo, TabKind};
use crate::find_similar::{FindSimilarMsg, SimilarScope};

fn part(reference: &str, value: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        lib_id: "Device:R".to_owned(),
        reference: reference.to_owned(),
        value: value.to_owned(),
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

fn wire(x: f64) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: Point::new(x, 20.0),
        end: Point::new(x + 10.0, 20.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    }
}

fn sheet_of(parts: Vec<Symbol>, wires: Vec<Wire>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_owned(),
        symbols: parts,
        wires,
        ..Default::default()
    }
}

/// An app showing `sheet` as its only tab, render cache built.
fn app_showing(sheet: SchematicSheet) -> Signex {
    let path = PathBuf::from("/w/p/find-similar.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "find-similar".to_owned(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app
}

fn find_similar_msg(msg: FindSimilarMsg) -> Message {
    Message::Selection(selection_request::SelectionRequest::FindSimilar(msg))
}

fn symbol_item(s: &Symbol) -> SelectedItem {
    SelectedItem::new(s.uuid, SelectedKind::Symbol)
}

fn selected_ids(app: &Signex) -> Vec<Uuid> {
    app.interaction_state
        .active_canvas()
        .selected
        .iter()
        .map(|i| i.uuid)
        .collect()
}

#[test]
fn the_object_under_the_cursor_is_the_seed_before_the_selection() {
    let a = SelectedItem::new(Uuid::from_u128(1), SelectedKind::Wire);
    let b = SelectedItem::new(Uuid::from_u128(2), SelectedKind::Wire);
    assert_eq!(pick_seed(Some(a), &[b]), Some(a));
    assert_eq!(pick_seed(None, &[b]), Some(b));
    assert_eq!(pick_seed(None, &[a, b]), None);
    assert_eq!(pick_seed(None, &[]), None);
}

#[test]
fn ok_selects_every_match_on_the_sheet_and_closes_the_dialog() {
    let parts = vec![
        part("R1", "10k", 0.0),
        part("R2", "10k", 10.0),
        part("R3", "4k7", 20.0),
    ];
    let ids: Vec<Uuid> = parts.iter().map(|p| p.uuid).collect();
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![wire(0.0)]));
    app.interaction_state.active_canvas_mut().selected = vec![seed];

    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    let state = app.ui_state.find_similar.as_ref().expect("dialog opened");
    assert_eq!(state.seed_label, "Symbol R1");
    assert_eq!(state.scope, SimilarScope::Sheet);

    // Kind is Same by default: every symbol matches.
    let _ = app.update(find_similar_msg(FindSimilarMsg::PropertySet(
        SimilarProperty::Value,
        true,
    )));
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));

    assert_eq!(selected_ids(&app), vec![ids[0], ids[1]]);
    assert!(app.ui_state.find_similar.is_none(), "OK closes the dialog");
}

#[test]
fn any_on_every_property_but_kind_selects_the_whole_kind() {
    let parts = vec![part("R1", "10k", 0.0), part("R2", "4k7", 10.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![wire(0.0)]));
    app.interaction_state.active_canvas_mut().selected = vec![seed];
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    assert_eq!(selected_ids(&app).len(), 2, "both symbols, not the wire");
}

#[test]
fn setting_a_property_to_same_then_any_returns_it_to_any() {
    let parts = vec![part("R1", "10k", 0.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![]));
    app.interaction_state.active_canvas_mut().selected = vec![seed];
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    let set = |same| {
        find_similar_msg(FindSimilarMsg::PropertySet(
            SimilarProperty::Footprint,
            same,
        ))
    };
    let _ = app.update(set(true));
    assert!(
        app.ui_state
            .find_similar
            .as_ref()
            .unwrap()
            .same
            .contains(&SimilarProperty::Footprint)
    );
    let _ = app.update(set(false));
    assert!(
        !app.ui_state
            .find_similar
            .as_ref()
            .unwrap()
            .same
            .contains(&SimilarProperty::Footprint)
    );
}

#[test]
fn no_seed_refuses_with_a_logged_warning_and_no_dialog() {
    let parts = vec![part("R1", "10k", 0.0), part("R2", "10k", 10.0)];
    let both: Vec<SelectedItem> = parts.iter().map(symbol_item).collect();
    let mut app = app_showing(sheet_of(parts, vec![]));
    app.interaction_state.active_canvas_mut().selected = both;
    let _ = crate::diagnostics::init_logging();

    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));

    assert!(app.ui_state.find_similar.is_none());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Find Similar: hover one object"))
    );
}

#[test]
fn the_object_under_the_cursor_seeds_the_dialog() {
    let near = wire(0.0);
    let hovered = SelectedItem::new(near.uuid, SelectedKind::Wire);
    let mut app = app_showing(sheet_of(vec![], vec![near, wire(100.0)]));
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested {
        at: Some((5.0, 20.0)),
    }));
    let state = app.ui_state.find_similar.as_ref().expect("dialog opened");
    assert_eq!(state.seed_label, "Wire");
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    assert_eq!(selected_ids(&app).len(), 2, "both wires match on kind");
    assert!(selected_ids(&app).contains(&hovered.uuid));
}

#[test]
fn closing_drops_the_dialog_and_a_late_project_result_is_ignored() {
    let parts = vec![part("R1", "10k", 0.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![]));
    app.interaction_state.active_canvas_mut().selected = vec![seed];
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    let _ = app.update(find_similar_msg(FindSimilarMsg::Closed));
    assert!(app.ui_state.find_similar.is_none());

    app.interaction_state.active_canvas_mut().selected.clear();
    let _ = app.update(find_similar_msg(FindSimilarMsg::ProjectSearched {
        generation: 1,
        result: std::sync::Arc::new(Ok(Default::default())),
    }));
    assert!(selected_ids(&app).is_empty(), "nothing was applied");
}

#[test]
fn the_project_search_counts_matches_on_other_sheets_only() {
    let a = sheet_of(
        vec![part("R1", "10k", 0.0), part("R2", "10k", 10.0)],
        vec![],
    );
    let b = sheet_of(
        vec![
            part("R3", "10k", 0.0),
            part("R4", "1k", 10.0),
            part("R5", "10k", 20.0),
        ],
        vec![],
    );
    let c = sheet_of(vec![part("R6", "1k", 0.0)], vec![]);
    let (pa, pb, pc) = (
        PathBuf::from("/p/a.snxsch"),
        PathBuf::from("/p/b.snxsch"),
        PathBuf::from("/p/c.snxsch"),
    );
    let criteria = vec![
        SimilarValue {
            property: SimilarProperty::Kind,
            value: "Symbol".into(),
        },
        SimilarValue {
            property: SimilarProperty::Value,
            value: "10k".into(),
        },
    ];
    let found = run_search(SearchInputs {
        criteria,
        open: [(pa.clone(), a), (pb.clone(), b), (pc.clone(), c)]
            .into_iter()
            .collect(),
        pages: vec![pa.clone(), pb.clone(), pc],
        root: pa.clone(),
        active: Some(pa),
        fold: crate::app::state::scope::CaseFold::host_default(),
    });
    assert_eq!(
        found.elsewhere,
        vec![(pb, 2)],
        "c has no 10k, a is the active sheet"
    );
    assert_eq!(found.sheets_searched, 3);
    assert!(found.unreadable.is_empty());
}

#[test]
fn esc_closes_the_dialog_before_anything_behind_it() {
    let parts = vec![part("R1", "10k", 0.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![]));
    app.interaction_state.active_canvas_mut().selected = vec![seed];
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    assert!(app.ui_state.find_similar.is_some());

    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));

    assert!(app.ui_state.find_similar.is_none());
}

#[test]
fn the_context_menu_row_opens_the_dialog_for_the_object_it_was_opened_over() {
    let over = wire(0.0);
    let id = over.uuid;
    let mut app = app_showing(sheet_of(vec![], vec![over, wire(100.0)]));
    // The cursor sat over the first wire when the menu opened.
    app.ui_state.cursor_document = app.document_state.active_path.clone();
    app.ui_state.cursor_x = 5.0;
    app.ui_state.cursor_y = 20.0;

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)));
    assert_eq!(
        app.interaction_state
            .context_menu
            .as_ref()
            .and_then(|m| m.world),
        Some((5.0, 20.0))
    );
    // The cursor then moved off to the menu row, away from the wire.
    app.ui_state.cursor_x = 500.0;
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::FindSimilar,
    )));

    assert!(app.interaction_state.context_menu.is_none());
    assert!(app.ui_state.find_similar.is_some(), "dialog opened");
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    assert!(selected_ids(&app).contains(&id));
}

/// `app_showing` placed in a project that lists its sheet, so a whole-project
/// search can start.
fn app_in_a_project(sheet: SchematicSheet) -> Signex {
    use crate::app::state::LoadedProject;
    use signex_types::project::{ProjectData, SheetEntry};
    let mut app = app_showing(sheet);
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from("/w/p/P.snxprj"),
        data: ProjectData {
            description: None,
            name: "P".to_string(),
            dir: "/w/p".to_string(),
            schematic_root: Some("find-similar.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "s".to_string(),
                filename: "find-similar.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    app.document_state.tabs[0].project_id = Some(id);
    app
}

fn project_result(
    count: usize,
) -> std::sync::Arc<Result<crate::find_similar::ProjectMatches, String>> {
    std::sync::Arc::new(Ok(crate::find_similar::ProjectMatches {
        elsewhere: vec![(PathBuf::from("/w/p/other.snxsch"), count)],
        sheets_searched: 2,
        ..Default::default()
    }))
}

fn open_project_dialog(app: &mut Signex, seed: SelectedItem) {
    app.interaction_state.active_canvas_mut().selected = vec![seed];
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    let _ = app.update(find_similar_msg(FindSimilarMsg::ScopeChosen(
        SimilarScope::Project,
    )));
}

#[test]
fn a_search_from_a_closed_dialog_cannot_answer_the_reopened_one() {
    let parts = vec![part("R1", "10k", 0.0), part("R2", "10k", 10.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_in_a_project(sheet_of(parts, vec![]));
    let _ = crate::diagnostics::init_logging();

    open_project_dialog(&mut app, seed);
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    let first = app.ui_state.find_similar_generation;
    assert!(app.ui_state.find_similar.as_ref().unwrap().running);

    // Close, reopen, search again: the new search has a new token.
    let _ = app.update(find_similar_msg(FindSimilarMsg::Closed));
    open_project_dialog(&mut app, seed);
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    let second = app.ui_state.find_similar_generation;
    assert_ne!(first, second);

    // The old result arrives: ignored, the dialog still waits.
    let _ = app.update(find_similar_msg(FindSimilarMsg::ProjectSearched {
        generation: first,
        result: project_result(7),
    }));
    assert!(app.ui_state.find_similar.as_ref().unwrap().running);
    assert!(selected_ids(&app).len() <= 1, "nothing was applied");

    // The new one lands.
    let _ = app.update(find_similar_msg(FindSimilarMsg::ProjectSearched {
        generation: second,
        result: project_result(3),
    }));
    assert!(app.ui_state.find_similar.is_none());
    assert_eq!(selected_ids(&app).len(), 2);
}

#[test]
fn clicking_same_twice_keeps_the_property_same() {
    let parts = vec![part("R1", "10k", 0.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![]));
    app.interaction_state.active_canvas_mut().selected = vec![seed];
    let _ = app.update(find_similar_msg(FindSimilarMsg::OpenRequested { at: None }));
    let set = |same| find_similar_msg(FindSimilarMsg::PropertySet(SimilarProperty::Value, same));
    let is_same = |app: &Signex| {
        app.ui_state
            .find_similar
            .as_ref()
            .unwrap()
            .same
            .contains(&SimilarProperty::Value)
    };
    let _ = app.update(set(true));
    let _ = app.update(set(true));
    assert!(is_same(&app), "Same clicked twice stays Same");
    let _ = app.update(set(false));
    let _ = app.update(set(false));
    assert!(!is_same(&app), "Any clicked twice stays Any");
}

#[test]
fn edits_are_ignored_while_a_project_search_runs() {
    let parts = vec![part("R1", "10k", 0.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_in_a_project(sheet_of(parts, vec![]));
    open_project_dialog(&mut app, seed);
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    assert!(app.ui_state.find_similar.as_ref().unwrap().running);

    let _ = app.update(find_similar_msg(FindSimilarMsg::PropertySet(
        SimilarProperty::Value,
        true,
    )));
    let _ = app.update(find_similar_msg(FindSimilarMsg::ScopeChosen(
        SimilarScope::Sheet,
    )));
    let state = app.ui_state.find_similar.as_ref().unwrap();
    assert!(!state.same.contains(&SimilarProperty::Value));
    assert_eq!(state.scope, SimilarScope::Project);
}

#[test]
fn switching_tabs_closes_the_dialog_so_a_result_never_lands_elsewhere() {
    let parts = vec![part("R1", "10k", 0.0), part("R2", "10k", 10.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_in_a_project(sheet_of(parts, vec![]));
    open_project_dialog(&mut app, seed);
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    let generation = app.ui_state.find_similar_generation;

    app.document_state.active_path = Some(PathBuf::from("another.snxsch"));
    let _ = app.update(find_similar_msg(FindSimilarMsg::ProjectSearched {
        generation,
        result: project_result(1),
    }));

    assert!(app.ui_state.find_similar.is_none());
    assert_eq!(
        selected_ids(&app).len(),
        1,
        "only the seed; nothing applied"
    );
}

#[test]
fn a_project_scope_outside_any_project_says_it_searched_the_sheet() {
    let parts = vec![part("R1", "10k", 0.0)];
    let seed = symbol_item(&parts[0]);
    let mut app = app_showing(sheet_of(parts, vec![]));
    let _ = crate::diagnostics::init_logging();
    open_project_dialog(&mut app, seed);
    let _ = app.update(find_similar_msg(FindSimilarMsg::ApplyPressed));
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Find Similar: not in a project"))
    );
    assert_eq!(selected_ids(&app).len(), 1, "the sheet was searched");
}
