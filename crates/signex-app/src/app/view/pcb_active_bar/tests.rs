use uuid::Uuid;

use super::*;
use crate::app::pcb_tools::ToolState;

/// The button that runs `command` on a click.
pub(super) fn button_for<'a>(
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

pub(super) fn runs(button: &ActiveBarButton<Message>, command: &str) -> bool {
    matches!(
        &button.on_press,
        Some(Message::Menu(MenuMessage::Command(id))) if id.as_str() == command
    )
}

pub(super) fn buttons(items: &[ActiveBarItem<Message>]) -> Vec<&ActiveBarButton<Message>> {
    items
        .iter()
        .filter_map(|item| match item {
            ActiveBarItem::Button(button) => Some(button),
            _ => None,
        })
        .collect()
}

/// An app with an empty board open in its tab. Its groups start on their
/// first live rows, whatever another test left in the shared prefs file.
pub(super) fn board_app() -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.pcb_bar_last_tools = PcbBarLastTools::default();
    let board: signex_types::pcb::PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    let path = std::env::temp_dir().join("pcb_active_bar.snxpcb");
    app.open_pcb_tab(path, "board".into(), board);
    app
}

pub(super) fn bar_of(app: &Signex) -> Vec<ActiveBarItem<Message>> {
    pcb_bar_items(
        app.ui_state.effective_theme_id(),
        &app.ui_state.i18n,
        app.interaction_state.pcb_bar_menu,
        &app.ui_state.pcb_bar_last_tools,
        app.main_window_board_tool(),
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
/// `view_rotate`; mockup row 108: View is drawn with the clockwise arrow it turns by.
#[test]
fn the_pcb_bar_buttons_run_board_sketch_and_view_rotate() {
    let tid = ThemeId::GraphiteDark;
    let items = pcb_bar_items(tid, &I18n::en_us(), None, &PcbBarLastTools::default(), None);
    let sketch = button_for(&items, "board_sketch");
    let rotate = button_for(&items, "view_rotate");
    assert!(runs(sketch, "board_sketch") && sketch.enabled);
    assert!(runs(rotate, "view_rotate") && rotate.enabled);
    assert_eq!(rotate.tooltip, "Rotate View 90° Clockwise");
    let ActiveBarIcon::Svg(handle) = &rotate.icon else {
        panic!("an SVG icon");
    };
    assert_eq!(handle, &ic::icon_dd_rotate_cw(tid));
}

/// mockup row 108: the board tools are drawn with the board tool glyphs (via, pad,
/// dimension, keepout), as their dropdown rows are; no button on the bar is
/// a text glyph.
#[test]
fn the_board_tool_buttons_are_drawn_with_svg_glyphs() {
    let tid = ThemeId::GraphiteLight;
    let i18n = I18n::en_us();
    let first = pcb_bar_items(tid, &i18n, None, &PcbBarLastTools::default(), None);
    let mut last = PcbBarLastTools::default();
    assert!(last.remember(PcbBarGroup::ViaPad, "place_board_pad"));
    assert!(last.remember(PcbBarGroup::CopperAreas, "place_board_keepout"));
    let used = pcb_bar_items(tid, &i18n, None, &last, None);
    let cases = [
        (&first, "place_board_via", ic::icon_pcb_via(tid)),
        (&first, "place_board_dimension", ic::icon_pcb_dimension(tid)),
        (&used, "place_board_pad", ic::icon_pcb_pad(tid)),
        (&used, "place_board_keepout", ic::icon_pcb_keepout(tid)),
    ];
    for (items, command, glyph) in cases {
        let ActiveBarIcon::Svg(handle) = &button_for(items, command).icon else {
            panic!("{command}: an SVG icon");
        };
        assert_eq!(handle, &glyph, "{command}");
    }
    for items in [&first, &used] {
        assert!(
            buttons(items)
                .iter()
                .all(|button| matches!(button.icon, ActiveBarIcon::Svg(_))),
            "every button is an SVG glyph"
        );
    }
}

/// Round 9 of #2262: the button says what it is, in its tooltip and its
/// board glyph, not the generic rectangle.
#[test]
fn the_pcb_bar_button_is_named_and_drawn_as_board_sketch() {
    let tid = ThemeId::GraphiteDark;
    let items = pcb_bar_items(tid, &I18n::en_us(), None, &PcbBarLastTools::default(), None);
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

/// #2352, #2373, #2388, #2389, #2392, #2424, mockup row 108: Selection Filter, Move,
/// Align | Components | Route, Tune, Via/Pad, Copper Areas | Dimensions,
/// Text, Graphics | Board Sketch | View; no Copper Drawing group, as the
/// Graphics Line and Arc draw tracks on copper (#2869). Fabrication Outputs
/// is not on the bar (Caner, 2026-11-21). A group with a live row runs it; a
/// group with none is greyed with a tooltip that says it is not on boards yet.
#[test]
fn the_pcb_bar_has_the_group_layout_and_greys_what_boards_cannot_do() {
    let items = pcb_bar_items(
        ThemeId::GraphiteDark,
        &I18n::en_us(),
        None,
        &PcbBarLastTools::default(),
        None,
    );
    let all = buttons(&items);
    assert_eq!(all.len(), 13);
    assert!(
        !all.iter()
            .any(|b| runs(b, "place_board_track") || runs(b, "place_board_arc")),
        "#2869: no Copper Drawing flyout"
    );
    assert!(
        !all.iter().any(|b| runs(b, "fabrication_outputs")),
        "Fabrication Outputs stays in its menu"
    );
    let separators = items
        .iter()
        .filter(|item| matches!(item, ActiveBarItem::Separator))
        .count();
    assert_eq!(
        separators, 5,
        "mockup row 108: none between Tune and Via/Pad"
    );
    let separator_after = |index: usize| {
        let slot = items
            .iter()
            .position(|item| match item {
                ActiveBarItem::Button(button) => std::ptr::eq(button, all[index]),
                _ => false,
            })
            .expect("a slot");
        matches!(items.get(slot + 1), Some(ActiveBarItem::Separator))
    };
    let after: Vec<usize> = (0..all.len()).filter(|&i| separator_after(i)).collect();
    assert_eq!(
        after,
        [2, 3, 7, 10, 11],
        "Align | Components | Copper Areas | Graphics | Board Sketch |"
    );
    let live = all.iter().filter(|button| button.enabled).count();
    assert_eq!(
        live, 13,
        "#2373, #2389, #2424: every button, Tune, Dimensions, Text and Graphics included, is live"
    );
    assert!(runs(all[1], "move_object"), "Move follows the filter");
    assert!(
        runs(all[3], "change_footprint"),
        "Component is not live yet"
    );
    assert!(runs(all[4], "place_board_route"));
    assert!(runs(all[5], "tune_net_length"), "#2389: Tune follows Route");
    assert!(runs(all[6], "place_board_via"), "#2373: Via is live");
    assert!(
        runs(all[7], "place_board_polygon_pour"),
        "#2216: Polygon Pour heads its group"
    );
    assert!(runs(all[8], "place_board_dimension"), "#2373");
    assert!(runs(all[9], "place_board_string"), "#2373");
    assert!(
        runs(all[10], "place_board_line"),
        "#2424: Line heads Graphics"
    );
    assert!(runs(all[11], "board_sketch"));
    assert!(runs(all[12], "view_rotate"), "View closes the bar");
    for button in all.iter().filter(|button| !button.enabled) {
        assert!(button.on_press.is_none() && button.on_right_press.is_none());
        assert!(
            button.tooltip.ends_with(": not available on boards yet"),
            "{}",
            button.tooltip
        );
    }
    assert!(all[0].tooltip.starts_with("Selection Filter"));
    assert!(all[4].tooltip.starts_with("Interactive Routing"));
    assert_eq!(all[5].tooltip, "Length Tuning");
    assert!(all[8].tooltip.starts_with("Place Aligned Dimension"), "#2507");
    assert!(all[9].tooltip.starts_with("Place String"));
}

/// #2352 / #2360 / #2392: every tool the Place menu greys on a board is
/// greyed on the bar too, as a button or a dropdown row, with the same
/// reason, and the bar greys nothing else for that reason.
#[test]
fn the_bar_greys_the_place_menus_board_tools() {
    let app = board_app();
    let i18n = I18n::en_us();
    let items = pcb_bar_items(
        ThemeId::GraphiteDark,
        &i18n,
        None,
        &PcbBarLastTools::default(),
        None,
    );
    let mut greyed: Vec<String> = buttons(&items)
        .into_iter()
        .filter(|button| !button.enabled)
        .map(|button| button.tooltip.clone())
        .collect();
    for group in PcbBarGroup::ALL {
        greyed.extend(
            rows(&app, PcbBarMenu::Group(group))
                .into_iter()
                .filter(|row| row.disabled)
                .filter_map(|row| row.tooltip),
        );
    }
    greyed.sort();
    greyed.dedup();
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

/// #2388: the Fabrication Outputs command, which its menu row runs, opens
/// the dialog on the board in view and writes nothing.
#[test]
fn the_fabrication_outputs_command_opens_its_dialog() {
    let mut app = board_app();
    let _ = app.update(run("fabrication_outputs").expect("a catalog command"));
    let state = app
        .ui_state
        .fab_outputs
        .as_ref()
        .expect("the dialog is open");
    assert!(state.board.ends_with("pcb_active_bar.snxpcb"));
    assert_eq!(state.name, "pcb_active_bar");
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

/// The entries of `menu` as the bar builds them for `app`.
pub(super) fn entries(app: &Signex, menu: PcbBarMenu) -> Vec<DropdownEntry<Message>> {
    let ctx = app.command_context(None);
    let sources = MenuSources {
        ctx: &ctx,
        filter: &app.ui_state.pcb_pick_filter,
        presets: &app.ui_state.pcb_filter_presets,
        editing: app.ui_state.pcb_filter_preset_editing,
        last: &app.ui_state.pcb_bar_last_tools,
    };
    pcb_bar_menu_entries(
        menu,
        &app.ui_state.i18n,
        &app.document_state.panel_ctx.tokens,
        app.ui_state.effective_theme_id(),
        &sources,
    )
}

/// The item rows of `menu`, headers and separators left out.
pub(super) fn rows(app: &Signex, menu: PcbBarMenu) -> Vec<DropdownItem<Message>> {
    entries(app, menu)
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownEntry::Item(item) => Some(item),
            _ => None,
        })
        .collect()
}

/// Picks the row of `menu` that runs `command`, from an open dropdown.
pub(super) fn pick(app: &mut Signex, menu: PcbBarMenu, command: &str) {
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

/// mockup row 108: each View row's arrow turns the way the row says; the half turn
/// uses the clockwise arrow and Reset has no icon, so the two no longer
/// share one.
#[test]
fn the_view_dropdown_icons_follow_the_turn() {
    let app = board_app();
    let tid = app.ui_state.effective_theme_id();
    let icon_of = |command: &str| {
        rows(&app, PcbBarMenu::View)
            .into_iter()
            .find(|row| row.command_id == Some(command))
            .unwrap_or_else(|| panic!("a {command} row"))
            .icon
    };
    assert_eq!(icon_of("view_rotate"), Some(ic::icon_dd_rotate_cw(tid)));
    assert_eq!(icon_of("view_rotate_ccw"), Some(ic::icon_dd_rotate(tid)));
    assert_eq!(icon_of("view_rotate_180"), Some(ic::icon_dd_rotate_cw(tid)));
    assert_eq!(icon_of("view_rotate_reset"), None);
}

/// The board Selection Filter dropdown, laid out: its single chip grid.
fn filter_grid(app: &Signex) -> iced::Element<'static, Message> {
    let mut grid = entries(app, PcbBarMenu::Filter);
    assert_eq!(grid.len(), 1, "one chip grid, as on the schematic");
    match grid.remove(0) {
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
    let tokens = app.document_state.panel_ctx.tokens;
    let schematic = crate::active_bar::filter_menu_entry(
        &tokens,
        crate::active_bar::FilterMenu {
            chips: Vec::new(),
            presets: Vec::new(),
            editing: None,
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

    // 3D bodies cannot be picked yet: its chip sends nothing.
    let bodies = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "3D Bodies");
    assert!(bodies.is_empty(), "{bodies:?}");
    // Rooms are picked since #2457: its chip toggles the kind.
    let rooms = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "Rooms");
    assert_eq!(rooms.len(), 1, "{rooms:?}");

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
    assert!(!texts.iter().any(|t| t.starts_with("Editing")), "{texts:?}");
    let applied = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "Filter 1");
    let _ = app.update(applied[0].clone());
    let on: Vec<PickKind> = app.ui_state.pcb_pick_filter.on_kinds().collect();
    assert_eq!(on, vec![PickKind::Vias]);
    // #2873: the click selects the preset for editing, and the dropdown
    // says so; a second click on it ends editing.
    let texts = grid_texts(filter_grid(&app));
    assert!(texts.iter().any(|t| t == "Editing Filter 1"), "{texts:?}");
    let again = crate::layout_probe::messages_on_click(filter_grid(&app), WINDOW, "Filter 1");
    let _ = app.update(again[0].clone());
    assert_eq!(app.ui_state.pcb_filter_preset_editing, None);
    let texts = grid_texts(filter_grid(&app));
    assert!(!texts.iter().any(|t| t.starts_with("Editing")), "{texts:?}");
}

/// mockup row 108: the Selection Filter button carries a badge with the number of
/// pickable kinds that are off, at its top-right corner; there is none while
/// every kind is on, and a kind boards cannot pick yet does not count.
#[test]
fn the_filter_badge_counts_the_kinds_that_are_off() {
    use pcb_geom::{PickFilter, PickKind};

    let mut app = board_app();
    app.ui_state.pcb_pick_filter = PickFilter::default();
    let bar_texts = |app: &Signex| {
        crate::layout_probe::lay_out_texts(app.pcb_active_bar(true), WINDOW)
            .into_iter()
            .map(|t| (t.text.replace(['\u{2068}', '\u{2069}'], ""), t.bounds))
            .collect::<Vec<_>>()
    };
    assert!(
        bar_texts(&app).is_empty(),
        "no badge while every kind is on"
    );

    app.ui_state.pcb_pick_filter.toggle(PickKind::Tracks);
    app.ui_state.pcb_pick_filter.toggle(PickKind::Vias);
    let texts = bar_texts(&app);
    let labels: Vec<&str> = texts.iter().map(|(text, _)| text.as_str()).collect();
    assert_eq!(labels, ["2"]);

    // The badge sits in the right half of the filter button, near its top.
    let m = signex_widgets::active_bar::Metrics::GRAPHITE;
    let items = bar_of(&app);
    let (offsets, bar_width) = signex_widgets::active_bar::slot_offsets(&items, &m);
    let slot_left = (WINDOW.width - bar_width) / 2.0 + offsets[0];
    let top = crate::active_bar::bar_top_margin() + m.padding;
    let badge = texts[0].1;
    assert!(badge.x >= slot_left + m.btn / 2.0, "{badge:?}");
    assert!(badge.x + badge.width <= slot_left + m.btn, "{badge:?}");
    assert!(badge.y >= top && badge.y < top + m.btn / 2.0, "{badge:?}");

    // Rooms are pickable (#2457), so turning them off counts.
    app.ui_state.pcb_pick_filter.toggle(PickKind::Rooms);
    let labels: Vec<String> = bar_texts(&app).into_iter().map(|(text, _)| text).collect();
    assert_eq!(labels, ["3"], "Rooms can be picked");

    // 3D bodies cannot be picked yet: turning them off changes nothing.
    app.ui_state.pcb_pick_filter.toggle(PickKind::Bodies3d);
    let labels: Vec<String> = bar_texts(&app).into_iter().map(|(text, _)| text).collect();
    assert_eq!(labels, ["3"], "3D bodies cannot be picked yet");
}

/// #2389: the Interactive Routing button arms the router on the board.
#[test]
fn the_routing_button_arms_interactive_routing() {
    let mut app = board_app();
    let items = bar_of(&app);
    let button = button_for(&items, "place_board_route");
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
