//! #2394: the layer set button and the full layer tab menu, through
//! `Signex::update`: a set shows its layers on the canvas and in the strip, a
//! saved set is kept for the next launch, Highlight dims the other layers,
//! the tabs take the short names, and Flip shows the board from below.

use std::path::PathBuf;

use iced::Size;
use signex_renderer::pcb::{BuiltinLayerSet, LayerSetId};
use signex_types::pcb::{LayerDef, PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{
    FileMsg, LayerSetMenuMsg, LayerStripMsg, LayerTabMenuMsg, Message, PcbMsg, PcbViewMsg, Signex,
};
use crate::keymap::AppCommandId;
use crate::layer_strip_prefs::LayerNameStyle;
use crate::layout_probe::{lay_out_texts, messages_on_click_at};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const WINDOW: Size = Size::new(1440.0, 900.0);

fn segment(n: u128, y: f64, layer: &str) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.5,
        layer: layer.to_owned(),
        net: 1,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    }
}

/// Top Layer, Top Overlay, Bottom Layer, in that order, a track on each
/// copper layer: the bottom one first in the file, so a draw order that
/// stacks by side differs from the file's.
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
            segment(2, 5.0, "BottomCopper"),
            segment(1, 0.0, "TopCopper"),
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
    let path = std::env::temp_dir().join("layer-sets.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "layer-sets".to_string(),
        result: Ok(Box::new(board())),
    }));
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    assert!(app.has_active_pcb());
    let bounds = app.canvas_bounds();
    app.interaction_state.pcb_canvas.resolve_pending_fit(bounds);
    (app, path, main)
}

/// The texts `window` shows, without Fluent's isolation marks.
fn texts(app: &Signex, window: iced::window::Id) -> Vec<String> {
    lay_out_texts(app.view(window), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn count(app: &Signex, window: iced::window::Id, label: &str) -> usize {
    texts(app, window).iter().filter(|t| *t == label).count()
}

/// Click the text `label` whose click sends a message `wanted` accepts, and
/// run what it sends.
fn click_where(
    app: &mut Signex,
    window: iced::window::Id,
    label: &str,
    wanted: impl Fn(&Message) -> bool,
) {
    let (at, sent) = lay_out_texts(app.view(window), WINDOW)
        .into_iter()
        .filter(|t| t.text == label)
        .map(|t| t.bounds.center())
        .map(|at| (at, messages_on_click_at(app.view(window), WINDOW, None, at)))
        .find(|(_, sent)| sent.iter().any(&wanted))
        .unwrap_or_else(|| panic!("no {label:?} sends what was wanted"));
    app.interaction_state.last_mouse_pos = (at.x, at.y);
    for msg in sent {
        let _ = app.update(msg);
    }
}

/// Open the layer set menu with the button showing `shown`.
fn open_set_menu(app: &mut Signex, window: iced::window::Id, shown: &str) {
    click_where(app, window, shown, |m| {
        matches!(
            m,
            Message::Pcb(PcbMsg::LayerSetMenu(LayerSetMenuMsg::Opened { .. }))
        )
    });
    assert!(app.interaction_state.pcb_layer_set_menu.is_some());
}

/// Click the open menu's row `label`.
fn pick_row(app: &mut Signex, window: iced::window::Id, label: &str) {
    click_where(app, window, label, |m| matches!(m, Message::Pcb(_)));
}

fn traces(app: &Signex) -> Vec<(f32, bool)> {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .traces
        .iter()
        .map(|t| (t.p0[1], t.dimmed))
        .collect()
}

#[test]
fn signal_layers_hide_the_other_layers_from_the_canvas_and_the_strip() {
    let (mut app, path, main) = open();
    let overlay_tabs = count(&app, main, "Top Overlay");
    assert!(overlay_tabs >= 1, "the strip lists Top Overlay");
    let signal_texts = count(&app, main, "Signal");

    open_set_menu(&mut app, main, "All");
    pick_row(&mut app, main, "Signal Layers");

    assert!(app.interaction_state.pcb_layer_set_menu.is_none());
    let view = app.pcb_layer_view(&path);
    assert_eq!(
        view.layer_set().map(|set| &set.id),
        Some(&LayerSetId::Builtin(BuiltinLayerSet::Signal))
    );
    assert!(
        view.is_hidden(OVERLAY),
        "the overlay is hidden on the canvas"
    );
    assert!(!view.is_hidden(TOP) && !view.is_hidden(BOTTOM));
    assert_eq!(traces(&app).len(), 2, "both copper tracks are drawn");
    assert_eq!(
        count(&app, main, "Top Overlay"),
        overlay_tabs - 1,
        "the strip no longer lists the overlay"
    );
    assert_eq!(
        count(&app, main, "Signal"),
        signal_texts + 1,
        "the button names the set"
    );

    // The active layer moves into the set when it falls out.
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        OVERLAY,
    ))));
    let _ = app.update(Message::Pcb(PcbMsg::View(
        PcbViewMsg::BuiltinLayerSetShown(BuiltinLayerSet::Signal),
    )));
    assert!(app.pcb_layer_view(&path).active().is_copper());

    // All Layers lists and shows every layer again.
    open_set_menu(&mut app, main, "Signal");
    pick_row(&mut app, main, "All Layers");
    let view = app.pcb_layer_view(&path);
    assert!(view.layer_set().is_none() && !view.is_hidden(OVERLAY));
}

#[test]
fn a_saved_set_survives_a_restart() {
    let (mut app, path, main) = open();
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::LayerHidden(BOTTOM))));
    open_set_menu(&mut app, main, "All");
    pick_row(&mut app, main, "Save current as set...");
    let set_menu = |m: LayerSetMenuMsg| Message::Pcb(PcbMsg::LayerSetMenu(m));
    let _ = app.update(set_menu(LayerSetMenuMsg::NameEdited("Top side".into())));
    let _ = app.update(set_menu(LayerSetMenuMsg::NameSubmitted));
    assert!(app.interaction_state.pcb_layer_set_menu.is_none());

    let saved = app
        .ui_state
        .layer_strip
        .saved_set("Top side")
        .expect("the set is saved")
        .clone();
    assert!(saved.layers.contains(&TOP) && saved.layers.contains(&OVERLAY));
    assert!(
        !saved.layers.contains(&BOTTOM),
        "a hidden layer is not in it"
    );
    assert_eq!(count(&app, main, "Top side"), 1, "the button names it");

    // What the app writes is what the next launch reads.
    let dir = tempfile::tempdir().expect("a temp dir");
    let prefs = dir.path().join("prefs.json");
    crate::fonts::write_layer_strip_pref_at(&prefs, &app.ui_state.layer_strip);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&prefs).expect("written")).expect("json");
    let read = crate::fonts::read_layer_strip_pref_from(Some(&json));
    assert_eq!(read, app.ui_state.layer_strip);

    let (mut next, next_path, next_main) = open();
    next.ui_state.layer_strip = read;
    open_set_menu(&mut next, next_main, "All");
    pick_row(&mut next, next_main, "Top side");
    let view = next.pcb_layer_view(&next_path);
    assert!(view.is_hidden(BOTTOM) && !view.is_hidden(TOP));
    assert_eq!(
        view.layer_set().map(|set| &set.id),
        Some(&LayerSetId::Saved("Top side".into()))
    );
    assert!(app.pcb_layer_view(&path).layer_set().is_some());
}

#[test]
fn highlight_dims_every_other_layer_and_toggles_off() {
    let (mut app, path, _main) = open();
    let tab_menu = |m: LayerTabMenuMsg| Message::Pcb(PcbMsg::LayerTabMenu(m));
    let highlight = |app: &mut Signex| {
        let _ = app.update(tab_menu(LayerTabMenuMsg::Opened {
            window: None,
            layer: BOTTOM,
        }));
        let _ = app.update(tab_menu(LayerTabMenuMsg::Picked(
            PcbViewMsg::LayerHighlightToggled(BOTTOM),
        )));
    };
    highlight(&mut app);
    assert_eq!(app.pcb_layer_view(&path).highlighted(), Some(BOTTOM));
    // The top track dimmed and below, the bottom track on top and bright.
    assert_eq!(traces(&app), vec![(0.0, true), (5.0, false)]);
    highlight(&mut app);
    assert_eq!(app.pcb_layer_view(&path).highlighted(), None);
    assert_eq!(
        traces(&app),
        vec![(5.0, true), (0.0, false)],
        "Top is active"
    );
}

#[test]
fn short_names_show_tl_and_bl_and_numbers_go_in_front() {
    let (mut app, _path, main) = open();
    let setting = |s| Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::SettingPicked(s)));
    let _ = app.update(setting(LayerStripMsg::NameStyleSet(LayerNameStyle::Short)));
    assert_eq!(app.ui_state.layer_strip.name_style, LayerNameStyle::Short);
    let shown = texts(&app, main);
    for name in ["TL", "TO", "BL"] {
        assert!(
            shown.iter().any(|t| t == name),
            "{name} on a tab: {shown:?}"
        );
    }

    let _ = app.update(setting(LayerStripMsg::LayerNumbersToggled));
    let shown = texts(&app, main);
    assert!(shown.iter().any(|t| t == "1 TL"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "2 BL"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "TO"), "an overlay has no number");
}

#[test]
fn flip_mirrors_the_view_and_draws_bottom_copper_above_top() {
    let (mut app, path, _main) = open();
    // With a non-copper layer active no copper is raised over the rest, so
    // the stacking of the sides shows.
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        OVERLAY,
    ))));
    let ys = |app: &Signex| traces(app).iter().map(|(y, _)| *y).collect::<Vec<_>>();
    assert_eq!(ys(&app), vec![5.0, 0.0], "from above, the file's order");

    let canvas = &app.interaction_state.pcb_canvas;
    let left = canvas.board_to_screen(Point::new(0.0, 0.0));
    let right = canvas.board_to_screen(Point::new(10.0, 0.0));
    assert!(right.x > left.x);

    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::BoardFlipToggled)));
    assert!(app.pcb_layer_view(&path).flipped());
    let canvas = &app.interaction_state.pcb_canvas;
    assert!(canvas.view_flipped());
    let left = canvas.board_to_screen(Point::new(0.0, 0.0));
    let right = canvas.board_to_screen(Point::new(10.0, 0.0));
    assert!(right.x < left.x, "board +x runs to the left from below");
    assert_eq!(ys(&app), vec![0.0, 5.0], "bottom copper drawn last, on top");
    let board = app.active_pcb().expect("a board");
    assert_eq!(board.segments.len(), 2, "the board itself is untouched");

    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::BoardFlipToggled)));
    assert!(!app.interaction_state.pcb_canvas.view_flipped());
}

/// Run command `name` from `operands` and `document`.
fn run_with<'s>(
    app: &mut Signex,
    name: &str,
    operands: &[&'s str],
    document: &'s str,
) -> CommandOutcome {
    let command = AppCommandId::new(name).expect("a command id");
    let mut all: Vec<&'s str> = operands.to_vec();
    all.push(document);
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    app.run_operands(&command, &args).0
}

/// The commands an agent drives name the board and do what the rows do.
#[test]
fn the_layer_commands_run_from_their_operands_on_the_named_board() {
    let (mut app, path, _main) = open();
    let document = format!("document={}", path.display());
    let run =
        |app: &mut Signex, name: &str, operands: &[&str]| run_with(app, name, operands, &document);
    assert_eq!(
        run(&mut app, "hide_layer", &["layer=BottomCopper"]),
        CommandOutcome::Ran
    );
    assert!(app.pcb_layer_view(&path).is_hidden(BOTTOM));
    assert_eq!(
        run(&mut app, "highlight_layer", &["layer=TopCopper"]),
        CommandOutcome::Ran
    );
    assert_eq!(app.pcb_layer_view(&path).highlighted(), Some(TOP));
    assert_eq!(
        run(&mut app, "show_layer_group", &["group=all"]),
        CommandOutcome::Ran
    );
    assert!(!app.pcb_layer_view(&path).is_hidden(BOTTOM));
    assert_eq!(
        run(&mut app, "show_layer_set", &["name=signal"]),
        CommandOutcome::Ran
    );
    assert!(app.pcb_layer_view(&path).is_hidden(OVERLAY));
    assert!(matches!(
        run(&mut app, "hide_layer", &["layer=Nowhere"]),
        CommandOutcome::Disabled { .. }
    ));
    assert!(matches!(
        run(&mut app, "show_layer_set", &["name=missing"]),
        CommandOutcome::Disabled { .. }
    ));
}
