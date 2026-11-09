//! What the Place and Design menus' rows do.

use super::*;
use crate::app::path_ops::settle_task;

#[test]
fn tool_rows_arm_their_tool_and_nothing_else() {
    for (path, tool) in [
        ("Place/Wire", "Wire"),
        ("Place/Bus", "Bus"),
        ("Place/Junction", "Junction"),
        ("Place/Text", "Text"),
        ("Place/No Connect", "NoConnect"),
    ] {
        let mut app = one_symbol_selected();
        let before = probe(&app);
        let _ = click(&mut app, path);
        assert_eq!(
            probe(&app),
            Probe {
                tool: tool.to_string(),
                ..before
            },
            "{path}"
        );
    }
}

#[test]
fn net_label_arms_the_net_ghost_and_ends_a_pending_port() {
    let mut app = one_symbol_selected();
    app.interaction_state.pending_port = Some((
        signex_types::schematic::LabelType::Global,
        "PORT".to_string(),
    ));
    let before = probe(&app);
    let _ = click(&mut app, "Place/Net Label");
    assert_eq!(
        probe(&app),
        Probe {
            tool: "Label".to_string(),
            pending_port: false,
            ghost_label: true,
            ..before
        }
    );
    let ghost = app.interaction_state.active_canvas().ghost_label.clone();
    assert_eq!(ghost.map(|g| g.text).as_deref(), Some("NET"));
}

#[test]
fn component_opens_the_part_picker() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Place/Component...");
    assert_eq!(
        probe(&app),
        Probe {
            picker: true,
            ..before
        }
    );
}

#[test]
fn power_port_arms_the_gnd_port() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Place/Power Port");
    let after = probe(&app);
    assert!(after.pending_power, "no power port armed");
    assert_eq!(
        app.interaction_state
            .pending_power
            .as_ref()
            .map(|p| p.0.as_str()),
        Some("GND")
    );
    assert_eq!(
        after,
        Probe {
            pending_power: true,
            tool: after.tool.clone(),
            ..before
        }
    );
}

#[test]
fn place_rows_need_a_schematic_and_the_sheet_entry_stub_is_inert() {
    let app = bare_app();
    for path in [
        "Place/Wire",
        "Place/Bus",
        "Place/Net Label",
        "Place/Component...",
        "Place/Power Port",
        "Place/Junction",
        "Place/Text",
        "Place/No Connect",
    ] {
        assert!(!row_is_live(&app, path), "{path} is live with no schematic");
    }
    assert!(!row_is_live(&one_symbol_selected(), "Place/Sheet Entry"));
}

fn unnumbered(refs: &[&str]) -> Signex {
    let symbols = refs
        .iter()
        .enumerate()
        .map(|(i, r)| symbol(i as u128 + 1, r, 10.0 * (i as f64 + 1.0), 10.0))
        .collect();
    app_with(sheet_of(symbols))
}

#[test]
fn annotate_opens_its_dialog_and_reset_asks_to_confirm() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Tools/Annotation/Annotate Schematics...");
    assert_eq!(
        probe(&app),
        Probe {
            annotate: true,
            ..before.clone()
        }
    );

    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Tools/Annotation/Reset Schematic Designators...");
    assert_eq!(
        probe(&app),
        Probe {
            annotate_reset: true,
            ..before
        }
    );
}

#[test]
fn annotate_quietly_numbers_the_unnumbered_symbols_without_a_dialog() {
    let mut app = unnumbered(&["R?", "R?"]);
    let before = probe(&app);
    let task = click(&mut app, "Tools/Annotation/Annotate Schematics Quietly");
    settle_task(&mut app, task);
    let after = probe(&app);
    assert_eq!(after.refs, ["R1", "R2"]);
    assert!(!after.annotate, "no dialog");
    assert_eq!(
        after,
        Probe {
            refs: vec!["R1".into(), "R2".into()],
            can_undo: true,
            dirty: true,
            ..before
        }
    );
}

#[test]
fn force_annotate_renumbers_every_symbol_from_one() {
    let mut app = unnumbered(&["R5", "R9"]);
    let before = probe(&app);
    let task = click(&mut app, "Tools/Annotation/Force Annotate All Schematics");
    settle_task(&mut app, task);
    let after = probe(&app);
    assert_eq!(after.refs, ["R1", "R2"]);
    assert_eq!(
        after,
        Probe {
            refs: vec!["R1".into(), "R2".into()],
            can_undo: true,
            dirty: true,
            ..before
        }
    );
}

#[test]
fn reset_duplicates_resets_only_the_repeated_designators() {
    let mut app = unnumbered(&["R1", "R1", "C7"]);
    let task = click(
        &mut app,
        "Tools/Annotation/Reset Duplicate Schematic Designators...",
    );
    settle_task(&mut app, task);
    assert_eq!(probe(&app).refs, ["R?", "R?", "C7"]);
}

#[test]
fn electrical_rules_check_opens_the_erc_dialog() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Project/Electrical Rules Check");
    assert_eq!(
        probe(&app),
        Probe {
            erc: true,
            ..before
        }
    );
}

#[test]
fn parameter_manager_opens_its_table() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Design/Parameter Manager...");
    assert_eq!(
        probe(&app),
        Probe {
            parameter_manager: true,
            ..before
        }
    );
}

#[test]
fn design_rows_without_a_schematic_are_greyed() {
    let app = bare_app();
    for path in [
        "Tools/Annotation/Annotate Schematics...",
        "Tools/Annotation/Reset Schematic Designators...",
        "Tools/Annotation/Reset Duplicate Schematic Designators...",
        "Tools/Annotation/Annotate Schematics Quietly",
        "Tools/Annotation/Force Annotate All Schematics",
        "Project/Electrical Rules Check",
        "Design/Parameter Manager...",
        "Reports/Bill of Materials…",
    ] {
        assert!(!row_is_live(&app, path), "{path}");
    }
}

/// #1922: Annotate Quietly and Force Annotate are greyed when no sheet is open
/// (a project alone), where their click used to only log "nothing to do".
#[test]
fn quiet_annotate_rows_are_greyed_with_a_project_but_no_sheet() {
    let mut app = bare_app();
    open_project(&mut app, "P.snxprj");
    for path in [
        "Tools/Annotation/Annotate Schematics Quietly",
        "Tools/Annotation/Force Annotate All Schematics",
    ] {
        assert!(!row_is_live(&app, path), "{path}");
    }
    // Electrical Rules Check still works from a project alone.
    assert!(row_is_live(&app, "Project/Electrical Rules Check"));
    assert!(row_is_live(
        &one_symbol_selected(),
        "Tools/Annotation/Annotate Schematics Quietly"
    ));
}

/// #1922: Wire, Bus and Junction run the Active Bar's own action, so, like the
/// Active Bar button, the click also closes an open Active Bar dropdown and
/// remembers the tool as the wiring group's default. The old menu messages did
/// neither.
#[test]
fn wiring_rows_close_an_open_active_bar_menu_and_remember_the_tool() {
    use crate::active_bar::{ActiveBarAction, ActiveBarMenu, ActiveBarMsg};
    for (path, remembered) in [
        ("Place/Wire", ActiveBarAction::DrawWire),
        ("Place/Bus", ActiveBarAction::DrawBus),
        ("Place/Junction", ActiveBarAction::PlaceJunction),
    ] {
        let mut app = one_symbol_selected();
        let _ = app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
            ActiveBarMenu::Filter,
        )));
        assert!(app.interaction_state.active_bar_menu.is_some());
        let _ = click(&mut app, path);
        assert!(
            app.interaction_state.active_bar_menu.is_none(),
            "{path} left the dropdown open"
        );
        assert_eq!(
            app.interaction_state.last_tool.get("wiring"),
            Some(&remembered),
            "{path}"
        );
    }
}
