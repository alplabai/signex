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
fn place_rows_need_a_schematic() {
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
        "Place/Sheet Entry",
    ] {
        assert!(!row_is_live(&app, path), "{path} is live with no schematic");
    }
    assert!(!row_is_live(&loaded_board_app(), "Place/Sheet Entry"));
}

/// The canvas Place flyout's Sheet Entry in an undocked sheet window, while
/// the main window shows a PCB: the gate reads the window's sheet (#2247
/// review), so the tool is armed, not refused with "No schematic is open".
#[test]
fn sheet_entry_from_an_undocked_sheets_flyout_is_gated_by_that_sheet() {
    use crate::app::{ContextAction, ContextMenuMsg};
    let pick = || {
        ContextMenuMsg::Action(ContextAction::Command {
            id: crate::keymap::AppCommandId::new("place_sheet_entry").expect("id"),
            project: None,
        })
    };
    // Control: the same pick from the main window's PCB is refused.
    let (mut app, window) = super::window_native_rows::with_undocked_window(loaded_board_app());
    assert!(app.has_active_pcb(), "the main window shows the board");
    let _ = app.update(Message::ContextMenu(pick()));
    assert_ne!(
        format!("{:?}", app.interaction_state.current_tool),
        "SheetEntry"
    );
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ScopedToWindow(
        window,
        Box::new(pick()),
    )));
    assert_eq!(
        format!("{:?}", app.interaction_state.current_tool),
        "SheetEntry"
    );
}

/// #2247: the row was a stub; it now arms the Active Bar's Sheet Entry tool.
#[test]
fn sheet_entry_arms_the_sheet_entry_tool_and_nothing_else() {
    let mut app = one_symbol_selected();
    assert!(row_is_live(&app, "Place/Sheet Entry"));
    let before = probe(&app);
    let _ = click(&mut app, "Place/Sheet Entry");
    assert_eq!(
        probe(&app),
        Probe {
            tool: "SheetEntry".to_string(),
            ..before
        }
    );
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

/// The first Design row, as the main window's bar draws it now.
fn update_pcb_row(app: &Signex) -> MenuItem {
    let model = menu_bar_model(&menu_context(app));
    let design = model
        .menus
        .iter()
        .find(|m| m.title == "Design")
        .expect("Design menu");
    match design.entries.first() {
        Some(MenuEntry::Item(item)) => item.clone(),
        other => panic!("Design starts with a row, got {other:?}"),
    }
}

/// #1312: on a project's sheet the row names the project's board and opens
/// the Update PCB dialog.
#[test]
fn design_update_pcb_names_the_board_on_a_sheet_and_opens_the_dialog() {
    let mut app = one_symbol_selected();
    let index = open_project(&mut app, "P.snxprj");
    app.document_state.projects[index].data.pcb_file = Some("P.snxpcb".to_string());
    let row = update_pcb_row(&app);
    assert!(
        row.label.starts_with("Update PCB Document"),
        "{}",
        row.label
    );
    assert!(row.label.contains("P.snxpcb"), "{}", row.label);
    assert!(row.is_enabled());
    let _ = app.update(Message::Menu(row.dispatches().expect("live").clone()));
    assert!(app.ui_state.update_pcb.is_some(), "the dialog opened");
}

/// #1312: on a project's board the same row reads Import Changes From the
/// project; without a project it is greyed.
#[test]
fn design_update_pcb_reads_import_changes_on_a_board() {
    let mut app = loaded_board_app();
    assert!(!update_pcb_row(&app).is_enabled(), "no project: greyed");
    open_project(&mut app, "P.snxprj");
    let row = update_pcb_row(&app);
    assert!(
        row.label.starts_with("Import Changes From"),
        "{}",
        row.label
    );
    assert!(row.label.contains("P.snxprj"), "{}", row.label);
    assert!(row.is_enabled());
}

/// #1312: File ▸ New ▸ PCB is live with a project open and greyed without.
#[test]
fn file_new_pcb_follows_the_open_project() {
    let mut app = bare_app();
    assert!(!row_is_live(&app, "File/New/PCB"));
    open_project(&mut app, "P.snxprj");
    assert!(row_is_live(&app, "File/New/PCB"));
}
