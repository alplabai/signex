//! #350 — "Generic No ERC" places a real directive instead of aliasing the
//! no-connect tool: it lands in `no_erc_directives`, undoes, can be selected
//! and deleted.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::panels::{NoErcRuleEdit, PanelMsg, SelectionFormMsg};
use signex_types::schematic::{SchematicSheet, SelectedKind};
use std::path::PathBuf;

fn app_with_empty_sheet() -> Signex {
    let path = PathBuf::from("no-erc.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "no-erc".to_string(),
        path: path.clone(),
        cached_document: None,
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

fn arm_no_erc(app: &mut Signex) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceNoERC,
    )));
}

#[test]
fn generic_no_erc_arms_its_own_tool_not_no_connect() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    assert_eq!(app.interaction_state.current_tool, Tool::NoErcDirective);
}

#[test]
fn clicking_with_the_tool_places_a_directive_and_undo_removes_it() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    click(&mut app, 10.0, 20.0);

    assert!(doc(&app).no_connects.is_empty());
    assert_eq!(doc(&app).no_erc_directives.len(), 1);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).no_erc_directives.is_empty());
}

#[test]
fn a_placed_directive_can_be_selected_and_deleted() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    click(&mut app, 10.0, 20.0);
    let uuid = doc(&app).no_erc_directives[0].uuid;
    let at = doc(&app).no_erc_directives[0].position;

    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        Tool::Select,
    )));
    click(&mut app, at.x, at.y);
    let selected = &app.interaction_state.active_canvas().selected;
    assert!(
        selected
            .iter()
            .any(|i| i.uuid == uuid && i.kind == SelectedKind::NoErcDirective),
        "the directive must be hit-testable, got {selected:?}"
    );

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert!(doc(&app).no_erc_directives.is_empty());
}

fn edit_rules(app: &mut Signex, id: uuid::Uuid, edit: NoErcRuleEdit) {
    let msg = PanelMsg::EditNoErcRules(id, edit);
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

/// #1699: ticking rules in Properties narrows the directive to them (stored
/// sorted), each tick is one undo step, and "All Rules" clears the list again.
#[test]
fn properties_rule_checkboxes_edit_the_list_one_undo_step_each() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    click(&mut app, 10.0, 20.0);
    let id = doc(&app).no_erc_directives[0].uuid;
    assert!(doc(&app).no_erc_directives[0].rules.is_empty());

    let tick = |rule_id: &str| NoErcRuleEdit::Rule {
        rule_id: rule_id.to_string(),
        silenced: true,
    };
    edit_rules(&mut app, id, tick("builtin::unused_pin"));
    edit_rules(&mut app, id, tick("builtin::pin_type_conflict"));
    assert_eq!(
        doc(&app).no_erc_directives[0].rules,
        vec!["builtin::pin_type_conflict", "builtin::unused_pin"]
    );
    assert!(!doc(&app).no_erc_directives[0].suppresses("builtin::orphan_label"));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        doc(&app).no_erc_directives[0].rules,
        vec!["builtin::unused_pin"]
    );

    edit_rules(&mut app, id, NoErcRuleEdit::AllRules);
    assert!(doc(&app).no_erc_directives[0].rules.is_empty());
    assert!(doc(&app).no_erc_directives[0].suppresses("builtin::orphan_label"));
}

/// Mockup row 49: with All rules on every rule is silenced; unticking All
/// opens the list without touching the document, and ticking one rule then
/// narrows the directive to that rule alone.
#[test]
fn unticking_all_rules_then_ticking_one_silences_only_that_rule() {
    let mut app = app_with_empty_sheet();
    arm_no_erc(&mut app);
    click(&mut app, 10.0, 20.0);
    let id = doc(&app).no_erc_directives[0].uuid;
    let builtin = |kind: signex_erc::RuleKind| signex_erc::RuleId::builtin(kind).as_str().to_string();
    for kind in signex_erc::RuleKind::ALL {
        assert!(doc(&app).no_erc_directives[0].suppresses(&builtin(kind)));
    }

    let untick = PanelMsg::SelectionForm(SelectionFormMsg::AllRulesUnticked(id));
    let _ = app.update(Message::Dock(DockMessage::Panel(untick)));
    assert_eq!(
        app.document_state.panel_ctx.selection_form.picking_rules_of,
        Some(id)
    );
    assert!(doc(&app).no_erc_directives[0].rules.is_empty());

    let picked = builtin(signex_erc::RuleKind::UnusedPin);
    edit_rules(
        &mut app,
        id,
        NoErcRuleEdit::Rule {
            rule_id: picked.clone(),
            silenced: true,
        },
    );
    assert_eq!(doc(&app).no_erc_directives[0].rules, vec![picked.clone()]);
    for kind in signex_erc::RuleKind::ALL {
        let rule = builtin(kind);
        assert_eq!(
            doc(&app).no_erc_directives[0].suppresses(&rule),
            rule == picked,
            "{rule}"
        );
    }
}
