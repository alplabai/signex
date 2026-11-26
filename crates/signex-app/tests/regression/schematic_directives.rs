//! #2420 — the Blanket, Compile Mask and Differential Pair tools place real
//! directives: selectable at their edge (a click inside a region still
//! reaches what it covers), deletable, and each placement one undo step.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::{SchematicSheet, SelectedKind};
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let path = PathBuf::from("directives.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _initial_task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "directives".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .engines
        .values()
        .next()
        .expect("engine")
        .document()
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn arm(app: &mut Signex, action: ActiveBarAction) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
}

fn select_tool(app: &mut Signex) {
    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        Tool::Select,
    )));
}

fn selected_directive(app: &Signex) -> bool {
    app.interaction_state
        .active_canvas()
        .selected
        .iter()
        .any(|i| i.kind == SelectedKind::Directive)
}

#[test]
fn a_blanket_is_picked_at_its_edge_not_inside_and_deletes() {
    let mut app = app_with_empty_sheet();
    arm(&mut app, ActiveBarAction::PlaceBlanket);
    assert_eq!(app.interaction_state.current_tool, Tool::Blanket);
    click(&mut app, 10.0, 10.0);
    click(&mut app, 50.0, 40.0);
    assert_eq!(doc(&app).directives.len(), 1);

    select_tool(&mut app);
    click(&mut app, 30.0, 25.0);
    assert!(
        !selected_directive(&app),
        "a click inside the region reaches what it covers"
    );
    click(&mut app, 30.0, 10.0);
    assert!(selected_directive(&app), "a click on the edge picks it");

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert!(doc(&app).directives.is_empty());
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).directives.len(), 1);
}

#[test]
fn a_compile_mask_and_a_diff_pair_are_placed_and_undone_one_step_each() {
    let mut app = app_with_empty_sheet();
    arm(&mut app, ActiveBarAction::PlaceCompileMask);
    click(&mut app, 10.0, 10.0);
    click(&mut app, 20.0, 20.0);
    arm(&mut app, ActiveBarAction::PlaceDiffPair);
    assert_eq!(app.interaction_state.current_tool, Tool::DiffPair);
    click(&mut app, 30.0, 30.0);
    assert_eq!(doc(&app).directives.len(), 2);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).directives.len(), 1);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).directives.is_empty());
}

#[test]
fn a_diff_pair_mark_is_selectable() {
    let mut app = app_with_empty_sheet();
    arm(&mut app, ActiveBarAction::PlaceDiffPair);
    click(&mut app, 30.0, 30.0);
    select_tool(&mut app);
    click(&mut app, 30.0, 30.0);
    assert!(selected_directive(&app));
}

/// #1489: a sheet with one wire from (0, 0) to (20, 0).
fn app_with_wire() -> Signex {
    let mut app = app_with_empty_sheet();
    let path = PathBuf::from("directives.snxsch");
    let mut sheet = doc(&app).clone();
    sheet.wires.push(signex_types::schematic::Wire {
        start: signex_types::schematic::Point::new(0.0, 0.0),
        end: signex_types::schematic::Point::new(20.0, 0.0),
        ..Default::default()
    });
    app.document_state
        .engines
        .insert(path, signex_engine::Engine::new(sheet).expect("engine"));
    app
}

fn net_parameters_rules(app: &Signex) -> Option<signex_types::schematic::NetRules> {
    doc(app).directives.iter().find_map(|d| match d {
        signex_types::schematic::SchDirective::NetParameters { rules, .. } => Some(rules.clone()),
        _ => None,
    })
}

#[test]
fn net_parameters_go_on_a_wire_and_are_refused_off_one() {
    let mut app = app_with_wire();
    arm(&mut app, ActiveBarAction::PlaceParameterSet);
    assert_eq!(app.interaction_state.current_tool, Tool::NetParameters);
    click(&mut app, 60.0, 60.0);
    assert!(doc(&app).directives.is_empty(), "nothing off a wire");
    assert!(doc(&app).labels.is_empty(), "never a net label (#1248)");
    click(&mut app, 10.0, 0.0);
    assert_eq!(doc(&app).directives.len(), 1);
    assert!(net_parameters_rules(&app).is_some());
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).directives.is_empty(), "one undo step");
}

#[test]
fn a_typed_rule_is_one_undo_step_and_a_checkbox_another() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::{DirectivePanelMsg, NetParametersEdit, NetRule, PanelMsg};
    let mut app = app_with_wire();
    arm(&mut app, ActiveBarAction::PlaceParameterSet);
    click(&mut app, 10.0, 0.0);
    let id = doc(&app).directives[0].uuid();
    let send = |app: &mut Signex, edit: NetParametersEdit| {
        let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Directive(
            DirectivePanelMsg::NetParametersEdited(id, edit),
        ))));
    };
    for typed in ["0", "0.", "0.8"] {
        send(
            &mut app,
            NetParametersEdit::Rule(NetRule::Width, typed.to_string()),
        );
    }
    send(&mut app, NetParametersEdit::ShowOnFlag(true));
    assert_eq!(net_parameters_rules(&app).unwrap().width_mm, Some(0.8));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        net_parameters_rules(&app).unwrap().width_mm,
        Some(0.8),
        "the checkbox was its own step"
    );
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        net_parameters_rules(&app).unwrap().width_mm,
        None,
        "the typing run was one step"
    );
    assert_eq!(
        doc(&app).directives.len(),
        1,
        "the placement is still there"
    );
}
