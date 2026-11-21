//! #2460: the board counters read the cached counts, the last DRC run and
//! the window's selection; each popover opens and closes per window, and its
//! actions act on that window's board.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use signex_types::pcb::{
    Footprint, LayerDef, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment,
};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::state::DrcInput;
use crate::app::{FileMsg, InputMsg, Message, Signex, StatusBarRequest, UiMsg};
use crate::status_bar::{
    BoardCounter, BoardPopoverEvent, BoardPopoverKind, BoardPopoverModel, ConnectionTally, DrcAge,
    DrcTally,
};

const C1: Uuid = Uuid::from_u128(0x100);

/// A 1 mm pad on net VIN at its part's origin.
fn pad(n: u128) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "VIN".into(),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn part(id: Uuid, reference: &str, x: f64, pad_id: u128) -> Footprint {
    Footprint {
        uuid: id,
        reference: reference.into(),
        value: "100n".into(),
        footprint_id: "C_0402".into(),
        position: Point::new(x, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad(pad_id)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// C1, U1 and J1 on net VIN at x = 0, 10 and 20; a track joins C1 and U1,
/// so one of the two connections is routed.
fn board() -> PcbBoard {
    let layer = |id: u8, name: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: vec![NetDef {
            number: 1,
            name: "VIN".into(),
        }],
        footprints: vec![
            part(C1, "C1", 0.0, 1),
            part(Uuid::from_u128(0x200), "U1", 10.0, 2),
            part(Uuid::from_u128(0x300), "J1", 20.0, 3),
        ],
        segments: vec![Segment {
            locked: false,
            attributes: Default::default(),
            uuid: Uuid::from_u128(0x400),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            width: 0.25,
            layer: "TopCopper".into(),
            net: 1,
            unknown_columns: Default::default(),
        }],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
        free: Default::default(),
    }
}

/// The board open in the main window, its ratsnest and counts computed.
fn app_with_board() -> (Signex, PathBuf, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let path = PathBuf::from("/w/counters.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "counters".to_string(),
        result: Ok(Box::new(board())),
    }));
    assert!(app.has_active_pcb());
    app.sync_pcb_canvas_from_visible_board();
    (app, path, main)
}

fn click(app: &mut Signex, window: iced::window::Id, counter: BoardCounter) {
    let request = StatusBarRequest::BoardCounterClicked {
        window: None,
        counter,
    };
    let _ = app.update(Signex::status_bar_message(window, None, request));
}

fn event(app: &mut Signex, event: BoardPopoverEvent) {
    let _ = app.update(Message::Ui(UiMsg::StatusBar(
        StatusBarRequest::BoardPopover(event),
    )));
}

fn open_kind(app: &Signex) -> Option<(iced::window::Id, BoardPopoverKind)> {
    let popover = app.ui_state.board_status.popover.as_ref()?;
    Some((popover.window, popover.kind))
}

#[test]
fn the_counters_read_the_cached_connection_counts() {
    let (app, _path, main) = app_with_board();
    let board = app.status_board(main).expect("a board");
    assert_eq!(
        board.connections,
        Some(ConnectionTally {
            routed: 1,
            total: 2,
            unrouted: 1,
        })
    );
    assert_eq!(board.drc, DrcTally::NotRun);
    assert_eq!(board.selected, 0);
    assert_eq!(board.open, None);
}

#[test]
fn a_counter_click_opens_its_popover_and_a_second_click_or_esc_closes_it() {
    let (mut app, _path, main) = app_with_board();
    click(&mut app, main, BoardCounter::Drc);
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::Drc)));
    assert_eq!(
        app.status_board(main).unwrap().open,
        Some(BoardPopoverKind::Drc)
    );
    click(&mut app, main, BoardCounter::Drc);
    assert_eq!(open_kind(&app), None, "a second click closes it");
    // Routed and Unrouted open the one Unrouted popover.
    click(&mut app, main, BoardCounter::Routed);
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::Unrouted)));
    click(&mut app, main, BoardCounter::Selected);
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::Selection)));
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert_eq!(open_kind(&app), None, "Esc closes it");
    click(&mut app, main, BoardCounter::Grid);
    event(&mut app, BoardPopoverEvent::Dismissed);
    assert_eq!(open_kind(&app), None, "a click outside closes it");
}

/// A popover of another window is not drawn in this one, and a click in
/// this one moves it here.
#[test]
fn a_popover_belongs_to_the_window_that_opened_it() {
    let (mut app, _path, main) = app_with_board();
    let other = iced::window::Id::unique();
    app.ui_state.board_status.popover = Some(crate::status_bar::BoardStatusPopover::new(
        other,
        BoardPopoverKind::Layer,
    ));
    assert!(app.board_popover_model(main).is_none());
    assert!(app.board_popover_card(main).is_none());
    click(&mut app, main, BoardCounter::Layer);
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::Layer)));
    assert!(matches!(
        app.board_popover_model(main),
        Some(BoardPopoverModel::Layer(_))
    ));
    assert!(app.board_popover_card(main).is_some());
}

#[test]
fn a_layer_row_makes_that_layer_active() {
    let (mut app, path, main) = app_with_board();
    click(&mut app, main, BoardCounter::Layer);
    let Some(BoardPopoverModel::Layer(list)) = app.board_popover_model(main) else {
        panic!("the Active layer popover");
    };
    let names: Vec<PcbLayer> = list.rows.iter().map(|row| row.layer).collect();
    assert!(names.contains(&PcbLayer::Copper(CopperLayer::Bottom)));
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    event(&mut app, BoardPopoverEvent::LayerPicked(bottom));
    assert_eq!(app.pcb_layer_view(&path).active(), bottom);
    assert_eq!(
        open_kind(&app),
        Some((main, BoardPopoverKind::Layer)),
        "stays open"
    );
}

#[test]
fn a_grid_preset_or_custom_pitch_sets_the_grid_and_a_bad_one_is_refused() {
    let (mut app, _path, main) = app_with_board();
    click(&mut app, main, BoardCounter::Grid);
    event(&mut app, BoardPopoverEvent::GridPresetPicked(0.5));
    assert_eq!(app.status_grid(main).size_mm, 0.5);
    event(&mut app, BoardPopoverEvent::CustomGridEdited("0.15".into()));
    event(&mut app, BoardPopoverEvent::CustomGridSubmitted);
    assert_eq!(app.status_grid(main).size_mm, 0.15);
    for bad in ["0", "-1", "abc"] {
        event(&mut app, BoardPopoverEvent::CustomGridEdited(bad.into()));
        event(&mut app, BoardPopoverEvent::CustomGridSubmitted);
        assert_eq!(app.status_grid(main).size_mm, 0.15, "{bad} changes nothing");
        let popover = app.ui_state.board_status.popover.as_ref().unwrap();
        assert!(popover.custom_refused, "{bad} is refused");
    }
    event(&mut app, BoardPopoverEvent::CustomGridEdited("1".into()));
    assert!(
        !app.ui_state
            .board_status
            .popover
            .as_ref()
            .unwrap()
            .custom_refused
    );
    let snap = app.ui_state.snap_enabled;
    event(&mut app, BoardPopoverEvent::SnapToggled);
    assert_eq!(app.ui_state.snap_enabled, !snap);
}

/// The DRC counter says how long ago the last run landed; its popover, the
/// board's edits since.
#[test]
fn the_drc_counter_ages_and_counts_the_edits_since() {
    let (mut app, path, main) = app_with_board();
    let revision = app.board_revision(&path);
    let report = signex_drc::DrcReport {
        rules: signex_drc::DrcRules::new(1),
        violations: Vec::new(),
        skipped: Vec::new(),
        skipped_keepouts: Vec::new(),
    };
    let landed = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    let drc = &mut app.interaction_state.pcb_drc;
    let generation = drc.begin(DrcInput {
        path: path.clone(),
        revision,
        net_classes: Vec::new(),
    });
    assert!(drc.finish(generation, report, BTreeMap::new(), landed));
    app.ui_state.board_status.now = Some(landed + Duration::from_secs(20));
    let tally = |app: &Signex| app.status_board(main).unwrap().drc;
    assert_eq!(
        tally(&app),
        DrcTally::Ran {
            violations: 0,
            age: DrcAge::JustNow
        }
    );
    app.ui_state.board_status.now = Some(landed + Duration::from_secs(150));
    assert_eq!(
        tally(&app),
        DrcTally::Ran {
            violations: 0,
            age: DrcAge::Minutes(2)
        }
    );
    // The tick stores the clock; the view never reads it.
    let before = app.ui_state.board_status.now;
    let _ = app.update(Message::Ui(UiMsg::StatusBar(
        StatusBarRequest::BoardClockTicked,
    )));
    assert_ne!(app.ui_state.board_status.now, before);
    // Two edits later.
    app.document_state
        .pcb_revisions
        .insert(path.clone(), revision + 2);
    click(&mut app, main, BoardCounter::Drc);
    let Some(BoardPopoverModel::Drc(list)) = app.board_popover_model(main) else {
        panic!("the DRC popover");
    };
    let (time, edits) = list.last_run.expect("a run");
    assert_eq!(edits, 2);
    assert_eq!(time.len(), "14:12:58".len());
}

#[test]
fn clear_deselects_and_closes_the_popover() {
    let (mut app, _path, main) = app_with_board();
    app.select_pcb_items(&[pcb_geom::ItemRef::Footprint(C1)]);
    assert_eq!(app.status_board(main).unwrap().selected, 1);
    click(&mut app, main, BoardCounter::Selected);
    let Some(BoardPopoverModel::Selection(list)) = app.board_popover_model(main) else {
        panic!("the Selection popover");
    };
    assert_eq!(list.count, 1);
    assert!(!list.linked, "a loose board has no schematic side");
    event(&mut app, BoardPopoverEvent::SelectionCleared);
    assert_eq!(app.status_board(main).unwrap().selected, 0);
    assert_eq!(open_kind(&app), None);
}

/// Route next frames the first unrouted connection and arms Interactive
/// Routing on the board in view.
#[test]
fn route_next_frames_the_connection_and_arms_routing() {
    let (mut app, _path, main) = app_with_board();
    click(&mut app, main, BoardCounter::Unrouted);
    let Some(BoardPopoverModel::Unrouted(list)) = app.board_popover_model(main) else {
        panic!("the Unrouted popover");
    };
    assert_eq!(list.unrouted, 1);
    assert_eq!(list.nets.len(), 1);
    assert_eq!(list.nets[0].name, "VIN");
    app.interaction_state.pcb_canvas.pending_fit.set(None);
    event(&mut app, BoardPopoverEvent::RouteNextRequested);
    assert!(app.interaction_state.pcb_canvas.pending_fit.get().is_some());
    let armed = app.interaction_state.pcb_tool.as_ref().map(|s| s.tool);
    assert_eq!(armed, Some(BoardPlaceTool::InteractiveRouting));
    assert_eq!(open_kind(&app), None);
}

// ─── The layer switcher preference (Caner, 2026-11-21) ───────────────────

/// What a left click on the Layer pill of `window`'s bar sends.
fn pill_click(app: &Signex, window: iced::window::Id) -> Vec<Message> {
    let caption = signex_i18n::t!(app.ui_state.i18n, signex_i18n::ids::PCB_STATUS_LAYER);
    crate::layout_probe::messages_on_click(
        app.view_status_bar(window),
        iced::Size::new(1600.0, 40.0),
        &caption.to_string(),
    )
}

fn prefs(msg: crate::preferences::PrefMsg) -> Message {
    Message::Preferences(crate::app::PreferencesMsg::Inner(msg))
}

#[test]
fn the_layer_switcher_defaults_to_strip_and_round_trips() {
    use crate::fonts::{read_layer_switcher_pref_from, write_layer_switcher_pref_at};
    use crate::layer_strip_prefs::LayerSwitcher;
    assert_eq!(read_layer_switcher_pref_from(None), LayerSwitcher::Strip);
    let junk = serde_json::json!({ "pcb_layer_switcher": "sideways" });
    assert_eq!(
        read_layer_switcher_pref_from(Some(&junk)),
        LayerSwitcher::Strip
    );
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("prefs.json");
    write_layer_switcher_pref_at(&path, LayerSwitcher::Dropdown);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        read_layer_switcher_pref_from(Some(&saved)),
        LayerSwitcher::Dropdown
    );
}

#[test]
fn under_strip_the_layer_pill_is_a_label_that_does_not_step() {
    use crate::layer_strip_prefs::LayerSwitcher;
    let (mut app, path, main) = app_with_board();
    app.ui_state.layer_switcher = LayerSwitcher::Strip;
    let before = app.pcb_layer_view(&path).active();
    assert!(app.shows_pcb_layer_strip());
    assert!(pill_click(&app, main).is_empty(), "a click sends nothing");
    assert_eq!(app.pcb_layer_view(&path).active(), before);
    assert_eq!(open_kind(&app), None);
}

/// Saved in Preferences it applies at once: the strip goes, the pill opens
/// the grouped list, a pick sets the layer and closes it, and Esc closes it.
#[test]
fn under_dropdown_the_pill_opens_the_list_and_a_pick_sets_the_layer() {
    use crate::app::PreferencesMsg;
    use crate::layer_strip_prefs::LayerSwitcher;
    use crate::preferences::PrefMsg;
    let (mut app, path, main) = app_with_board();
    app.ui_state.layer_switcher = LayerSwitcher::Strip;
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(prefs(PrefMsg::DraftLayerSwitcher(LayerSwitcher::Dropdown)));
    assert!(app.ui_state.preferences_draft_differs());
    assert!(app.shows_pcb_layer_strip(), "a draft changes nothing");
    let _ = app.update(prefs(PrefMsg::Save));
    let _ = app.update(prefs(PrefMsg::DiscardAndClose));
    assert_eq!(app.ui_state.layer_switcher, LayerSwitcher::Dropdown);
    assert!(!app.shows_pcb_layer_strip(), "the strip is gone");

    let clicked = pill_click(&app, main);
    assert_eq!(clicked.len(), 1, "the pill is a button");
    let _ = app.update(clicked[0].clone());
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::Layer)));
    let Some(BoardPopoverModel::LayerDropdown(groups)) = app.board_popover_model(main) else {
        panic!("the layer dropdown");
    };
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    assert!(
        groups
            .iter()
            .flat_map(|group| &group.rows)
            .any(|row| row.layer == bottom)
    );
    event(&mut app, BoardPopoverEvent::LayerPicked(bottom));
    assert_eq!(app.pcb_layer_view(&path).active(), bottom);
    assert_eq!(open_kind(&app), None, "a pick closes it");

    click(&mut app, main, BoardCounter::Layer);
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert_eq!(open_kind(&app), None, "Esc closes it");
    click(&mut app, main, BoardCounter::Layer);
    event(&mut app, BoardPopoverEvent::Dismissed);
    assert_eq!(open_kind(&app), None, "a press outside closes it");
}

/// What a right press on the Layer pill of `window`'s bar sends.
fn pill_right_press(app: &Signex, window: iced::window::Id) -> Vec<Message> {
    use iced::mouse::{Button, Event};
    let size = iced::Size::new(1600.0, 40.0);
    let caption = signex_i18n::t!(app.ui_state.i18n, signex_i18n::ids::PCB_STATUS_LAYER);
    let texts = crate::layout_probe::lay_out_texts(app.view_status_bar(window), size);
    let at = crate::layout_probe::find(&texts, &caption.to_string())
        .bounds
        .center();
    crate::layout_probe::messages_for_events(
        app.view_status_bar(window),
        size,
        at,
        &[iced::Event::Mouse(Event::ButtonPressed(Button::Right))],
    )
}

/// #2460 under Dropdown: the click opens the list, the right-click the
/// Active layer popover, which stays open on a pick.
#[test]
fn under_dropdown_the_pill_click_lists_and_its_right_click_opens_the_popover() {
    use crate::layer_strip_prefs::LayerSwitcher;
    let (mut app, path, main) = app_with_board();
    app.ui_state.layer_switcher = LayerSwitcher::Dropdown;

    for message in pill_click(&app, main) {
        let _ = app.update(message);
    }
    assert!(matches!(
        app.board_popover_model(main),
        Some(BoardPopoverModel::LayerDropdown(_))
    ));
    event(&mut app, BoardPopoverEvent::Dismissed);

    for message in pill_right_press(&app, main) {
        let _ = app.update(message);
    }
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::ActiveLayer)));
    assert!(matches!(
        app.board_popover_model(main),
        Some(BoardPopoverModel::Layer(_))
    ));
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    event(&mut app, BoardPopoverEvent::LayerPicked(bottom));
    assert_eq!(app.pcb_layer_view(&path).active(), bottom);
    assert_eq!(
        open_kind(&app),
        Some((main, BoardPopoverKind::ActiveLayer)),
        "the Active layer popover stays open on a pick"
    );
}

/// What a click on the row `label` of the open layer dropdown sends.
fn dropdown_row_click(app: &Signex, main: iced::window::Id, label: &str) -> Vec<Message> {
    let card = app.board_popover_card(main).expect("the dropdown");
    crate::layout_probe::messages_on_click(card, iced::Size::new(1600.0, 900.0), label)
}

fn open_dropdown(app: &mut Signex, main: iced::window::Id) {
    app.ui_state.layer_switcher = crate::layer_strip_prefs::LayerSwitcher::Dropdown;
    click(app, main, BoardCounter::Layer);
}

/// Caner (2026-11-21): under Dropdown the sets are in the layer list, and a
/// picked set is shown as from the strip's set button.
#[test]
fn the_layer_dropdown_lists_the_sets_and_a_pick_shows_the_set() {
    use signex_renderer::pcb::{BuiltinLayerSet, LayerSetId};
    let (mut app, path, main) = app_with_board();
    open_dropdown(&mut app, main);
    let card = app.board_popover_card(main).expect("the dropdown");
    let texts = crate::layout_probe::lay_out_texts(card, iced::Size::new(1600.0, 900.0));
    for label in [
        "All Layers",
        "Signal Layers",
        "Save current as set...",
        "Manage sets...",
    ] {
        assert!(texts.iter().any(|t| t.text == label), "{label}");
    }
    for message in dropdown_row_click(&app, main, "Signal Layers") {
        let _ = app.update(message);
    }
    assert_eq!(
        app.pcb_layer_view(&path)
            .layer_set()
            .map(|set| set.id.clone()),
        Some(LayerSetId::Builtin(BuiltinLayerSet::Signal))
    );
    assert_eq!(open_kind(&app), None, "the pick closes the dropdown");
    assert!(app.interaction_state.pcb_layer_set_menu.is_none());
}

/// Save current as set... reaches the naming card, above the status bar.
#[test]
fn save_from_the_layer_dropdown_opens_the_naming_card() {
    let (mut app, _path, main) = app_with_board();
    open_dropdown(&mut app, main);
    for message in dropdown_row_click(&app, main, "Save current as set...") {
        let _ = app.update(message);
    }
    let menu = app
        .interaction_state
        .pcb_layer_set_menu
        .as_ref()
        .expect("the layer set menu");
    assert!(matches!(&menu.card, crate::app::LayerSetCard::Naming(name) if name.is_empty()));
    let tokens = &app.document_state.panel_ctx.tokens;
    let (_, height) = app.ui_state.size_of_window(main);
    assert!(menu.y < height - crate::status_bar::status_bar_height(tokens));
    assert_eq!(open_kind(&app), None);
}

/// #2460 under Strip: the pill takes no click, and its right-click opens
/// the Active layer popover.
#[test]
fn under_strip_the_pill_right_click_opens_the_popover() {
    use crate::layer_strip_prefs::LayerSwitcher;
    let (mut app, _path, main) = app_with_board();
    app.ui_state.layer_switcher = LayerSwitcher::Strip;
    assert!(pill_click(&app, main).is_empty());
    for message in pill_right_press(&app, main) {
        let _ = app.update(message);
    }
    assert_eq!(open_kind(&app), Some((main, BoardPopoverKind::ActiveLayer)));
    assert!(matches!(
        app.board_popover_model(main),
        Some(BoardPopoverModel::Layer(_))
    ));
}
