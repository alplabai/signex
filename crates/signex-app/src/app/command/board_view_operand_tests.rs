//! #2363: the Layers panel's board view configuration through
//! `Signex::update` and through its commands. A panel control and its
//! command change the named board's view the same way, and the canvas draws
//! what the view says.

use std::path::PathBuf;

use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{BoardObject, PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::pcb::{LayerDef, PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{FileMsg, Message, PcbViewMsg, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::PanelMsg;

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);

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

/// Top and Bottom copper, a track on each.
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
        options: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![segment(1, 0.0, "TopCopper"), segment(2, 5.0, "BottomCopper")],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
        free: Default::default(),
    }
}

fn open() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = std::env::temp_dir().join("board-view-config.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "board-view-config".to_string(),
        result: Ok(Box::new(board())),
    }));
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    assert!(app.has_active_pcb());
    (app, path)
}

fn panel(app: &mut Signex, event: PcbViewMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbLayers(event))));
}

fn snapshot(app: &Signex) -> PcbSnapshot {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .clone()
}

/// The canvas's track lines: (y, colour).
fn lines(app: &Signex) -> Vec<(f32, [f32; 4])> {
    let snapshot = snapshot(app);
    let theme = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
    let mut scene = Scene::default();
    PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::LINES, &mut scene);
    snapshot
        .traces
        .iter()
        .zip(&scene.lines)
        .map(|(t, l)| (t.p0[1], l.color))
        .collect()
}

fn run(app: &mut Signex, path: &std::path::Path, name: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(name).expect("a command id");
    let document = format!("document={}", path.display());
    let mut all: Vec<&str> = operands.to_vec();
    all.push(&document);
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    app.run_operands(&command, &args).0
}

#[test]
fn a_layer_eye_hides_it_from_the_draw_and_its_command_shows_it() {
    let (mut app, path) = open();
    panel(&mut app, PcbViewMsg::LayerVisibilityToggled(BOTTOM));
    assert!(lines(&app).iter().all(|(y, _)| *y == 0.0), "Bottom is gone");
    assert_eq!(
        run(&mut app, &path, "set_layer_visible", &["layer=BottomCopper", "on=true"]),
        CommandOutcome::Ran
    );
    assert_eq!(lines(&app).len(), 2);
    assert_eq!(
        run(&mut app, &path, "set_layer_visible", &["layer=BottomCopper", "on=false"]),
        CommandOutcome::Ran
    );
    assert!(app.pcb_layer_view(&path).is_hidden(BOTTOM));
}

#[test]
fn a_colour_change_recolours_the_canvas_and_the_chip() {
    let (mut app, path) = open();
    let before = lines(&app);
    panel(
        &mut app,
        PcbViewMsg::LayerColourSet {
            layer: TOP,
            colour: Some([255, 0, 255]),
        },
    );
    let after = lines(&app);
    let top = |lines: &[(f32, [f32; 4])]| lines.iter().find(|(y, _)| *y == 0.0).unwrap().1;
    assert_eq!(&top(&after)[..3], &[1.0, 0.0, 1.0]);
    assert_ne!(top(&before), top(&after));
    assert_eq!(
        app.document_state.panel_ctx.pcb_layers.colour(TOP),
        iced::Color::from_rgb8(255, 0, 255),
        "the chip shows the canvas colour"
    );
    assert_eq!(
        run(&mut app, &path, "set_layer_color", &["layer=TopCopper"]),
        CommandOutcome::Ran
    );
    assert_eq!(top(&lines(&app)), top(&before), "the palette colour again");
    assert_eq!(
        run(&mut app, &path, "set_layer_color", &["layer=TopCopper", "color=#00ff00"]),
        CommandOutcome::Ran
    );
    assert_eq!(&top(&lines(&app))[..3], &[0.0, 1.0, 0.0]);
}

#[test]
fn used_on_and_the_active_layer_run_from_the_panel_and_the_commands() {
    let (mut app, path) = open();
    panel(&mut app, PcbViewMsg::UsedLayersOnlySet(true));
    assert!(app.document_state.panel_ctx.pcb_layers.used.is_some());
    assert_eq!(
        run(&mut app, &path, "show_used_layers_only", &["on=false"]),
        CommandOutcome::Ran
    );
    assert!(app.document_state.panel_ctx.pcb_layers.used.is_none());
    assert_eq!(
        run(&mut app, &path, "set_active_layer", &["layer=BottomCopper"]),
        CommandOutcome::Ran
    );
    assert_eq!(app.pcb_layer_view(&path).active(), BOTTOM);
}

#[test]
fn single_layer_mode_leaves_only_the_active_layer() {
    let (mut app, _path) = open();
    panel(&mut app, PcbViewMsg::SingleLayerModeToggled);
    assert!(lines(&app).iter().all(|(y, _)| *y == 0.0), "only Top is drawn");
}

#[test]
fn view_from_bottom_side_mirrors_the_canvas() {
    let (mut app, _path) = open();
    panel(&mut app, PcbViewMsg::BoardFlipToggled);
    assert!(app.interaction_state.pcb_canvas.view_flipped());
}

#[test]
fn a_transparency_changes_the_track_alpha_and_an_object_eye_hides_tracks() {
    let (mut app, path) = open();
    let before = lines(&app);
    panel(
        &mut app,
        PcbViewMsg::ObjectTransparencySet {
            kind: BoardObject::Tracks,
            percent: 50,
        },
    );
    for ((_, a), (_, b)) in before.iter().zip(lines(&app).iter()) {
        assert!((b[3] - a[3] * 0.5).abs() < 1e-6, "{a:?} -> {b:?}");
    }
    assert_eq!(
        run(&mut app, &path, "set_object_transparency", &["kind=tracks", "value=0"]),
        CommandOutcome::Ran
    );
    assert_eq!(lines(&app), before);
    assert_eq!(
        run(&mut app, &path, "set_object_visible", &["kind=tracks", "on=false"]),
        CommandOutcome::Ran
    );
    assert!(snapshot(&app).traces.is_empty());
    assert_eq!(
        run(&mut app, &path, "set_object_visible", &["kind=all", "on=true"]),
        CommandOutcome::Ran
    );
    assert_eq!(snapshot(&app).traces.len(), 2);
}

#[test]
fn a_kind_the_view_cannot_honour_is_refused() {
    let (mut app, path) = open();
    for (name, operands) in [
        ("set_object_visible", vec!["kind=rooms", "on=false"]),
        ("set_object_transparency", vec!["kind=texts", "value=10"]),
        ("set_object_transparency", vec!["kind=tracks", "value=150"]),
        ("set_layer_visible", vec!["layer=Nowhere", "on=true"]),
    ] {
        assert!(
            matches!(
                run(&mut app, &path, name, &operands),
                CommandOutcome::Disabled { .. }
            ),
            "{name} {operands:?}"
        );
    }
}
