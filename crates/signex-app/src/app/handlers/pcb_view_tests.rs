//! #2223 review: the board view follows the window it is used in, and the
//! status pill never names a hidden layer.

use std::path::PathBuf;

use signex_types::pcb::{LayerDef, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::app::state::WindowKind;
use crate::app::{FileMsg, Message, PcbMsg, PcbViewMsg, Signex, UiMsg};
use crate::keymap::ShortcutTrigger;

fn board() -> PcbBoard {
    let layer = |id: u8, name: &str| LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    };
    PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

fn open(app: &mut Signex, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "board".to_string(),
        result: Ok(Box::new(board())),
    }));
    path
}

/// The main window shows board A; an undocked window shows board B.
fn two_windows() -> (Signex, PathBuf, PathBuf, iced::window::Id, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let a = open(&mut app, "/w/a.snxpcb");
    let b = open(&mut app, "/w/b.snxpcb");
    let index_a = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == a)
        .expect("tab a");
    app.document_state.active_tab = index_a;
    let main = iced::window::Id::unique();
    let undocked = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.windows.insert(
        undocked,
        WindowKind::UndockedTab {
            path: b.clone(),
            title: "b".to_string(),
        },
    );
    assert!(app.has_active_pcb());
    (app, a, b, main, undocked)
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

#[test]
fn a_layer_key_in_an_undocked_window_steps_that_windows_board() {
    let (mut app, a, b, main, undocked) = two_windows();
    press(&mut app, undocked, "-");
    assert_eq!(app.pcb_layer_view(&b).active(), CopperLayer::Bottom);
    assert_eq!(
        app.pcb_layer_view(&a).active(),
        CopperLayer::Top,
        "A untouched"
    );
    press(&mut app, main, "-");
    assert_eq!(app.pcb_layer_view(&a).active(), CopperLayer::Bottom);
}

#[test]
fn each_window_pill_names_its_own_boards_layer() {
    let (mut app, _a, _b, main, undocked) = two_windows();
    let _ = app.update(Message::Pcb(PcbMsg::ViewInWindow {
        window: undocked,
        event: PcbViewMsg::ActiveLayerStepped { forward: true },
    }));
    let name = |app: &Signex, window| {
        app.status_active_layer(window)
            .map(|(_, name)| name.into_string())
    };
    assert_eq!(name(&app, main).as_deref(), Some("Top Copper"));
    assert_eq!(name(&app, undocked).as_deref(), Some("Bottom Copper"));
}

#[test]
fn hiding_the_active_layer_moves_the_pill_and_hiding_all_copper_removes_it() {
    let (mut app, a, _b, main, _undocked) = two_windows();
    let hide = |app: &mut Signex, layer| {
        let _ = app.update(Message::Pcb(PcbMsg::View(
            PcbViewMsg::LayerVisibilityToggled(PcbLayer::Copper(layer)),
        )));
    };
    hide(&mut app, CopperLayer::Top);
    assert_eq!(app.pcb_layer_view(&a).active(), CopperLayer::Bottom);
    let pill = app.status_active_layer(main).map(|(_, n)| n.into_string());
    assert_eq!(pill.as_deref(), Some("Bottom Copper"));
    hide(&mut app, CopperLayer::Bottom);
    assert!(
        app.status_active_layer(main).is_none(),
        "no copper is shown"
    );
    // `+` with every copper layer hidden leaves the active layer alone.
    press(&mut app, main, "+");
    assert_eq!(app.pcb_layer_view(&a).active(), CopperLayer::Bottom);
}
