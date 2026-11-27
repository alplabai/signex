//! #2401: the Layer Stack manager through `Signex::update`, and its agent
//! form `set_layer_stack` through `run_operands`. Every edit is one board
//! undo step.

use std::path::{Path, PathBuf};

use signex_types::material_library::{Material, MaterialKind};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_stack::{BoardFinish, LayerStack, StackLayerKind};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::layer_stack::{
    CopperUse, FinishMsg, MaterialDraft, MaterialField, MaterialPick, MaterialSource, MaterialsMsg,
    StackSubTab, StackUnit, UserLibrary,
};
use crate::panels::{LayerStackFacts, LayerStackMsg, PanelKind, PanelMsg, StackField};

/// Top copper, the top overlay and bottom copper, 1.6 mm, and a track on
/// Inner 1 when `inner` is set (with Inner 1 listed).
fn board(inner: bool) -> PcbBoard {
    let mut layers = vec![
        serde_json::json!({"id": 0, "name": "TopCopper", "layer_type": "signal"}),
        serde_json::json!({"id": 1, "name": "TopSilkscreen", "layer_type": ""}),
        serde_json::json!({"id": 31, "name": "BottomCopper", "layer_type": "signal"}),
    ];
    let mut segments = Vec::new();
    if inner {
        layers.insert(
            1,
            serde_json::json!({"id": 2, "name": "Inner1Copper", "layer_type": "signal"}),
        );
        segments.push(serde_json::json!({
            "uuid": Uuid::from_u128(7), "start": {"x": 0.0, "y": 0.0},
            "end": {"x": 5.0, "y": 0.0}, "width": 0.25, "layer": "Inner1Copper"
        }));
    }
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": layers,
        "segments": segments,
    }))
    .unwrap()
}

fn app_with(dir: &Path, inner: bool) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board(inner));
    (app, path)
}

fn send(app: &mut Signex, msg: LayerStackMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::LayerStack(msg))));
}

fn facts(app: &Signex) -> LayerStackFacts {
    app.document_view
        .panel_ctx
        .layer_stack
        .clone()
        .expect("the manager's facts")
}

fn board_of<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state.pcb_board(path).expect("board")
}

/// The copper of the layer list, in its order: the layer tabs.
fn copper_tabs(app: &Signex, path: &Path) -> Vec<String> {
    board_of(app, path)
        .layers
        .iter()
        .filter(|d| d.name.ends_with("Copper"))
        .map(|d| d.name.clone())
        .collect()
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn row_of(facts: &LayerStackFacts, kind: StackLayerKind, nth: usize) -> usize {
    facts
        .stack
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == kind)
        .nth(nth)
        .map(|(i, _)| i)
        .expect("a row of that kind")
}

#[test]
fn two_inner_layers_join_the_layer_tabs_and_the_total() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let command = AppCommandId::new("board_layer_stack").unwrap();
    let _ = app.update(Message::Menu(MenuMessage::Command(command)));
    assert!(
        app.document_state.tabs[app.document_state.active_tab]
            .kind
            .is_layer_stack(),
        "the command opens its tab"
    );

    let shown = facts(&app);
    assert!(
        !shown.stored,
        "derived from the layers until the first edit"
    );
    assert_eq!(shown.stack.copper_count(), 2);
    let total = shown.stack.total_thickness_mm();
    let top = row_of(&shown, StackLayerKind::Signal, 0);
    send(
        &mut app,
        LayerStackMsg::RowSelected {
            board: path.clone(),
            row: top,
        },
    );
    for _ in 0..2 {
        send(
            &mut app,
            LayerStackMsg::LayerAdded {
                board: path.clone(),
                kind: StackLayerKind::Signal,
            },
        );
    }

    assert_eq!(
        copper_tabs(&app, &path),
        ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"],
        "the new layers are tabs, top to bottom"
    );
    let shown = facts(&app);
    assert!(shown.stored);
    assert_eq!(shown.stack.copper_count(), 4);
    let added = 2.0 * (0.2 + signex_types::pcb_stack::DEFAULT_COPPER_MM);
    assert!((shown.stack.total_thickness_mm() - total - added).abs() < 1e-9);
    assert_eq!(
        board_of(&app, &path).thickness,
        shown.stack.total_thickness_mm(),
        "the board is as thick as its stack"
    );

    undo(&mut app);
    assert_eq!(copper_tabs(&app, &path).len(), 3, "one undo per edit");
    undo(&mut app);
    assert_eq!(copper_tabs(&app, &path), ["TopCopper", "BottomCopper"]);
    assert!(board_of(&app, &path).stackup.is_none());
    assert_eq!(board_of(&app, &path).thickness, 1.6);
}

#[test]
fn a_typed_thickness_applies_on_enter_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let row = row_of(&facts(&app), StackLayerKind::Dielectric, 0);
    send(
        &mut app,
        LayerStackMsg::FieldTyped {
            board: path.clone(),
            row,
            field: StackField::Thickness,
            text: "0.5 mm".into(),
        },
    );
    assert!(
        board_of(&app, &path).stackup.is_none(),
        "typing applies nothing"
    );
    assert_eq!(facts(&app).drafts.len(), 1);
    send(
        &mut app,
        LayerStackMsg::FieldSubmitted {
            board: path.clone(),
            row,
            field: StackField::Thickness,
        },
    );
    let stored = board_of(&app, &path).stackup.clone().expect("stored");
    assert_eq!(stored.layers[row].thickness_mm, 0.5);
    assert!(facts(&app).drafts.is_empty());
    undo(&mut app);
    assert!(board_of(&app, &path).stackup.is_none());
}

/// Mockup row 177: a layer's Df is typed in the Stack table and applied on
/// Enter as one undo step; 0 clears it again.
#[test]
fn a_typed_loss_tangent_applies_on_enter_and_zero_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let row = row_of(&facts(&app), StackLayerKind::Dielectric, 0);
    let type_df = |app: &mut Signex, text: &str| {
        send(
            app,
            LayerStackMsg::FieldTyped {
                board: path.clone(),
                row,
                field: StackField::LossTangent,
                text: text.into(),
            },
        );
        send(
            app,
            LayerStackMsg::FieldSubmitted {
                board: path.clone(),
                row,
                field: StackField::LossTangent,
            },
        );
    };
    type_df(&mut app, "0.018");
    let df = |app: &Signex| {
        board_of(app, &path)
            .stackup
            .as_ref()
            .expect("stored")
            .layers[row]
            .loss_tangent
    };
    assert_eq!(df(&app), 0.018);
    assert_eq!(facts(&app).stack.layers[row].loss_tangent, 0.018);
    type_df(&mut app, "0");
    assert_eq!(df(&app), 0.0, "0 clears the Df");
    undo(&mut app);
    assert_eq!(df(&app), 0.018, "one undo per edit");
}

#[test]
fn a_layer_with_a_track_on_it_is_not_removed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), true);
    let before = serde_json::to_string(board_of(&app, &path)).unwrap();
    let inner = row_of(&facts(&app), StackLayerKind::Signal, 1);
    send(
        &mut app,
        LayerStackMsg::RowSelected {
            board: path.clone(),
            row: inner,
        },
    );
    send(
        &mut app,
        LayerStackMsg::RowRemoved {
            board: path.clone(),
        },
    );
    assert_eq!(
        serde_json::to_string(board_of(&app, &path)).unwrap(),
        before,
        "the refused edit changed nothing"
    );
    assert_eq!(facts(&app).selected, Some(inner), "the row stays selected");
}

#[test]
fn set_layer_stack_stores_the_stack_an_agent_sends_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let mut stack = LayerStack::derived(&board_of(&app, &path).layers, 1.6);
    let bottom = stack
        .layers
        .iter()
        .rposition(|l| l.kind.is_copper())
        .unwrap();
    stack.layers.insert(
        bottom,
        signex_types::pcb_stack::StackLayer::new(StackLayerKind::Dielectric),
    );
    stack.layers.insert(
        bottom,
        signex_types::pcb_stack::StackLayer::new(StackLayerKind::Plane),
    );
    let command = AppCommandId::new("set_layer_stack").unwrap();
    let document = format!("document={}", path.display());
    let rows = format!("stack={}", serde_json::to_string(&stack).unwrap());
    let args = CommandArgs::parse_operands(&command, &[document.as_str(), rows.as_str()])
        .expect("operands");
    let (outcome, _) = app.run_operands(&command, &args);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("answered with the stack: {outcome:?}");
    };
    assert_eq!(
        answer["copper_layers"],
        serde_json::json!(["TopCopper", "Inner1Copper", "BottomCopper"])
    );
    assert_eq!(copper_tabs(&app, &path).len(), 3);
    let plane = board_of(&app, &path)
        .layers
        .iter()
        .find(|d| d.name == "Inner1Copper")
        .map(|d| d.layer_type.clone());
    assert_eq!(plane.as_deref(), Some("plane"));
    assert!(app.step_board_history(&path, true), "one undo step");
    assert_eq!(copper_tabs(&app, &path).len(), 2);

    let bad =
        CommandArgs::parse_operands(&command, &[document.as_str(), "stack=[]"]).expect("operands");
    let (outcome, _) = app.run_operands(&command, &bad);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "a stack without copper is refused: {outcome:?}"
    );
}

/// Four copper layers and a through via with a size on each of them.
fn board_with_full_via() -> PcbBoard {
    let layer =
        |id: u8, name: &str| serde_json::json!({"id": id, "name": name, "layer_type": "signal"});
    let sizes: Vec<serde_json::Value> =
        ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
            .iter()
            .zip([0.6, 0.55, 0.5, 0.45])
            .map(|(l, size)| serde_json::json!({"layer": l, "size": size}))
            .collect();
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [layer(0, "TopCopper"), layer(1, "Inner1Copper"),
                   layer(2, "Inner2Copper"), layer(31, "BottomCopper")],
        "vias": [{"uuid": Uuid::from_u128(9), "position": {"x": 0.0, "y": 0.0},
                  "diameter": 0.6, "drill": 0.2, "layers": ["TopCopper", "BottomCopper"],
                  "layer_sizes": {"mode": "full", "layers": sizes}}]
    }))
    .unwrap()
}

#[test]
fn removing_an_inner_layer_drops_its_via_sizes_saves_and_undoes_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board_with_full_via());
    let before = serde_json::to_string(board_of(&app, &path)).unwrap();
    let inner1 = row_of(&facts(&app), StackLayerKind::Signal, 1);
    send(
        &mut app,
        LayerStackMsg::RowSelected {
            board: path.clone(),
            row: inner1,
        },
    );
    send(
        &mut app,
        LayerStackMsg::RowRemoved {
            board: path.clone(),
        },
    );

    let b = board_of(&app, &path);
    assert_eq!(
        copper_tabs(&app, &path),
        ["TopCopper", "Inner1Copper", "BottomCopper"]
    );
    let Some(signex_types::pcb::LayerSizes::Full { layers }) = &b.vias[0].layer_sizes else {
        panic!("still a full list: {:?}", b.vias[0].layer_sizes);
    };
    let held: Vec<(&str, f64)> = layers.iter().map(|l| (l.layer.as_str(), l.size)).collect();
    assert_eq!(
        held,
        [
            ("TopCopper", 0.6),
            ("Inner1Copper", 0.5),
            ("BottomCopper", 0.45)
        ],
        "Inner 1's size is gone and Inner 2's moved up with its layer"
    );
    signex_types::format::SnxPcb::new(b.clone())
        .write_string()
        .expect("the board saves");

    undo(&mut app);
    assert_eq!(
        serde_json::to_string(board_of(&app, &path)).unwrap(),
        before,
        "one undo puts every size back"
    );
}

/// #2776: a board edit and its undo reach the net table and the Layer Stack
/// with no other panel refresh.
#[test]
fn the_net_table_and_the_stack_follow_an_edit_and_its_undo() {
    use signex_engine::pcb::{BoardCommand, BoardItemId};
    let dir = tempfile::tempdir().unwrap();
    let mut with_net = board(true);
    with_net.nets = serde_json::from_value(serde_json::json!([{"number": 1, "name": "GND"}]))
        .expect("a net list");
    with_net.segments[0].net = 1;
    let segment = with_net.segments[0].uuid;
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), with_net);
    app.refresh_panel_ctx();
    let length = |app: &Signex| {
        let nets = app.document_view.panel_ctx.pcb_nets.as_ref();
        nets.expect("the net table").nets[0].length_mm
    };
    assert_eq!(length(&app), 5.0);
    let delete = BoardCommand::DeleteItems {
        items: vec![BoardItemId::Segment(segment)],
    };
    app.execute_board_commands(&path, vec![delete], None)
        .expect("the command runs");
    assert_eq!(length(&app), 0.0, "the edit");
    undo(&mut app);
    assert_eq!(length(&app), 5.0, "its undo");

    let row = row_of(&facts(&app), StackLayerKind::Dielectric, 0);
    let was = facts(&app).stack.layers[row].thickness_mm;
    let mut stack = facts(&app).stack;
    stack.layers[row].thickness_mm = was + 0.1;
    app.apply_layer_stack(&path, stack).expect("stored");
    assert!(facts(&app).stored);
    undo(&mut app);
    assert_eq!(facts(&app).stack.layers[row].thickness_mm, was);
    assert!(!facts(&app).stored, "undo shows the derived stack again");
}

/// #2776: an undocked window keeps its own panel context, so an edit and an
/// undo made while the main window shows another tab reach its net table
/// and Layer Stack too.
#[test]
fn an_undocked_windows_net_table_and_stack_follow_an_edit_and_its_undo() {
    use crate::app::state::WindowKind;
    use crate::dock::PanelPosition;
    use signex_engine::pcb::{BoardCommand, BoardItemId};
    let dir = tempfile::tempdir().unwrap();
    let mut with_net = board(true);
    with_net.nets = serde_json::from_value(serde_json::json!([{"number": 1, "name": "GND"}]))
        .expect("a net list");
    with_net.segments[0].net = 1;
    let segment = with_net.segments[0].uuid;
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), with_net);
    app.document_view
        .dock
        .add_panel(PanelPosition::Right, PanelKind::PcbNets);

    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: path.clone(),
            title: "board".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    let canvas = app.new_pcb_canvas_for(&path).expect("board canvas");
    app.interaction_state.pcb_canvases.insert(window, canvas);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "sheet".into(),
        path: dir.path().join("sheet.snxsch"),
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.refresh_window_panel_ctxs(None);

    let length = |app: &Signex| {
        let nets = app.panel_ctx_for(window).pcb_nets.as_ref();
        nets.expect("the window's net table").nets[0].length_mm
    };
    assert_eq!(length(&app), 5.0);
    let delete = BoardCommand::DeleteItems {
        items: vec![BoardItemId::Segment(segment)],
    };
    app.execute_board_commands(&path, vec![delete], None)
        .expect("the command runs");
    assert_eq!(length(&app), 0.0, "the edit");
    assert!(app.step_board_history(&path, true));
    assert_eq!(length(&app), 5.0, "its undo");

    let stack = |app: &Signex| {
        let facts = app.panel_ctx_for(window).layer_stack.clone();
        facts.expect("the window's stack")
    };
    let row = row_of(&stack(&app), StackLayerKind::Dielectric, 0);
    let was = stack(&app).stack.layers[row].thickness_mm;
    let mut thicker = stack(&app).stack;
    thicker.layers[row].thickness_mm = was + 0.1;
    app.apply_layer_stack(&path, thicker).expect("stored");
    assert_eq!(stack(&app).stack.layers[row].thickness_mm, was + 0.1);
    assert!(app.step_board_history(&path, true));
    assert_eq!(stack(&app).stack.layers[row].thickness_mm, was);
}

fn type_stackup_name(app: &mut Signex, path: &Path, text: &str) {
    send(
        app,
        LayerStackMsg::StackupNameTyped {
            board: path.to_path_buf(),
            text: text.into(),
        },
    );
}

fn submit_stackup_name(app: &mut Signex, path: &Path) {
    send(
        app,
        LayerStackMsg::StackupNameSubmitted {
            board: path.to_path_buf(),
        },
    );
}

/// Decision 2026-11-22 (mockup row 105): the toolbar's name keeps a draft
/// until Enter, is one undo step, reaches the file, and an unchanged Enter
/// is no step.
#[test]
fn the_toolbar_stackup_name_is_one_undo_step_and_reaches_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);

    type_stackup_name(&mut app, &path, "  sensor-node 4L ");
    assert_eq!(facts(&app).name_draft.as_deref(), Some("  sensor-node 4L "));
    assert!(
        board_of(&app, &path).stackup.is_none(),
        "a draft is no edit"
    );

    submit_stackup_name(&mut app, &path);
    let shown = facts(&app);
    assert_eq!(shown.name_draft, None);
    assert_eq!(shown.stack.name.as_deref(), Some("sensor-node 4L"));
    let text = signex_types::format::SnxPcb::new(board_of(&app, &path).clone())
        .write_string()
        .unwrap();
    assert!(text.contains("stack_name = \"sensor-node 4L\""), "{text}");

    type_stackup_name(&mut app, &path, "sensor-node 4L");
    submit_stackup_name(&mut app, &path);
    assert!(app.step_board_history(&path, true));
    assert!(
        board_of(&app, &path).stackup.is_none(),
        "one undo takes the name back: the same name again made no step"
    );
}

#[test]
fn the_facts_count_items_name_pour_nets_and_the_projects_fab_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    let zone = |net: &str, id: u128| -> signex_types::pcb::Zone {
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::from_u128(id), "net_name": net, "layer": "Inner1Copper"
        }))
        .unwrap()
    };
    let mut pcb = board(true);
    pcb.zones = vec![zone("GND", 8), zone("GND", 9), zone("AGND", 10)];
    app.open_pcb_tab(path, "board".into(), pcb);
    let project = app.document_state.projects[0].id;
    app.document_state.projects[0].data.project_defaults.profile = Some("advanced".into());
    let at = app.document_state.active_tab;
    app.document_state.tabs[at].project_id = Some(project);

    let shown = app.layer_stack_facts().expect("the manager's facts");
    let row = |token: &str| {
        shown
            .stack
            .layers
            .iter()
            .position(|l| l.layer == token)
            .expect("a copper row")
    };
    assert_eq!(
        shown.copper_use[row("Inner1Copper")],
        Some(CopperUse {
            items: 4,
            nets: vec!["AGND".into(), "GND".into()],
        }),
        "the track and three pours; each net once, sorted"
    );
    assert_eq!(
        shown.copper_use[row("TopCopper")],
        Some(CopperUse::default())
    );
    let dielectric = row_of(&shown, StackLayerKind::Dielectric, 0);
    assert_eq!(shown.copper_use[dielectric], None, "only copper rows");
    assert_eq!(shown.copper_use.len(), shown.stack.layers.len());
    assert_eq!(shown.fab_profile.as_deref(), Some("Advanced"));

    app.document_state.projects[0].data.project_defaults.profile = None;
    let shown = app.layer_stack_facts().expect("the manager's facts");
    assert_eq!(shown.fab_profile, None, "a project that names no profile");
}

#[test]
fn a_thickness_edit_keeps_the_layer_tab_order() {
    let dir = tempfile::tempdir().unwrap();
    // The overlay is listed between Inner 1 and Bottom.
    let (mut app, path) = app_with(dir.path(), true);
    let layers = board_of(&app, &path).layers.clone();
    let row = row_of(&facts(&app), StackLayerKind::Dielectric, 0);
    send(
        &mut app,
        LayerStackMsg::FieldTyped {
            board: path.clone(),
            row,
            field: StackField::Thickness,
            text: "0.5 mm".into(),
        },
    );
    send(
        &mut app,
        LayerStackMsg::FieldSubmitted {
            board: path.clone(),
            row,
            field: StackField::Thickness,
        },
    );
    assert!(board_of(&app, &path).stackup.is_some(), "the edit applied");
    assert_eq!(
        board_of(&app, &path).layers,
        layers,
        "the tabs keep their order"
    );
}

// Mockup rows 176 and 177: the Materials and Finish sub-tabs.

fn materials(app: &mut Signex, msg: MaterialsMsg) {
    send(app, LayerStackMsg::Materials(msg));
}

fn finish(app: &mut Signex, msg: FinishMsg) {
    send(app, LayerStackMsg::Finish(msg));
}

fn sub_tab(app: &mut Signex, path: &Path, tab: StackSubTab) {
    send(
        app,
        LayerStackMsg::SubTabPicked {
            board: path.to_path_buf(),
            tab,
        },
    );
}

fn built_in(app: &Signex, name: &str) -> MaterialPick {
    let index = facts(app)
        .materials
        .built_in
        .iter()
        .position(|m| m.name == name)
        .expect("a built-in material");
    MaterialPick {
        source: MaterialSource::BuiltIn,
        index,
    }
}

#[test]
fn the_user_library_is_read_once_when_materials_first_shows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    assert_eq!(facts(&app).materials.library, UserLibrary::NotRead);
    sub_tab(&mut app, &path, StackSubTab::Drills);
    assert_eq!(
        facts(&app).materials.library,
        UserLibrary::NotRead,
        "only the Materials sub-tab reads it"
    );
    sub_tab(&mut app, &path, StackSubTab::Materials);
    assert_eq!(facts(&app).materials.library, UserLibrary::Reading);
    let mine = Material {
        name: "Mine".into(),
        kind: MaterialKind::Core,
        dielectric_constant: 3.6,
        loss_tangent: 0.004,
        thickness_mm: 0.2,
    };
    materials(
        &mut app,
        MaterialsMsg::UserLibraryRead(Ok(vec![mine.clone()])),
    );
    assert_eq!(
        facts(&app).materials.library,
        UserLibrary::Read(vec![mine.clone()])
    );
    sub_tab(&mut app, &path, StackSubTab::Stack);
    sub_tab(&mut app, &path, StackSubTab::Materials);
    assert_eq!(
        facts(&app).materials.library,
        UserLibrary::Read(vec![mine]),
        "not read again"
    );
}

#[test]
fn a_failed_read_keeps_the_library_unwritable() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    sub_tab(&mut app, &path, StackSubTab::Materials);
    materials(
        &mut app,
        MaterialsMsg::UserLibraryRead(Err("garbled".into())),
    );
    assert_eq!(
        facts(&app).materials.library,
        UserLibrary::Failed("garbled".into())
    );
    materials(
        &mut app,
        MaterialsMsg::DraftTyped {
            board: path.clone(),
            field: MaterialField::Name,
            text: "Mine".into(),
        },
    );
    materials(&mut app, MaterialsMsg::Added { board: path });
    let panel = &app.interaction_state.pcb_layer_stack;
    assert_eq!(panel.library, UserLibrary::Failed("garbled".into()));
    assert!(!panel.library_write.running, "nothing is written over it");
}

/// The accepted answer of 2026-11-24: the board stores a copy of the values
/// it uses. One undo step, and undo takes the copy back.
#[test]
fn using_a_material_copies_its_values_into_the_selected_layer_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let row = row_of(&facts(&app), StackLayerKind::Dielectric, 0);
    send(
        &mut app,
        LayerStackMsg::RowSelected {
            board: path.clone(),
            row,
        },
    );
    sub_tab(&mut app, &path, StackSubTab::Materials);
    let pick = built_in(&app, "HF hydrocarbon-ceramic core");
    materials(
        &mut app,
        MaterialsMsg::Selected {
            board: path.clone(),
            pick,
        },
    );
    assert_eq!(facts(&app).materials.selected, Some(pick));
    materials(
        &mut app,
        MaterialsMsg::Used {
            board: path.clone(),
        },
    );
    let stored = board_of(&app, &path).stackup.clone().expect("stored");
    let layer = &stored.layers[row];
    assert_eq!(layer.material, "HF hydrocarbon-ceramic core");
    assert_eq!(layer.thickness_mm, 0.508);
    assert_eq!(layer.dielectric_constant, 3.48);
    assert_eq!(layer.loss_tangent, 0.0037);
    assert!(stored.has_materials_data(), "the board keeps the Df");
    undo(&mut app);
    assert!(board_of(&app, &path).stackup.is_none(), "one undo step");
}

#[test]
fn a_material_that_does_not_fit_the_layer_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let copper = row_of(&facts(&app), StackLayerKind::Signal, 0);
    send(
        &mut app,
        LayerStackMsg::RowSelected {
            board: path.clone(),
            row: copper,
        },
    );
    let pick = built_in(&app, "Prepreg 1080");
    materials(
        &mut app,
        MaterialsMsg::Selected {
            board: path.clone(),
            pick,
        },
    );
    materials(
        &mut app,
        MaterialsMsg::Used {
            board: path.clone(),
        },
    );
    assert!(board_of(&app, &path).stackup.is_none());
}

#[test]
fn an_added_material_joins_the_user_library_and_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    sub_tab(&mut app, &path, StackSubTab::Materials);
    materials(&mut app, MaterialsMsg::UserLibraryRead(Ok(Vec::new())));
    for (field, text) in [
        (MaterialField::Name, "My prepreg"),
        (MaterialField::Thickness, "0.1"),
        (MaterialField::DielectricConstant, "3.9"),
        (MaterialField::LossTangent, "0.02"),
    ] {
        materials(
            &mut app,
            MaterialsMsg::DraftTyped {
                board: path.clone(),
                field,
                text: text.into(),
            },
        );
    }
    materials(
        &mut app,
        MaterialsMsg::Added {
            board: path.clone(),
        },
    );
    let shown = facts(&app).materials;
    let UserLibrary::Read(entries) = &shown.library else {
        panic!("{:?}", shown.library);
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "My prepreg");
    assert_eq!(entries[0].kind, MaterialKind::Prepreg, "the draft's kind");
    assert_eq!(entries[0].thickness_mm, 0.1);
    assert_eq!(shown.draft, MaterialDraft::default(), "the row is cleared");
    assert!(app.interaction_state.pcb_layer_stack.library_write.running);

    // A second edit while the write runs is owed, then written.
    materials(
        &mut app,
        MaterialsMsg::Removed {
            board: path.clone(),
        },
    );
    let write = &app.interaction_state.pcb_layer_stack.library_write;
    assert!(write.running && write.owed);
    materials(&mut app, MaterialsMsg::UserLibraryWritten(Ok(())));
    let write = &app.interaction_state.pcb_layer_stack.library_write;
    assert!(write.running && !write.owed, "the owed write started");
    materials(
        &mut app,
        MaterialsMsg::UserLibraryWritten(Err("disk full".into())),
    );
    assert_eq!(
        facts(&app).materials.write_error.as_deref(),
        Some("disk full")
    );
    assert!(board_of(&app, &path).stackup.is_none(), "no board step");
}

#[test]
fn a_draft_that_is_not_a_number_or_a_taken_name_is_not_added() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    sub_tab(&mut app, &path, StackSubTab::Materials);
    materials(&mut app, MaterialsMsg::UserLibraryRead(Ok(Vec::new())));
    let draft = |app: &mut Signex, field, text: &str| {
        materials(
            app,
            MaterialsMsg::DraftTyped {
                board: path.clone(),
                field,
                text: text.into(),
            },
        );
    };
    draft(&mut app, MaterialField::Name, "Prepreg 1080");
    draft(&mut app, MaterialField::Thickness, "thin");
    materials(
        &mut app,
        MaterialsMsg::Added {
            board: path.clone(),
        },
    );
    assert_eq!(facts(&app).materials.library, UserLibrary::Read(Vec::new()));
    draft(&mut app, MaterialField::Thickness, "0.08");
    materials(
        &mut app,
        MaterialsMsg::Added {
            board: path.clone(),
        },
    );
    assert_eq!(
        facts(&app).materials.library,
        UserLibrary::Read(Vec::new()),
        "a built-in name is taken"
    );
    assert!(!app.interaction_state.pcb_layer_stack.library_write.running);
}

/// Mockup row 176: a finish pick is one step that stores the finish with
/// the board; undo takes it back.
#[test]
fn a_finish_pick_is_stored_with_the_board_as_one_step() {
    use signex_types::pcb_stack::{FinishColor, SurfaceFinish};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    let shown = facts(&app).finish;
    assert!(!shown.stored);
    assert_eq!(shown.finish, BoardFinish::default());

    let same = BoardFinish::default();
    finish(
        &mut app,
        FinishMsg::Picked {
            board: path.clone(),
            finish: same,
        },
    );
    assert!(
        board_of(&app, &path).stackup.is_none(),
        "picking what shows stores nothing"
    );

    let mut black = BoardFinish {
        surface: SurfaceFinish::Osp,
        ..BoardFinish::default()
    };
    black.mask.color = FinishColor::Black;
    finish(
        &mut app,
        FinishMsg::Picked {
            board: path.clone(),
            finish: black.clone(),
        },
    );
    let stored = board_of(&app, &path).stackup.clone().expect("stored");
    assert_eq!(stored.finish, Some(black));
    assert!(facts(&app).finish.stored);
    undo(&mut app);
    assert!(board_of(&app, &path).stackup.is_none());
}

#[test]
fn the_mask_thickness_over_laminate_applies_on_enter_in_the_tab_unit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), false);
    send(
        &mut app,
        LayerStackMsg::UnitPicked {
            board: path.clone(),
            unit: StackUnit::Um,
        },
    );
    finish(
        &mut app,
        FinishMsg::OverLaminateTyped {
            board: path.clone(),
            text: "30".into(),
        },
    );
    assert_eq!(facts(&app).finish.draft.as_deref(), Some("30"));
    assert!(
        board_of(&app, &path).stackup.is_none(),
        "typing applies nothing"
    );
    finish(
        &mut app,
        FinishMsg::OverLaminateSubmitted {
            board: path.clone(),
        },
    );
    let stored = board_of(&app, &path).stackup.clone().expect("stored");
    let mm = stored.finish.expect("finish").mask.over_laminate_mm;
    assert!((mm - 0.03).abs() < 1e-12, "{mm}");
    assert_eq!(facts(&app).finish.draft, None);
}
