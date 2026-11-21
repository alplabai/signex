//! The Filter (Graphite mockup #55) and SCH Filter (Graphite mockup #56)
//! panels: the kind toggles with their counts, the readout, the editor the
//! Filter panel follows and the SCH Filter presets.

use std::path::PathBuf;

use pcb_geom::PickKind;
use signex_types::pcb::{PcbBoard, Via, ViaType};
use signex_types::schematic::{Point, SchematicSheet, Wire};

use super::kinds::bulk_pick;
use super::{FilterEditor, KindCounts, SchFilterMsg, board_counts};
use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, CustomFilterPreset, SelectionFilter};
use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{PanelContext, PanelKind, PanelMsg, PcbBoardMsg, PcbBoardSummary};

const WINDOW: iced::Size = iced::Size::new(320.0, 1600.0);

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        color: None,
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: 0.0,
        locked: false,
    }
}

/// An app on a schematic tab whose sheet holds three wires, with its
/// panel context built.
fn app_on_sheet() -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires: vec![
            wire((0.0, 0.0), (10.0, 0.0)),
            wire((10.0, 0.0), (10.0, 10.0)),
            wire((10.0, 10.0), (20.0, 10.0)),
        ],
        ..Default::default()
    };
    let path = PathBuf::from("selection-filter.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "selection-filter".to_string(),
        path: path.clone(),
        cached_document: None,
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
    let _ = app.set_all_selection_filters(true);
    app.refresh_panel_ctx();
    app
}

fn texts(kind: PanelKind, ctx: &PanelContext) -> Vec<String> {
    crate::layout_probe::lay_out_texts(crate::panels::view_panel(kind, ctx), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn click(kind: PanelKind, ctx: &PanelContext, label: &str) -> Vec<PanelMsg> {
    crate::layout_probe::messages_on_click(crate::panels::view_panel(kind, ctx), WINDOW, label)
}

fn send(app: &mut Signex, msg: SchFilterMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SchFilter(msg))));
}

#[test]
fn the_sheet_counts_each_kind_by_the_pick_rule() {
    let app = app_on_sheet();
    let facts = &app.document_state.panel_ctx.selection_filter;
    assert_eq!(facts.editor, FilterEditor::Schematic);
    assert_eq!(facts.schematic.get(SelectionFilter::Wires), 3);
    for kind in SelectionFilter::ALL {
        if *kind != SelectionFilter::Wires {
            assert_eq!(facts.schematic.get(*kind), 0, "{kind:?}");
        }
    }
}

#[test]
fn both_panels_list_the_twelve_kinds_with_their_counts() {
    let app = app_on_sheet();
    let ctx = &app.document_state.panel_ctx;
    for panel in [PanelKind::Filter, PanelKind::SchFilter] {
        let shown = texts(panel, ctx);
        for kind in SelectionFilter::ALL {
            let label = t_label(ctx, *kind);
            assert!(shown.contains(&label), "{panel:?}: {label} in {shown:?}");
        }
        assert!(shown.iter().any(|t| t == "3"), "{panel:?}: {shown:?}");
        assert!(
            shown.iter().filter(|t| *t == "0").count() >= SelectionFilter::ALL.len() - 1,
            "{panel:?}: {shown:?}"
        );
    }
}

fn t_label(ctx: &PanelContext, kind: SelectionFilter) -> String {
    signex_i18n::t!(ctx.i18n, kind.message()).into_string()
}

#[test]
fn the_readout_counts_the_kinds_that_are_on() {
    let mut app = app_on_sheet();
    send(
        &mut app,
        SchFilterMsg::KindToggled(SelectionFilter::Parameters),
    );
    send(
        &mut app,
        SchFilterMsg::KindToggled(SelectionFilter::DrawingObjects),
    );
    let ctx = &app.document_state.panel_ctx;
    for panel in [PanelKind::Filter, PanelKind::SchFilter] {
        let shown = texts(panel, ctx);
        assert!(
            shown.iter().any(|t| t == "10 of 12 kinds"),
            "{panel:?}: {shown:?}"
        );
    }
}

#[test]
fn all_and_none_light_only_at_the_ends() {
    assert_eq!(bulk_pick(12, 12), Some(true));
    assert_eq!(bulk_pick(0, 12), Some(false));
    assert_eq!(bulk_pick(10, 12), None);
}

#[test]
fn all_none_and_a_toggle_in_the_filter_panel_edit_the_schematic_filter() {
    let mut app = app_on_sheet();
    let ctx = app.document_state.panel_ctx.clone();
    let none = click(PanelKind::Filter, &ctx, "None");
    assert!(matches!(
        none.as_slice(),
        [PanelMsg::SchFilter(SchFilterMsg::NonePicked)]
    ));
    let _ = app.update(Message::Dock(DockMessage::Panel(none[0].clone())));
    assert!(app.interaction_state.selection_filters.is_empty());

    let all = click(PanelKind::Filter, &app.document_state.panel_ctx, "All");
    let _ = app.update(Message::Dock(DockMessage::Panel(all[0].clone())));
    assert_eq!(
        app.interaction_state.selection_filters.len(),
        SelectionFilter::ALL.len()
    );

    let wires = click(PanelKind::Filter, &app.document_state.panel_ctx, "Wires");
    assert!(matches!(
        wires.as_slice(),
        [PanelMsg::SchFilter(SchFilterMsg::KindToggled(
            SelectionFilter::Wires
        ))]
    ));
}

/// A board tab's Filter panel: the board kinds, its counts and the board
/// filter's own messages.
fn board_ctx(app: &Signex) -> PanelContext {
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.pcb_board = Some(PcbBoardSummary::default());
    ctx.selection_filter.editor = FilterEditor::Board;
    ctx.selection_filter.board = KindCounts::of([PickKind::Vias, PickKind::Vias]);
    ctx
}

#[test]
fn the_filter_panel_follows_the_active_editor() {
    let app = app_on_sheet();
    let sheet = texts(PanelKind::Filter, &app.document_state.panel_ctx);
    assert!(sheet.iter().any(|t| t == "Schematic"), "{sheet:?}");
    assert!(sheet.iter().any(|t| t == "Net Labels"), "{sheet:?}");
    assert!(!sheet.iter().any(|t| t == "Vias"), "{sheet:?}");

    let ctx = board_ctx(&app);
    let board = texts(PanelKind::Filter, &ctx);
    assert!(board.iter().any(|t| t == "Board"), "{board:?}");
    assert!(board.iter().any(|t| t == "Vias"), "{board:?}");
    assert!(board.iter().any(|t| t == "13 of 13 kinds"), "{board:?}");
    assert!(!board.iter().any(|t| t == "Net Labels"), "{board:?}");
    let vias = click(PanelKind::Filter, &ctx, "Vias");
    assert!(matches!(
        vias.as_slice(),
        [PanelMsg::PcbBoard(PcbBoardMsg::FilterKindToggled(
            PickKind::Vias
        ))]
    ));
    let none = click(PanelKind::Filter, &ctx, "None");
    assert!(matches!(
        none.as_slice(),
        [PanelMsg::PcbBoard(PcbBoardMsg::FilterAllSet(false))]
    ));
}

#[test]
fn with_no_document_the_filter_panel_is_the_empty_card() {
    let (app, _task) = Signex::new();
    let shown = texts(PanelKind::Filter, &app.document_state.panel_ctx);
    assert!(shown.iter().any(|t| t == "Selection Filter"), "{shown:?}");
}

fn board_with_vias(n: u128) -> PcbBoard {
    let via = |id: u128| Via {
        attributes: Default::default(),
        uuid: uuid::Uuid::from_u128(id),
        position: signex_types::schematic::Point::new(1.0, 1.0),
        diameter: 0.6,
        drill: 0.3,
        layers: Vec::new(),
        net: 0,
        locked: false,
        layer_sizes: None,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: (1..=n).map(via).collect(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
    }
}

#[test]
fn the_board_counts_its_objects_by_pick_kind() {
    let counts = board_counts(&board_with_vias(3));
    assert_eq!(counts.get(PickKind::Vias), 3);
    assert_eq!(counts.get(PickKind::Tracks), 0);
}

#[test]
fn save_current_keeps_the_kinds_that_are_on_up_to_the_limit() {
    let mut app = app_on_sheet();
    send(&mut app, SchFilterMsg::NonePicked);
    send(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Wires));
    for _ in 0..CUSTOM_FILTER_PRESET_LIMIT + 2 {
        send(&mut app, SchFilterMsg::PresetSaved);
    }
    let presets = &app.interaction_state.custom_filter_presets;
    assert_eq!(presets.len(), CUSTOM_FILTER_PRESET_LIMIT);
    assert_eq!(presets[0].filters, vec![SelectionFilter::Wires]);
    assert_eq!(
        app.interaction_state.active_custom_filter_tab,
        CUSTOM_FILTER_PRESET_LIMIT - 1
    );
    assert_eq!(app.document_state.panel_ctx.custom_filter_presets, *presets);
}

fn app_with_presets(names: &[&str]) -> Signex {
    let mut app = app_on_sheet();
    app.interaction_state.custom_filter_presets = names
        .iter()
        .map(|name| CustomFilterPreset::of_kinds((*name).to_string(), |_| true))
        .collect();
    app.document_state.panel_ctx.custom_filter_presets =
        app.interaction_state.custom_filter_presets.clone();
    app
}

#[test]
fn applying_a_preset_makes_it_the_edited_one() {
    let mut app = app_with_presets(&["A", "B", "C"]);
    send(&mut app, SchFilterMsg::PresetApplied(2));
    assert_eq!(app.interaction_state.active_custom_filter_tab, 2);
    assert_eq!(app.document_state.panel_ctx.active_custom_filter_tab, 2);
}

#[test]
fn rename_and_delete_keep_the_edited_preset_in_range() {
    let mut app = app_with_presets(&["A", "B", "C"]);
    send(&mut app, SchFilterMsg::PresetApplied(2));
    send(&mut app, SchFilterMsg::PresetRenameStarted(2));
    assert_eq!(
        app.document_state.panel_ctx.selection_filter.rename,
        Some((2, "C".to_string()))
    );
    send(
        &mut app,
        SchFilterMsg::PresetRenameTyped("Power".to_string()),
    );
    send(
        &mut app,
        SchFilterMsg::PresetRenamed(2, "Power".to_string()),
    );
    assert_eq!(app.interaction_state.custom_filter_presets[2].name, "Power");
    assert_eq!(app.document_state.panel_ctx.selection_filter.rename, None);

    send(&mut app, SchFilterMsg::PresetRenameStarted(2));
    send(&mut app, SchFilterMsg::PresetDeleted(2));
    assert_eq!(app.interaction_state.custom_filter_presets.len(), 2);
    assert_eq!(app.interaction_state.active_custom_filter_tab, 1);
    assert_eq!(app.document_state.panel_ctx.selection_filter.rename, None);
    send(&mut app, SchFilterMsg::PresetDeleted(7));
    assert_eq!(app.interaction_state.custom_filter_presets.len(), 2);
}

#[test]
fn the_presets_section_names_the_edited_preset_and_its_kinds() {
    let mut app = app_with_presets(&["Wiring"]);
    app.interaction_state.custom_filter_presets[0] =
        CustomFilterPreset::of_kinds("Wiring".to_string(), |k| {
            matches!(k, SelectionFilter::Wires | SelectionFilter::Buses)
        });
    app.document_state.panel_ctx.custom_filter_presets =
        app.interaction_state.custom_filter_presets.clone();
    let shown = texts(PanelKind::SchFilter, &app.document_state.panel_ctx);
    // The section title is upper-cased like every collapsible section.
    for label in ["PRESETS", "Save current", "Name", "Delete", "Wiring"] {
        assert!(shown.iter().any(|t| t == label), "{label} in {shown:?}");
    }
    assert!(
        shown.iter().any(|t| t == "Wiring: Wires and Buses"),
        "{shown:?}"
    );
    let saved = click(
        PanelKind::SchFilter,
        &app.document_state.panel_ctx,
        "Save current",
    );
    assert!(matches!(
        saved.as_slice(),
        [PanelMsg::SchFilter(SchFilterMsg::PresetSaved)]
    ));
}

/// The rename draft lives in the Model: a full panel context rebuild keeps
/// it, and it is dropped once its preset is gone.
#[test]
fn the_rename_draft_survives_a_panel_context_rebuild() {
    let mut app = app_with_presets(&["A", "B"]);
    send(&mut app, SchFilterMsg::PresetRenameStarted(1));
    send(&mut app, SchFilterMsg::PresetRenameTyped("Pwr".to_string()));
    assert_eq!(
        app.interaction_state.filter_preset_rename,
        Some((1, "Pwr".to_string()))
    );
    app.refresh_panel_ctx();
    assert_eq!(
        app.document_state.panel_ctx.selection_filter.rename,
        Some((1, "Pwr".to_string()))
    );
    app.interaction_state.custom_filter_presets.truncate(1);
    app.refresh_panel_ctx();
    assert_eq!(app.document_state.panel_ctx.selection_filter.rename, None);
}
