use uuid::Uuid;

use super::*;

/// The button that runs `command` on a click.
fn button_for<'a>(
    items: &'a [ActiveBarItem<Message>],
    command: &str,
) -> &'a ActiveBarButton<Message> {
    items
        .iter()
        .find_map(|item| match item {
            ActiveBarItem::Button(button) if runs(button, command) => Some(button),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a button that runs {command}"))
}

fn runs(button: &ActiveBarButton<Message>, command: &str) -> bool {
    matches!(
        &button.on_press,
        Some(Message::Menu(MenuMessage::Command(id))) if id.as_str() == command
    )
}

fn buttons(items: &[ActiveBarItem<Message>]) -> Vec<&ActiveBarButton<Message>> {
    items
        .iter()
        .filter_map(|item| match item {
            ActiveBarItem::Button(button) => Some(button),
            _ => None,
        })
        .collect()
}

/// An app with an empty board open in its tab.
fn board_app() -> Signex {
    let (mut app, _task) = Signex::new();
    let board: signex_types::pcb::PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    let path = std::env::temp_dir().join("pcb_active_bar.snxpcb");
    app.open_pcb_tab(path, "board".into(), board);
    app
}

fn bar_of(app: &Signex) -> Vec<ActiveBarItem<Message>> {
    pcb_bar_items(
        app.ui_state.effective_theme_id(),
        &app.ui_state.i18n,
        app.interaction_state.pcb_bar_menu,
    )
}

/// Clicks the button that runs `command` on `app`'s bar.
fn click(app: &mut Signex, command: &str) {
    let press = button_for(&bar_of(app), command)
        .on_press
        .clone()
        .expect("a live button");
    let _ = app.update(press);
}

fn angle(app: &Signex) -> f32 {
    app.interaction_state.pcb_canvas.view_rotation_deg()
}

/// #2234 and #2316: the PCB Active Bar's buttons run `board_sketch` and
/// `view_rotate`.
#[test]
fn the_pcb_bar_buttons_run_board_sketch_and_view_rotate() {
    let tid = ThemeId::GraphiteDark;
    let items = pcb_bar_items(tid, &I18n::en_us(), None);
    let sketch = button_for(&items, "board_sketch");
    let rotate = button_for(&items, "view_rotate");
    assert!(runs(sketch, "board_sketch") && sketch.enabled);
    assert!(runs(rotate, "view_rotate") && rotate.enabled);
    assert_eq!(rotate.tooltip, "Rotate View 90° Clockwise");
    let ActiveBarIcon::Svg(handle) = &rotate.icon else {
        panic!("an SVG icon");
    };
    assert_eq!(handle, &ic::icon_view_rotate(tid));
}

/// Round 9 of #2262: the button says what it is, in its tooltip and its
/// board glyph, not the generic rectangle.
#[test]
fn the_pcb_bar_button_is_named_and_drawn_as_board_sketch() {
    let tid = ThemeId::GraphiteDark;
    let items = pcb_bar_items(tid, &I18n::en_us(), None);
    let button = button_for(&items, "board_sketch");
    assert!(
        button.tooltip.starts_with("Board Sketch"),
        "{}",
        button.tooltip
    );
    let ActiveBarIcon::Svg(handle) = &button.icon else {
        panic!("an SVG icon");
    };
    assert_eq!(handle, &ic::icon_board_sketch(tid));
    assert_ne!(handle, &ic::icon_dd_rect(tid));
}

/// #2352, #2373: the groups in the PCB editor's order, the Selection
/// Filter, Move, Align, Board Sketch, Fabrication Outputs (#2388), View and the
/// live placement tools live, and
/// every other button greyed with a tooltip that says it is not on boards
/// yet.
#[test]
fn the_pcb_bar_has_the_footprint_layout_and_greys_what_boards_cannot_do() {
    let items = pcb_bar_items(ThemeId::GraphiteDark, &I18n::en_us(), None);
    let all = buttons(&items);
    assert_eq!(all.len(), 18);
    let separators = items
        .iter()
        .filter(|item| matches!(item, ActiveBarItem::Separator))
        .count();
    assert_eq!(separators, 4);
    let live: Vec<_> = all.iter().filter(|button| button.enabled).collect();
    let live_tools = BoardPlaceTool::PLACE_MENU
        .iter()
        .filter(|tool| tool.command().is_some())
        .count();
    assert_eq!(
        live.len(),
        6 + live_tools,
        "filter, move, align, sketch, fabrication outputs, view and the live tools"
    );
    assert!(runs(all[1], "move_object"), "Move follows the filter");
    assert!(runs(all[5], "place_board_via"), "#2373: Via is live");
    assert!(runs(all[15], "board_sketch"));
    assert!(runs(all[16], "fabrication_outputs"));
    assert!(runs(all[17], "view_rotate"), "View closes the bar");
    for button in all.iter().filter(|button| !button.enabled) {
        assert!(button.on_press.is_none() && button.on_right_press.is_none());
        assert!(
            button.tooltip.ends_with(": not available on boards yet"),
            "{}",
            button.tooltip
        );
    }
    assert!(all[0].tooltip.starts_with("Selection Filter"));
    assert!(all[3].tooltip.starts_with("Place Component"));
    assert!(all[14].tooltip.starts_with("Interactive Routing"));
}

/// #2352 / #2360: the bar greys exactly the tools the Place menu greys on a
/// board, with the same reason, and nothing else.
#[test]
fn the_bar_greys_the_place_menus_board_tools() {
    let i18n = I18n::en_us();
    let items = pcb_bar_items(ThemeId::GraphiteDark, &i18n, None);
    let mut greyed: Vec<String> = buttons(&items)
        .into_iter()
        .filter(|button| !button.enabled)
        .map(|button| button.tooltip.clone())
        .collect();
    greyed.sort();
    let mut place: Vec<String> = BoardPlaceTool::PLACE_MENU
        .iter()
        .filter_map(|tool| match tool.state() {
            ToolState::Unavailable(reason) => Some(t!(&i18n, reason).into_string()),
            ToolState::Live(_) => None,
        })
        .collect();
    place.sort();
    assert_eq!(greyed, place);
}

/// #2352: Move arms the board's click-to-click move, as Edit ▸ Move does.
#[test]
fn the_move_button_arms_the_board_move() {
    let mut app = board_app();
    click(&mut app, "move_object");
    assert!(app.interaction_state.pcb_carry.is_some(), "Move is armed");
}

/// #2234 / #2352: Board Sketch enters the board's sketch mode.
#[test]
fn the_board_sketch_button_enters_sketch_mode() {
    let mut app = board_app();
    click(&mut app, "board_sketch");
    assert!(app.board_sketch_in_view().is_some());
}

/// #2388: Fabrication Outputs opens its dialog on the board in view and
/// writes nothing.
#[test]
fn the_fabrication_outputs_button_opens_its_dialog() {
    let mut app = board_app();
    click(&mut app, "fabrication_outputs");
    let state = app
        .ui_state
        .fab_outputs
        .as_ref()
        .expect("the dialog is open");
    assert!(state.board.ends_with("pcb_active_bar.snxpcb"));
    assert_eq!(state.options.name, "pcb_active_bar");
    assert!(state.writing.is_none() && state.outcome.is_none());
}

/// #2316 / #2352: View turns the board view a quarter turn clockwise.
#[test]
fn the_view_button_turns_the_board_view() {
    let mut app = board_app();
    click(&mut app, "view_rotate");
    assert_eq!(angle(&app), 90.0);
}

/// #2352: a right-click opens the button's dropdown and draws the button
/// selected; another closes it, and so does a click outside.
#[test]
fn a_right_click_opens_and_closes_the_dropdown() {
    let mut app = board_app();
    let toggle = |app: &Signex, command: &str| {
        button_for(&bar_of(app), command)
            .on_right_press
            .clone()
            .expect("a dropdown")
    };
    let _ = app.update(toggle(&app, "view_rotate"));
    assert_eq!(app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::View));
    assert!(button_for(&bar_of(&app), "view_rotate").selected);
    assert!(!button_for(&bar_of(&app), "move_object").selected);
    let _ = app.update(toggle(&app, "move_object"));
    assert_eq!(app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::Move));
    let _ = app.update(toggle(&app, "move_object"));
    assert_eq!(app.interaction_state.pcb_bar_menu, None);
    let _ = app.update(toggle(&app, "move_object"));
    let _ = app.update(bar_message(PcbBarMsg::MenuDismissed));
    assert_eq!(app.interaction_state.pcb_bar_menu, None);
}

fn rows(app: &Signex, menu: PcbBarMenu) -> Vec<DropdownItem<Message>> {
    pcb_bar_menu_entries(
        menu,
        &app.ui_state.i18n,
        &app.document_state.panel_ctx.tokens,
        app.ui_state.effective_theme_id(),
        &app.command_context(None),
        &app.ui_state.pcb_pick_filter,
        &app.ui_state.pcb_filter_presets,
    )
    .into_iter()
    .filter_map(|entry| match entry {
        DropdownEntry::Item(item) => Some(item),
        _ => None,
    })
    .collect()
}

/// Picks the row of `menu` that runs `command`, from an open dropdown.
fn pick(app: &mut Signex, menu: PcbBarMenu, command: &str) {
    let _ = app.update(bar_message(PcbBarMsg::MenuToggled(menu)));
    let row = rows(app, menu)
        .into_iter()
        .find(|row| row.command_id == Some(command))
        .unwrap_or_else(|| panic!("a {command} row"));
    let _ = app.update(row.on_press.expect("a live row"));
    assert_eq!(
        app.interaction_state.pcb_bar_menu, None,
        "the pick closes it"
    );
}

/// #2352: each View row turns the view as its View menu row does.
#[test]
fn the_view_dropdown_rows_turn_the_board_view() {
    let mut app = board_app();
    pick(&mut app, PcbBarMenu::View, "view_rotate");
    assert_eq!(angle(&app), 90.0);
    pick(&mut app, PcbBarMenu::View, "view_rotate_180");
    assert_eq!(angle(&app), 270.0);
    pick(&mut app, PcbBarMenu::View, "view_rotate_ccw");
    assert_eq!(angle(&app), 180.0);
    pick(&mut app, PcbBarMenu::View, "view_rotate_reset");
    assert_eq!(angle(&app), 0.0);
}

/// #2352: the Move rows are labelled as their menu rows, Move runs from an
/// empty selection, and the rows that act on a selection are greyed without
/// one, by the menu bar's gate.
#[test]
fn the_move_dropdown_follows_the_menu_rows_and_their_gate() {
    let mut app = board_app();
    let move_rows = rows(&app, PcbBarMenu::Move);
    let labels: Vec<_> = move_rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Move",
            "Move Selection",
            "Rotate",
            "Rotate Clockwise",
            "Flip Selection"
        ]
    );
    let live: Vec<_> = move_rows
        .iter()
        .filter(|row| !row.disabled)
        .filter_map(|row| row.command_id)
        .collect();
    assert_eq!(live, ["move_object"], "nothing is selected");
    pick(&mut app, PcbBarMenu::Move, "move_object");
    assert!(app.interaction_state.pcb_carry.is_some(), "Move is armed");
}

/// #2352: the View rows' Reset is greyed while the view is not turned, as
/// View ▸ Reset View Rotation is.
#[test]
fn the_view_dropdown_greys_reset_at_zero() {
    let app = board_app();
    let reset = rows(&app, PcbBarMenu::View)
        .into_iter()
        .find(|row| row.command_id == Some("view_rotate_reset"))
        .expect("a reset row");
    assert!(reset.disabled && reset.on_press.is_none());
}

/// The board Selection Filter dropdown, laid out: its single chip grid.
fn filter_grid(app: &Signex) -> iced::Element<'static, Message> {
    let mut entries = pcb_bar_menu_entries(
        PcbBarMenu::Filter,
        &app.ui_state.i18n,
        &app.document_state.panel_ctx.tokens,
        app.ui_state.effective_theme_id(),
        &app.command_context(None),
        &app.ui_state.pcb_pick_filter,
        &app.ui_state.pcb_filter_presets,
    );
    assert_eq!(entries.len(), 1, "one chip grid, as on the schematic");
    match entries.remove(0) {
        DropdownEntry::Custom(grid) => grid,
        _ => panic!("the filter dropdown is the schematic's chip grid"),
    }
}

const WINDOW: iced::Size = iced::Size::new(1200.0, 800.0);

/// The texts of a laid-out dropdown, isolation marks removed.
fn grid_texts<M>(grid: iced::Element<'_, M>) -> Vec<String> {
    crate::layout_probe::lay_out_texts(grid, WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// #2373, #2383: the Selection Filter button opens the schematic's filter
/// dropdown (one builder, `active_bar::filter_menu_entry`): All / None,
/// then a chip per board kind. A chip toggles its kind in the one filter
/// the Properties chips edit, and the dropdown stays open.
#[test]
fn the_selection_filter_is_the_schematic_dropdown_with_the_board_kinds() {
    use pcb_geom::PickKind;

    let mut app = board_app();
    app.ui_state.pcb_filter_presets.clear();
    let filter = buttons(&bar_of(&app))[0].on_press.clone().expect("live");
    let _ = app.update(filter);
    assert_eq!(app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::Filter));

    // The schematic's dropdown, built by the same function, opens with the
    // same All / None control.
    let tokens = app.document_state.panel_ctx.tokens.clone();
    let schematic = crate::active_bar::filter_menu_entry(
        &tokens,
        crate::active_bar::FilterMenu {
            chips: Vec::new(),
            presets: Vec::new(),
            on: 0,
            all: (),
            none: (),
        },
    );
    let DropdownEntry::Custom(schematic) = schematic else {
        panic!("a chip grid");
    };
    let board = grid_texts(filter_grid(&app));
    assert_eq!(grid_texts(schematic), ["All", "None"]);
    assert_eq!(&board[..2], ["All", "None"]);
    assert!(board.iter().any(|t| t == "Tracks") && board.iter().any(|t| t == "Keepouts"));

    let picked = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "Tracks");
    assert_eq!(picked.len(), 1);
    let _ = app.update(picked[0].clone());
    assert!(!app.ui_state.pcb_pick_filter.is_on(PickKind::Tracks));
    assert_eq!(app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::Filter));

    // Rooms cannot be picked yet: its chip sends nothing.
    let rooms = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "Rooms");
    assert!(rooms.is_empty(), "{rooms:?}");

    let none = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "None");
    let _ = app.update(none[0].clone());
    assert_eq!(app.ui_state.pcb_pick_filter.on_kinds().count(), 0);
}

/// #2383: a preset saved from the board filter is a preset chip of the
/// Active Bar's Selection Filter dropdown, and the chip applies it.
#[test]
fn a_saved_preset_is_a_chip_of_the_filter_dropdown() {
    use crate::dock::DockMessage;
    use crate::panels::{PanelMsg, PcbBoardMsg};
    use pcb_geom::PickKind;

    let mut app = board_app();
    app.ui_state.pcb_filter_presets.clear();
    let panel = |msg| Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg)));
    // Vias only, saved; the write task is left unrun so prefs.json stays.
    let _ = app.update(panel(PcbBoardMsg::FilterAllSet(false)));
    let _ = app.update(panel(PcbBoardMsg::FilterKindToggled(PickKind::Vias)));
    let _ = app.update(panel(PcbBoardMsg::FilterPresetAdded));
    let _ = app.update(panel(PcbBoardMsg::FilterAllSet(true)));
    assert_eq!(app.ui_state.pcb_filter_presets.len(), 1);

    let _ = app.update(bar_message(PcbBarMsg::MenuToggled(PcbBarMenu::Filter)));
    let texts = grid_texts(filter_grid(&app));
    assert_eq!(texts[2], "Filter 1", "presets follow All / None: {texts:?}");
    let applied = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "Filter 1");
    let _ = app.update(applied[0].clone());
    let on: Vec<PickKind> = app.ui_state.pcb_pick_filter.on_kinds().collect();
    assert_eq!(on, vec![PickKind::Vias]);
}

/// #2389: the Interactive Routing button arms the router on the board.
#[test]
fn the_routing_button_arms_interactive_routing() {
    let mut app = board_app();
    let button = button_for(&bar_of(&app), "place_board_route");
    assert!(button.enabled);
    assert!(
        button.tooltip.starts_with("Interactive Routing ("),
        "{}",
        button.tooltip
    );
    click(&mut app, "place_board_route");
    assert_eq!(
        app.interaction_state.pcb_tool.as_ref().map(|s| s.tool),
        Some(BoardPlaceTool::InteractiveRouting)
    );
}
