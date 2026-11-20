//! The PCB Active Bar's tool groups (#2392), driven through `Signex::update`.

use super::tests::{bar_of, board_app, button_for, buttons, rows, runs};
use super::*;
use crate::app::pcb_bar_groups::PcbBarRow;

/// The button whose right-click opens `group`'s dropdown.
fn group_button_of(app: &Signex, group: PcbBarGroup) -> ActiveBarButton<Message> {
    bar_of(app)
        .into_iter()
        .find(|item| opens(item, PcbBarMenu::Group(group)))
        .and_then(|item| match item {
            ActiveBarItem::Button(button) => Some(button),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a {group:?} button with a dropdown"))
}

/// The tool armed on the board in view.
fn armed(app: &Signex) -> Option<BoardPlaceTool> {
    app.interaction_state
        .pcb_tool
        .as_ref()
        .map(|session| session.tool)
}

/// Runs `task` as the iced runtime would, so a preference write lands.
fn run_task(task: iced::Task<Message>) {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async { while stream.next().await.is_some() {} });
}

/// Opens `group`'s dropdown with a right-click on its button and runs the
/// row that runs `command`. The task the pick returned.
fn pick_from_group(app: &mut Signex, group: PcbBarGroup, command: &str) -> iced::Task<Message> {
    let open = group_button_of(app, group)
        .on_right_press
        .expect("a dropdown");
    let _ = app.update(open);
    assert_eq!(
        app.interaction_state.pcb_bar_menu,
        Some(PcbBarMenu::Group(group))
    );
    assert!(group_button_of(app, group).selected, "drawn open");
    let row = rows(app, PcbBarMenu::Group(group))
        .into_iter()
        .find(|row| row.command_id == Some(command))
        .unwrap_or_else(|| panic!("a {command} row"));
    let task = app.update(row.on_press.expect("a live row"));
    assert_eq!(
        app.interaction_state.pcb_bar_menu, None,
        "the pick closes it"
    );
    task
}

/// Every group sits on the bar once, in the table's order.
#[test]
fn the_bar_shows_every_group_once() {
    let laid_out: Vec<PcbBarGroup> = BAR_GROUPS
        .iter()
        .flat_map(|run| run.iter().copied())
        .collect();
    assert_eq!(laid_out, PcbBarGroup::ALL);
}

/// #2392: each group with more than one row opens its dropdown on a
/// right-click; a row runs its tool, which is armed on the board, and the
/// group's button shows and runs that tool next.
#[test]
fn each_group_opens_its_dropdown_and_a_row_arms_its_tool() {
    let cases = [
        // #2389: every routing tool is a row of Route.
        (PcbBarGroup::Route, BoardPlaceTool::DiffPairRouting),
        (PcbBarGroup::Route, BoardPlaceTool::QuickRoute),
        (PcbBarGroup::Route, BoardPlaceTool::MultiRoute),
        (PcbBarGroup::Route, BoardPlaceTool::InteractiveRouting),
        (PcbBarGroup::ViaPad, BoardPlaceTool::Via),
        (PcbBarGroup::CopperAreas, BoardPlaceTool::Fill),
        (PcbBarGroup::CopperAreas, BoardPlaceTool::Keepout),
        (PcbBarGroup::CopperDrawing, BoardPlaceTool::Track),
        // #2424: the Graphics group.
        (PcbBarGroup::Graphics, BoardPlaceTool::GraphicArc),
        (PcbBarGroup::Graphics, BoardPlaceTool::TextFrame),
    ];
    let mut app = board_app();
    for (group, tool) in cases {
        let command = tool.command().expect("a live tool");
        let _ = pick_from_group(&mut app, group, command);
        assert_eq!(armed(&app), Some(tool), "{command} arms its tool");
        assert!(runs(&group_button_of(&app, group), command), "{command}");
    }
}

/// #2392: while one of its tools is armed a group's button is drawn
/// selected, as on the schematic bar; ending the tool clears it.
#[test]
fn a_group_is_drawn_selected_while_its_tool_is_armed() {
    let mut app = board_app();
    assert!(!group_button_of(&app, PcbBarGroup::CopperAreas).selected);
    let _ = pick_from_group(&mut app, PcbBarGroup::CopperAreas, "place_board_fill");
    assert_eq!(app.interaction_state.pcb_bar_menu, None);
    assert!(group_button_of(&app, PcbBarGroup::CopperAreas).selected);
    assert!(!group_button_of(&app, PcbBarGroup::ViaPad).selected);
    let _ = pick_from_group(&mut app, PcbBarGroup::ViaPad, "place_board_via");
    assert!(group_button_of(&app, PcbBarGroup::ViaPad).selected);
    assert!(!group_button_of(&app, PcbBarGroup::CopperAreas).selected);
    app.end_board_tool();
    assert_eq!(armed(&app), None);
    assert!(!group_button_of(&app, PcbBarGroup::ViaPad).selected);
}

/// #2392: the Components group lists Component and Change Footprint; the
/// swap is greyed by its menu gate while nothing is selected.
#[test]
fn the_components_group_lists_the_part_rows() {
    let app = board_app();
    let _ = group_button_of(&app, PcbBarGroup::Components);
    let rows = rows(&app, PcbBarMenu::Group(PcbBarGroup::Components));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].label, "Component");
    assert_eq!(rows[1].command_id, Some("change_footprint"));
    assert!(rows[1].disabled, "nothing is selected");
}

/// #2389: the Route group lists every routing tool and the Tune group both
/// tuning commands, each with its own icon; a Tune row runs its command
/// (with no track selected it only says so) and the group shows it next.
#[test]
fn the_route_and_tune_groups_list_their_rows() {
    let mut app = board_app();
    let route = rows(&app, PcbBarMenu::Group(PcbBarGroup::Route));
    let labels: Vec<&str> = route.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Interactive Routing",
            "Differential Pair Routing",
            "Quick Route",
            "Multi-Route"
        ]
    );
    assert!(route.iter().all(|row| !row.disabled && row.icon.is_some()));
    let tune = rows(&app, PcbBarMenu::Group(PcbBarGroup::Tune));
    let commands: Vec<_> = tune.iter().map(|row| row.command_id).collect();
    assert_eq!(commands, [Some("tune_net_length"), Some("tune_diff_pair")]);
    assert_eq!(tune[0].label, "Length Tuning");
    assert_eq!(tune[1].label, "Differential Pair Length Tuning");
    assert_ne!(tune[0].icon, tune[1].icon, "each row has its own glyph");
    let path = app.active_tab_path().expect("a board");
    let before = serde_json::to_string(app.document_state.pcb_board(&path).unwrap()).unwrap();
    let _ = pick_from_group(&mut app, PcbBarGroup::Tune, "tune_diff_pair");
    let after = serde_json::to_string(app.document_state.pcb_board(&path).unwrap()).unwrap();
    assert_eq!(before, after, "nothing selected: nothing tuned");
    assert!(runs(
        &group_button_of(&app, PcbBarGroup::Tune),
        "tune_diff_pair"
    ));
}

/// #2392: a row whose tool is not built yet is greyed, runs nothing, and
/// its tooltip says why, the reason Place on a board gives.
#[test]
fn a_tool_not_built_yet_is_a_greyed_row_with_its_reason() {
    let app = board_app();
    // #2373: Pad and Arc are live; Component is the tool still to build.
    let cases = [(PcbBarGroup::Components, "Component", "Place Component")];
    for (group, label, reason) in cases {
        let row = rows(&app, PcbBarMenu::Group(group))
            .into_iter()
            .find(|row| row.label == label)
            .unwrap_or_else(|| panic!("a {label} row"));
        assert!(row.disabled && row.on_press.is_none(), "{label}");
        assert_eq!(
            row.tooltip.as_deref(),
            Some(format!("{reason}: not available on boards yet").as_str())
        );
        assert!(row.icon.is_some(), "{label} has an icon");
    }
}

/// #2392, #2373: a group of one row is one button with no dropdown; the
/// Dimensions and Text groups run their tool.
#[test]
fn a_group_of_one_row_is_a_button_with_no_dropdown() {
    let app = board_app();
    let items = bar_of(&app);
    for (group, reason) in [
        (PcbBarGroup::Dimensions, "Place Dimension"),
        (PcbBarGroup::Text, "Place String"),
    ] {
        assert_eq!(group.rows().len(), 1, "{group:?}");
        let button = buttons(&items)
            .into_iter()
            .find(|button| button.tooltip.starts_with(reason))
            .unwrap_or_else(|| panic!("a {group:?} button"));
        assert!(button.enabled, "{group:?} is live");
        assert!(button.on_right_press.is_none(), "{group:?} has no dropdown");
    }
}

/// #2392: a tool armed from the Place menu (or a key, or the palette) is
/// what its group's button shows next.
#[test]
fn a_tool_run_from_place_becomes_its_groups_button() {
    let mut app = board_app();
    let keepout = AppCommandId::new("place_board_keepout").expect("a command");
    let _ = app.update(Message::Menu(MenuMessage::Command(keepout)));
    assert_eq!(armed(&app), Some(BoardPlaceTool::Keepout));
    assert_eq!(
        app.ui_state
            .pcb_bar_last_tools
            .current(PcbBarGroup::CopperAreas),
        PcbBarRow::Tool(BoardPlaceTool::Keepout)
    );
    let _ = button_for(&bar_of(&app), "place_board_keepout");
}

/// #2392: the group keeps its last tool across a restart: the pick's task
/// writes it to `prefs.json` and a new session's bar shows it.
#[test]
fn a_group_remembers_its_last_tool_after_a_restart() {
    let mut app = board_app();
    let task = pick_from_group(&mut app, PcbBarGroup::CopperDrawing, "place_board_track");
    run_task(task);
    let task = pick_from_group(&mut app, PcbBarGroup::ViaPad, "place_board_via");
    run_task(task);
    let task = pick_from_group(&mut app, PcbBarGroup::CopperAreas, "place_board_fill");
    run_task(task);
    assert_eq!(
        crate::fonts::read_pcb_bar_last_tools().current(PcbBarGroup::CopperAreas),
        PcbBarRow::Tool(BoardPlaceTool::Fill),
        "the task wrote it"
    );

    let (restarted, _task) = Signex::new();
    assert_eq!(
        restarted
            .ui_state
            .pcb_bar_last_tools
            .current(PcbBarGroup::CopperAreas),
        PcbBarRow::Tool(BoardPlaceTool::Fill)
    );
    let button = group_button_of(&restarted, PcbBarGroup::CopperAreas);
    assert!(runs(&button, "place_board_fill"));
}

/// #2216: Copper Areas lists Pour Cutout and Slice Pour after Polygon Pour
/// and ends with Repour Selected and Repour All; every row is live on a
/// board, and Pour Cutout picked from it is armed.
#[test]
fn the_copper_areas_group_lists_the_pour_rows() {
    let mut app = board_app();
    let ids: Vec<Option<&str>> = rows(&app, PcbBarMenu::Group(PcbBarGroup::CopperAreas))
        .iter()
        .map(|row| row.command_id)
        .collect();
    assert_eq!(
        ids,
        [
            Some("place_board_polygon_pour"),
            Some("place_board_pour_cutout"),
            Some("place_board_slice_pour"),
            Some("place_board_region"),
            Some("place_board_fill"),
            Some("place_board_keepout"),
            Some("place_board_room"),
            Some("repour_selected"),
            Some("repour_all"),
        ]
    );
    let labels: Vec<String> = rows(&app, PcbBarMenu::Group(PcbBarGroup::CopperAreas))
        .into_iter()
        .filter(|row| !row.disabled)
        .map(|row| row.label)
        .collect();
    assert_eq!(labels.len(), 9, "every row is live: {labels:?}");
    assert!(labels.iter().any(|l| l == "Pour Cutout"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Repour All"), "{labels:?}");
    let _ = pick_from_group(
        &mut app,
        PcbBarGroup::CopperAreas,
        "place_board_pour_cutout",
    );
    assert_eq!(armed(&app), Some(BoardPlaceTool::PourCutout));
}

/// A click on the canvas of the board in view at (`x`, `y`) mm.
fn canvas_click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(
        crate::pcb_canvas::PcbCanvasMsg::Clicked {
            at: signex_types::pcb::Point::new(x, y),
            tolerance_mm: 0.05,
            mode: pcb_geom::ClickMode::Replace,
        },
    ));
}

/// The pointer moved over the canvas to (`x`, `y`) mm.
fn canvas_hover(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(
        crate::pcb_canvas::PcbCanvasMsg::PointerMoved {
            at: signex_types::pcb::Point::new(x, y),
            zoom_pct: 100.0,
            tolerance_mm: 0.05,
        },
    ));
}

/// The maintainer's report (2026-11-18): Interactive Routing picked from
/// the Route dropdown routes from a pad. The click on the pad starts the
/// route on its net, the leg follows the pointer, a click adds it and Esc
/// lands it as one undo step on the pad's net and layer.
#[test]
fn the_route_row_routes_from_a_pad_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let board: signex_types::pcb::PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}],
        "footprints": [{
            "uuid": uuid::Uuid::from_u128(2), "reference": "U1", "value": "",
            "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
            "position": {"x": 0.0, "y": 0.0},
            "pads": [{
                "uuid": uuid::Uuid::from_u128(3), "number": "1", "pad_type": "smd",
                "shape": "circle", "position": {"x": 0.0, "y": 0.0},
                "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                "net": {"number": 1, "name": "A"}
            }]
        }]
    }))
    .unwrap();
    let workspace = dir.path().to_string_lossy();
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(&workspace, &[]);
    app.ui_state.pcb_bar_last_tools = PcbBarLastTools::default();
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);

    let _ = pick_from_group(&mut app, PcbBarGroup::Route, "place_board_route");
    assert_eq!(armed(&app), Some(BoardPlaceTool::InteractiveRouting));
    assert!(app.board_tool_status().is_some(), "the status bar says so");
    canvas_click(&mut app, 0.0, 0.0);
    canvas_hover(&mut app, 5.0, 3.0);
    assert!(app.interaction_state.pcb_canvas.has_part_ghost());
    canvas_click(&mut app, 5.0, 3.0);
    app.clear_pcb_selection();

    let tracks = &app.document_state.pcb_board(&path).expect("board").segments;
    assert!(!tracks.is_empty(), "the route landed");
    let on_pad_net = |t: &signex_types::pcb::Segment| t.net == 1 && t.layer == "TopCopper";
    assert!(tracks.iter().all(on_pad_net), "{tracks:?}");
    let _ = app.update(Message::Edit(crate::app::EditMsg::Undo));
    let board = app.document_state.pcb_board(&path).expect("board");
    assert!(board.segments.is_empty(), "one undo removes the route");
}
