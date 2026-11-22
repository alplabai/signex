//! #2401: the Layer Stack manager through `Signex::update`, and its agent
//! form `set_layer_stack` through `run_operands`. Every edit is one board
//! undo step.

use std::path::{Path, PathBuf};

use signex_types::pcb::PcbBoard;
use signex_types::pcb_stack::{LayerStack, StackLayerKind};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
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
    app.document_state
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
        app.is_panel_open(PanelKind::LayerStack),
        "the command opens it"
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
        let nets = app.document_state.panel_ctx.pcb_nets.as_ref();
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
