//! #2417: the SCH Filter panel is the schematic Selection Filter, and the two
//! filter commands set the same filter from operands, in the window and
//! headless. The Net Classes panel is the net-class editor as a table.

use std::collections::HashSet;

use super::{CommandArgs, CommandOutcome};
use crate::active_bar::{CustomFilterPreset, SelectionFilter};
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::{NetClassMsg, PanelKind, PanelMsg, SchFilterMsg};

const WINDOW: iced::Size = iced::Size::new(320.0, 1200.0);

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn args(command: &str, operands: &[&str]) -> CommandArgs {
    CommandArgs::parse_operands(&id(command), operands).expect("operands parse")
}

fn set(filters: &[SelectionFilter]) -> HashSet<SelectionFilter> {
    filters.iter().copied().collect()
}

/// A fresh app with every category on and two presets: "Wiring" and an
/// unnamed second one.
fn app() -> Signex {
    let (mut app, _task) = Signex::new();
    let _ = app.set_all_selection_filters(true);
    app.interaction_state.custom_filter_presets = vec![
        CustomFilterPreset::capture(
            "Wiring".to_string(),
            &set(&[SelectionFilter::Wires, SelectionFilter::Buses]),
        ),
        CustomFilterPreset::capture(String::new(), &set(&[SelectionFilter::Components])),
    ];
    app.document_state.panel_ctx.custom_filter_presets =
        app.interaction_state.custom_filter_presets.clone();
    app
}

fn panel(app: &mut Signex, msg: SchFilterMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SchFilter(msg))));
}

/// The filter the canvas picks with, checked against the copy the panels
/// draw from.
fn filters(app: &Signex) -> HashSet<SelectionFilter> {
    assert_eq!(
        app.interaction_state.selection_filters, app.document_state.panel_ctx.selection_filters,
        "the panels show the filter the canvas uses"
    );
    app.interaction_state.selection_filters.clone()
}

fn texts<M>(element: iced::Element<'_, M>) -> Vec<String> {
    crate::layout_probe::lay_out_texts(element, WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_sch_filter_panel_is_no_longer_planned() {
    let app = app();
    let shown = texts(crate::panels::view_panel(
        PanelKind::SchFilter,
        &app.document_state.panel_ctx,
    ));
    for label in [
        "All",
        "None",
        "Wiring",
        "Filter 2",
        "Wires",
        "Net Labels",
        "Other",
    ] {
        assert!(shown.iter().any(|t| t == label), "{label} in {shown:?}");
    }
    assert!(!shown.iter().any(|t| t.contains("Planned")), "{shown:?}");
}

/// The chips wrap to the panel's width instead of running off its edge.
#[test]
fn the_sch_filter_chips_wrap_inside_a_narrow_panel() {
    let app = app();
    let laid = crate::layout_probe::lay_out_texts(
        crate::panels::view_panel(PanelKind::SchFilter, &app.document_state.panel_ctx),
        WINDOW,
    );
    for filter in SelectionFilter::ALL {
        let chip = crate::layout_probe::find(&laid, filter.label());
        assert!(
            chip.bounds.x + chip.bounds.width <= WINDOW.width,
            "{} ends at {}",
            filter.label(),
            chip.bounds.x + chip.bounds.width
        );
    }
}

#[test]
fn a_chip_click_in_the_panel_toggles_the_canvas_filter() {
    let mut app = app();
    let clicked = crate::layout_probe::messages_on_click(
        crate::panels::view_panel(PanelKind::SchFilter, &app.document_state.panel_ctx),
        WINDOW,
        "Wires",
    );
    assert_eq!(clicked.len(), 1);
    assert!(matches!(
        clicked[0],
        PanelMsg::SchFilter(SchFilterMsg::KindToggled(SelectionFilter::Wires))
    ));
    let _ = app.update(Message::Dock(DockMessage::Panel(clicked[0].clone())));
    assert!(!filters(&app).contains(&SelectionFilter::Wires));
    assert_eq!(filters(&app).len(), SelectionFilter::ALL.len() - 1);

    panel(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Wires));
    assert!(filters(&app).contains(&SelectionFilter::Wires));
}

#[test]
fn all_none_and_a_preset_in_the_panel_set_the_canvas_filter() {
    let mut app = app();
    panel(&mut app, SchFilterMsg::NonePicked);
    assert!(filters(&app).is_empty());
    panel(&mut app, SchFilterMsg::AllPicked);
    assert_eq!(filters(&app).len(), SelectionFilter::ALL.len());
    panel(&mut app, SchFilterMsg::PresetApplied(0));
    assert_eq!(
        filters(&app),
        set(&[SelectionFilter::Wires, SelectionFilter::Buses])
    );
}

/// The panel and the Active Bar dropdown edit one filter.
#[test]
fn the_panel_and_the_active_bar_dropdown_agree() {
    use crate::active_bar::ActiveBarMsg;
    let mut by_panel = app();
    let mut by_bar = app();
    panel(&mut by_panel, SchFilterMsg::PresetApplied(1));
    let _ = by_bar.update(Message::ActiveBar(ActiveBarMsg::ApplyCustomFilter(1)));
    assert_eq!(filters(&by_panel), filters(&by_bar));
    panel(
        &mut by_panel,
        SchFilterMsg::KindToggled(SelectionFilter::Ports),
    );
    let _ = by_bar.update(Message::ActiveBar(ActiveBarMsg::ToggleFilter(
        SelectionFilter::Ports,
    )));
    assert_eq!(filters(&by_panel), filters(&by_bar));
}

#[test]
fn set_selection_filters_sets_the_named_kinds_in_the_window_and_headless() {
    for headless in [false, true] {
        let mut app = app();
        let command = id("set_selection_filters");
        let given = args("set_selection_filters", &["kinds=wires, net_labels"]);
        let (outcome, _task) = if headless {
            app.run_headless_command_in_test(&command, given)
        } else {
            app.run_operands(&command, &given)
        };
        assert_eq!(outcome, CommandOutcome::Ran, "headless={headless}");
        assert_eq!(
            filters(&app),
            set(&[SelectionFilter::Wires, SelectionFilter::NetLabels]),
            "headless={headless}"
        );
    }
}

/// `all` and `none` are the All and None chips.
#[test]
fn set_selection_filters_all_and_none_match_the_chips() {
    let mut by_command = app();
    let mut by_click = app();
    for (word, chip) in [
        ("none", SchFilterMsg::NonePicked),
        ("all", SchFilterMsg::AllPicked),
    ] {
        let given = args("set_selection_filters", &[&format!("kinds={word}")]);
        let (outcome, _) = by_command.run_operands(&id("set_selection_filters"), &given);
        assert_eq!(outcome, CommandOutcome::Ran);
        panel(&mut by_click, chip);
        assert_eq!(filters(&by_command), filters(&by_click), "{word}");
    }
}

#[test]
fn an_unknown_kind_is_refused_and_changes_nothing() {
    let mut app = app();
    let before = filters(&app);
    let given = args("set_selection_filters", &["kinds=wires,rooms"]);
    let (outcome, _) = app.run_operands(&id("set_selection_filters"), &given);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(
        reason.contains("`rooms`") && reason.contains("net_labels"),
        "{reason}"
    );
    assert_eq!(filters(&app), before);
}

#[test]
fn every_kind_token_names_its_kind() {
    for filter in SelectionFilter::ALL {
        assert_eq!(SelectionFilter::from_token(filter.token()), Some(*filter));
    }
    assert_eq!(SelectionFilter::from_token("Wires"), None);
}

/// A preset by name, or by the label its chip shows when it has none: the
/// same filter a click on its chip gives.
#[test]
fn apply_selection_filter_preset_is_a_click_on_its_chip() {
    for (name, idx) in [("Wiring", 0), ("Filter 2", 1)] {
        for headless in [false, true] {
            let mut by_command = app();
            let mut by_click = app();
            let command = id("apply_selection_filter_preset");
            let given = args("apply_selection_filter_preset", &[&format!("name={name}")]);
            let (outcome, _) = if headless {
                by_command.run_headless_command_in_test(&command, given)
            } else {
                by_command.run_operands(&command, &given)
            };
            assert_eq!(outcome, CommandOutcome::Ran, "{name} headless={headless}");
            panel(&mut by_click, SchFilterMsg::PresetApplied(idx));
            assert_eq!(filters(&by_command), filters(&by_click), "{name}");
        }
    }
}

#[test]
fn an_unknown_preset_is_refused_and_changes_nothing() {
    let mut app = app();
    let before = filters(&app);
    let given = args("apply_selection_filter_preset", &["name=Power"]);
    let (outcome, _) = app.run_operands(&id("apply_selection_filter_preset"), &given);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("`Power`")),
        "{outcome:?}"
    );
    assert_eq!(filters(&app), before);
}

/// From the palette or a key the commands have no operands: they say what
/// they need and change nothing.
#[test]
fn without_operands_the_filter_commands_say_what_they_need() {
    for name in ["set_selection_filters", "apply_selection_filter_preset"] {
        let mut app = app();
        let before = filters(&app);
        let (outcome, _) = app.dispatch_command_reporting(&id(name), CommandArgs::none(), None);
        let reason = super::operand_handlers::needs_operands_reason(&id(name)).expect("a reason");
        assert_eq!(
            outcome,
            CommandOutcome::Disabled {
                reason: reason.to_string()
            },
            "{name}"
        );
        assert_eq!(filters(&app), before, "{name}");
    }
}

// ---- Net Classes panel ----

fn net_class_app() -> Signex {
    use signex_types::net_class::NetClass;
    let dir = tempfile::tempdir().unwrap().keep();
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.to_string_lossy(),
        &["a.snxsch"],
    );
    let mut power = NetClass::named("Power");
    power.nets = vec!["GND".to_string()];
    power.patterns = vec!["VCC*".to_string()];
    power.style.wire_width_mm = Some(0.4);
    app.document_state.projects[0].data.net_classes = vec![power, NetClass::named("Signal")];
    app.document_state.active_project = Some(app.document_state.projects[0].id);
    app.refresh_panel_ctx();
    app
}

#[test]
fn the_net_classes_panel_shows_the_class_table() {
    let app = net_class_app();
    let shown = texts(crate::panels::view_panel(
        PanelKind::NetClasses,
        &app.document_state.panel_ctx,
    ));
    for label in [
        "Name",
        "Colour",
        "Width",
        "Members",
        "Power",
        "Signal",
        "GND, VCC*",
        "0.4 mm",
    ] {
        assert!(shown.iter().any(|t| t == label), "{label} in {shown:?}");
    }
    assert!(!shown.iter().any(|t| t.contains("Planned")), "{shown:?}");
}

/// A click on a row selects the class the fields under the table edit, and
/// an edit from the panel is one step of the project's net-class history.
#[test]
fn a_row_click_in_the_net_classes_panel_selects_the_class_to_edit() {
    let mut app = net_class_app();
    let clicked = crate::layout_probe::messages_on_click(
        crate::panels::view_panel(PanelKind::NetClasses, &app.document_state.panel_ctx),
        WINDOW,
        "Signal",
    );
    assert_eq!(clicked.len(), 1);
    assert!(matches!(
        clicked[0],
        PanelMsg::NetClass(NetClassMsg::Select(1))
    ));
    let _ = app.update(Message::Dock(DockMessage::Panel(clicked[0].clone())));
    let shown = &app.document_state.panel_ctx.net_classes;
    assert_eq!(shown.selected, 1);
    assert_eq!(shown.draft.name, "Signal");

    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(
        NetClassMsg::RemoveSelected,
    ))));
    let classes = &app.document_state.projects[0].data.net_classes;
    assert_eq!(classes.len(), 1);
    assert!(app.document_state.panel_ctx.net_classes.can_undo);
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(
        NetClassMsg::Undo,
    ))));
    assert_eq!(app.document_state.projects[0].data.net_classes.len(), 2);
}

/// Net classes belong to the project, so the panel is offered on a sheet too.
#[test]
fn the_net_classes_panel_does_not_need_a_board() {
    assert!(!PanelKind::NetClasses.needs_pcb());
    assert!(PanelKind::LayerStack.needs_pcb());
}
