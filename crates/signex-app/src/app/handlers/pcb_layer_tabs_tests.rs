//! #2357: the layer tabs under the board canvas and the layer keys set the
//! active layer, which the status pill names. #2359: one Layers panel on a
//! board tab lists the board's layers and drives the same view.

use std::path::PathBuf;

use iced::Size;
use signex_types::pcb::{LayerDef, PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use crate::app::state::WindowKind;
use crate::app::{FileMsg, LayerTabMenuMsg, Message, PcbMsg, Signex, UiMsg, WindowMsg};
use crate::dock::DockMessage;
use crate::keymap::ShortcutTrigger;
use crate::layout_probe::{
    lay_out_texts, messages_for_events, messages_on_click, messages_on_click_at,
};
use crate::panels::{PanelKind, PanelMsg, view_panel};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const WINDOW: Size = Size::new(1440.0, 900.0);

fn segment(n: u128, y: f64, layer: &str) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.5,
        layer: layer.to_owned(),
        net: 1,
        unknown_columns: Default::default(),
    }
}

/// Top Layer, Top Overlay, Bottom Layer, in that order, a track on each
/// copper layer.
fn board() -> PcbBoard {
    let layer = |id: u8, name: &str, kind: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: kind.to_owned(),
        display_name: String::new(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        options: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![
            layer(0, "TopCopper", "signal"),
            layer(1, "TopSilkscreen", ""),
            layer(31, "BottomCopper", "signal"),
        ],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![
            segment(1, 0.0, "TopCopper"),
            segment(2, 5.0, "BottomCopper"),
        ],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

fn open() -> (Signex, PathBuf, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = PathBuf::from("/w/tabs.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "tabs".to_string(),
        result: Ok(Box::new(board())),
    }));
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    assert!(app.has_active_pcb());
    (app, path, main)
}

fn press(app: &mut Signex, window: iced::window::Id, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key");
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: Some(window),
        stroke: strokes.remove(0),
    }));
}

fn pill(app: &Signex, window: iced::window::Id) -> Option<String> {
    app.status_active_layer(window)
        .map(|(_, name)| name.into_string())
}

#[test]
fn a_tab_click_sets_the_active_layer_and_the_status_readout() {
    let (mut app, path, main) = open();
    assert_eq!(pill(&app, main).as_deref(), Some("Top Layer"));
    // The opened board's first fit resolves on the canvas's first event and
    // publishes a cursor move; settle it first so the click reads only the
    // tab's own message.
    let bounds = app.canvas_bounds();
    app.interaction_state.pcb_canvas.resolve_pending_fit(bounds);
    let sent = messages_on_click(app.view(main), WINDOW, "Bottom Layer");
    assert_eq!(sent.len(), 1, "the tab sends one message: {sent:?}");
    for msg in sent {
        let _ = app.update(msg);
    }
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
    assert_eq!(pill(&app, main).as_deref(), Some("Bottom Layer"));
    // The bottom track now draws last, on top of the rest.
    let snapshot = app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot");
    assert_eq!(snapshot.traces.last().map(|t| t.p0[1]), Some(5.0));

    // A non-copper tab is a layer like any other.
    let sent = messages_on_click(app.view(main), WINDOW, "Top Overlay");
    for msg in sent {
        let _ = app.update(msg);
    }
    assert_eq!(app.pcb_layer_view(&path).active(), OVERLAY);
    assert_eq!(pill(&app, main).as_deref(), Some("Top Overlay"));
}

#[test]
fn plus_steps_to_the_next_layer_and_star_to_the_next_signal_layer() {
    let (mut app, path, main) = open();
    press(&mut app, main, "+");
    assert_eq!(app.pcb_layer_view(&path).active(), OVERLAY, "board order");
    assert_eq!(pill(&app, main).as_deref(), Some("Top Overlay"));
    press(&mut app, main, "+");
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
    press(&mut app, main, "-");
    assert_eq!(app.pcb_layer_view(&path).active(), OVERLAY);
    press(&mut app, main, "-");
    assert_eq!(app.pcb_layer_view(&path).active(), TOP);
    press(&mut app, main, "*");
    assert_eq!(
        app.pcb_layer_view(&path).active(),
        BOTTOM,
        "the overlay is not a signal layer"
    );
}

#[test]
fn shift_s_toggles_single_layer_mode() {
    let (mut app, path, main) = open();
    press(&mut app, main, "Shift+S");
    assert!(app.pcb_layer_view(&path).single_layer());
    let traces = app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .traces
        .len();
    assert_eq!(traces, 1, "only the Top track is drawn");
    assert!(app.document_state.panel_ctx.pcb_layers.single_layer);
    press(&mut app, main, "Shift+S");
    assert!(!app.pcb_layer_view(&path).single_layer());
}

fn panel_send(app: &mut Signex, sent: Vec<PanelMsg>) {
    for msg in sent {
        let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
    }
}

/// #2359: on a board tab the one Layers panel lists the board's layers in
/// groups; a row click sets the active layer, an eye hides the layer from
/// the canvas draw.
#[test]
fn the_layers_panel_on_a_board_tab_lists_and_drives_the_board_layers() {
    let (mut app, path, main) = open();
    let panel = Size::new(300.0, 1800.0);
    let ctx = app.document_state.panel_ctx.clone();
    let texts = lay_out_texts(view_panel(PanelKind::Layers, &ctx), panel);
    for label in [
        "SIGNAL",
        "Top Layer",
        "Bottom Layer",
        "OVERLAY",
        "Top Overlay",
    ] {
        assert!(texts.iter().any(|t| t.text == label), "{label} listed");
    }

    let sent = messages_on_click(view_panel(PanelKind::Layers, &ctx), panel, "Bottom Layer");
    panel_send(&mut app, sent);
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
    assert_eq!(pill(&app, main).as_deref(), Some("Bottom Layer"));

    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbLayers(
        crate::app::PcbViewMsg::LayerVisibilityToggled(TOP),
    ))));
    let snapshot = app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot");
    assert!(
        snapshot.traces.iter().all(|t| t.p0[1] != 0.0),
        "the hidden Top track is not drawn"
    );
    assert!(app.document_state.panel_ctx.pcb_layers.is_hidden(TOP));
}

/// The centre of the last text `label` laid out in `window`'s view: an open
/// menu paints after the panels, so its row is the last one with the text.
fn last_text_at(app: &Signex, window: iced::window::Id, label: &str) -> iced::Point {
    lay_out_texts(app.view(window), WINDOW)
        .into_iter()
        .rfind(|t| t.text == label)
        .unwrap_or_else(|| panic!("no {label:?} in the view"))
        .bounds
        .center()
}

/// Right-click the tab of `label` and run what it sends. The same name may
/// also show in a panel or the status bar; the tab is the one whose
/// right-click opens its menu, owned by `window` (`None` for the main one).
fn right_click_tab(app: &mut Signex, window: iced::window::Id, label: &str) {
    let press = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
        iced::mouse::Button::Right,
    ));
    let owner = app.overlay_owner(window);
    let opens = |m: &Message| {
        matches!(
            m,
            Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Opened {
                window: opened_in,
                ..
            })) if *opened_in == owner
        )
    };
    let (at, sent) = lay_out_texts(app.view(window), WINDOW)
        .into_iter()
        .filter(|t| t.text == label)
        .map(|t| t.bounds.center())
        .map(|at| {
            (
                at,
                messages_for_events(app.view(window), WINDOW, at, &[press.clone()]),
            )
        })
        .find(|(_, sent)| sent.iter().any(opens))
        .unwrap_or_else(|| panic!("no tab {label:?} opens a menu on a right-click"));
    // The pointer subscription keeps this; the menu opens where it is.
    app.interaction_state.last_mouse_pos = (at.x, at.y);
    for msg in sent {
        let _ = app.update(msg);
    }
}

/// Click the menu row `label` and run what it sends.
fn pick_row(app: &mut Signex, window: iced::window::Id, label: &str) {
    let at = last_text_at(app, window, label);
    for msg in messages_on_click_at(app.view(window), WINDOW, None, at) {
        let _ = app.update(msg);
    }
}

/// #2379: a right-click on a layer tab opens its menu; each row changes what
/// the board view shows and closes the menu.
#[test]
fn a_tab_right_click_opens_a_menu_whose_rows_change_visibility() {
    let (mut app, path, main) = open();
    let bounds = app.canvas_bounds();
    app.interaction_state.pcb_canvas.resolve_pending_fit(bounds);

    right_click_tab(&mut app, main, "Bottom Layer");
    let menu = app
        .interaction_state
        .pcb_layer_tab_menu
        .expect("the menu is open");
    assert_eq!(menu.layer, BOTTOM);
    pick_row(&mut app, main, "Hide Bottom Layer");
    assert!(app.interaction_state.pcb_layer_tab_menu.is_none());
    assert!(app.pcb_layer_view(&path).is_hidden(BOTTOM));
    let traces = |app: &Signex| {
        app.interaction_state
            .pcb_canvas
            .active_renderer_snapshot()
            .expect("snapshot")
            .traces
            .len()
    };
    assert_eq!(traces(&app), 1, "the bottom track is no longer drawn");

    right_click_tab(&mut app, main, "Top Overlay");
    pick_row(&mut app, main, "Show only this layer");
    let view = app.pcb_layer_view(&path);
    assert_eq!(view.active(), OVERLAY);
    assert!(!view.is_hidden(OVERLAY));
    assert!(view.is_hidden(TOP) && view.is_hidden(BOTTOM));
    assert_eq!(traces(&app), 0, "no copper is drawn");
    assert_eq!(pill(&app, main).as_deref(), Some("Top Overlay"));

    right_click_tab(&mut app, main, "Top Layer");
    pick_row(&mut app, main, "Show all layers");
    assert!(app.interaction_state.pcb_layer_tab_menu.is_none());
    assert!(!app.pcb_layer_view(&path).has_hidden());
    assert_eq!(traces(&app), 2);
}

#[test]
fn esc_and_a_click_outside_close_the_tab_menu_and_change_nothing() {
    let (mut app, path, main) = open();
    let bounds = app.canvas_bounds();
    app.interaction_state.pcb_canvas.resolve_pending_fit(bounds);
    right_click_tab(&mut app, main, "Bottom Layer");
    let _ = app.update(Message::Input(crate::app::InputMsg::EscapePressed {
        window: None,
    }));
    assert!(app.interaction_state.pcb_layer_tab_menu.is_none());

    right_click_tab(&mut app, main, "Bottom Layer");
    let _ = app.update(Message::Pcb(PcbMsg::LayerTabMenu(
        LayerTabMenuMsg::Dismissed,
    )));
    assert!(app.interaction_state.pcb_layer_tab_menu.is_none());
    assert!(!app.pcb_layer_view(&path).has_hidden());
    assert_eq!(app.pcb_layer_view(&path).active(), TOP);
}

/// The main window shows `/w/tabs.snxpcb`; an undocked window shows a
/// second board with the same layers, with its own fitted canvas.
fn undocked() -> (Signex, PathBuf, PathBuf, iced::window::Id) {
    let (mut app, main_path, _main) = open();
    let path = PathBuf::from("/w/undocked.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "undocked".to_string(),
        result: Ok(Box::new(board())),
    }));
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == main_path)
        .expect("the main board's tab");
    app.document_state.active_tab = index;
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: path.clone(),
            title: "undocked".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    let canvas = app.new_pcb_canvas_for(&path).expect("a PCB canvas");
    let bounds = app.canvas_bounds();
    canvas.resolve_pending_fit(bounds);
    app.interaction_state.pcb_canvases.insert(window, canvas);
    app.interaction_state.pcb_canvas.resolve_pending_fit(bounds);
    (app, main_path, path, window)
}

/// #2379 review: a pick in an undocked window's tab menu changes that
/// window's board view, not the main window's.
#[test]
fn a_pick_in_an_undocked_windows_tab_menu_changes_that_windows_board() {
    let (mut app, main_path, path, window) = undocked();
    right_click_tab(&mut app, window, "Bottom Layer");
    let menu = app
        .interaction_state
        .pcb_layer_tab_menu
        .expect("the menu is open");
    assert_eq!(menu.window, Some(window));
    assert_eq!(menu.layer, BOTTOM);
    pick_row(&mut app, window, "Hide Bottom Layer");
    assert!(app.interaction_state.pcb_layer_tab_menu.is_none());
    assert!(app.pcb_layer_view(&path).is_hidden(BOTTOM));
    assert!(
        !app.pcb_layer_view(&main_path).has_hidden(),
        "the main window's board is untouched"
    );
}

/// #2379 review, as #1910 for the other menus: closing or reattaching the
/// undocked window drops the tab menu it opened, so no menu nobody can see
/// is left to take the next Esc.
#[test]
fn closing_or_reattaching_an_undocked_window_drops_its_tab_menu() {
    let closers: [fn(iced::window::Id) -> WindowMsg; 2] =
        [WindowMsg::SecondaryWindowClosed, WindowMsg::ReattachTab];
    for closed in closers {
        let (mut app, _main_path, _path, window) = undocked();
        let _ = app.update(Message::Pcb(PcbMsg::LayerTabMenu(
            LayerTabMenuMsg::Opened {
                window: Some(window),
                layer: BOTTOM,
            },
        )));
        assert!(app.interaction_state.pcb_layer_tab_menu.is_some());
        let _ = app.update(Message::Window(closed(window)));
        assert!(app.interaction_state.pcb_layer_tab_menu.is_none());
    }
}

/// A tab menu the main window opened stays when an undocked window closes.
#[test]
fn closing_an_undocked_window_keeps_the_main_windows_tab_menu() {
    let (mut app, _main_path, _path, window) = undocked();
    let _ = app.update(Message::Pcb(PcbMsg::LayerTabMenu(
        LayerTabMenuMsg::Opened {
            window: None,
            layer: BOTTOM,
        },
    )));
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    assert!(app.interaction_state.pcb_layer_tab_menu.is_some());
}
