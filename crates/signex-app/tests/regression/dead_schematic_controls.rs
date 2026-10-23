//! #791 — four schematic controls that used to do nothing: View > Cycle
//! Grid Size, View > Zoom In / Zoom Out, and the Annotate dialog's
//! "Update Changes List" (removed: the preview is live) and "All On" /
//! "All Off" (were placeholders that closed the dialog).

use iced::{Point as ScreenPoint, Rectangle};
use signex_app::app::{AnnotateMsg, FileMsg, Message, Signex, UiMsg, WindowMsg};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::ShortcutTrigger;
use signex_app::menu_bar::MenuMessage;
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use std::collections::HashMap;
use std::path::PathBuf;

fn symbol(uuid: uuid::Uuid, reference: &str, x: f64) -> Symbol {
    Symbol {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid,
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: String::new(),
        datasheet: String::new(),
        position: Point::new(x, 0.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols,
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

fn app_with(symbols: Vec<Symbol>) -> (Signex, PathBuf) {
    let path = PathBuf::from("dead-controls.snxsch");
    let engine = signex_engine::Engine::new(sheet_with(symbols)).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "dead-controls".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());

    (app, path)
}

const VIEWPORT: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

#[test]
fn cycle_grid_steps_snap_and_linked_visible_grid_through_the_presets() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.visible_grid_mm = 1.27;

    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 2.54);
    assert_eq!(app.ui_state.visible_grid_mm, 2.54);

    // Wraps from the last preset back to the first.
    app.ui_state.grid_size_mm = 5.08;
    app.ui_state.visible_grid_mm = 5.08;
    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 0.635);
    assert_eq!(app.ui_state.visible_grid_mm, 0.635);
}

#[test]
fn ui_grid_cycle_message_steps_the_grid_like_the_menu_item() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 2.54;
    app.ui_state.visible_grid_mm = 2.54;

    let _ = app.update(Message::Ui(UiMsg::GridCycle));
    assert_eq!(app.ui_state.grid_size_mm, 5.08);
    assert_eq!(app.ui_state.visible_grid_mm, 5.08);
}

#[test]
fn cycle_grid_from_a_non_preset_size_lands_on_the_first_preset() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 0.5;
    app.ui_state.visible_grid_mm = 0.5;

    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 0.635);
}

#[test]
fn cycle_grid_leaves_a_deliberately_different_visible_grid_alone() {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.visible_grid_mm = 5.08;

    let _ = app.update(Message::Menu(MenuMessage::CycleGrid));
    assert_eq!(app.ui_state.grid_size_mm, 2.54);
    assert_eq!(app.ui_state.visible_grid_mm, 5.08);
}

#[test]
fn zoom_in_and_out_change_scale_and_keep_the_view_centre_fixed() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    let centre = ScreenPoint::new(400.0, 300.0);
    let world_at_centre = |app: &Signex| {
        let (ox, oy, s) = app.interaction_state.active_canvas().live_camera();
        ((centre.x - ox) / s, (centre.y - oy) / s)
    };
    let (_, _, s0) = app.interaction_state.active_canvas().live_camera();
    let w0 = world_at_centre(&app);

    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let (_, _, s1) = app.interaction_state.active_canvas().live_camera();
    assert!(
        (s1 / s0 - 1.1).abs() < 1e-4,
        "one notch in = x1.1, got {}",
        s1 / s0
    );
    let w1 = world_at_centre(&app);
    assert!((w1.0 - w0.0).abs() < 1e-3 && (w1.1 - w0.1).abs() < 1e-3);

    let _ = app.update(Message::Menu(MenuMessage::ZoomOut));
    let (_, _, s2) = app.interaction_state.active_canvas().live_camera();
    assert!((s2 - s0).abs() < 1e-3, "in then out returns to the start");
}

#[test]
fn zoom_stops_at_the_camera_limits() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    for _ in 0..200 {
        let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    }
    let (_, _, max) = app.interaction_state.active_canvas().live_camera();
    for _ in 0..200 {
        let _ = app.update(Message::Menu(MenuMessage::ZoomOut));
    }
    let (_, _, min) = app.interaction_state.active_canvas().live_camera();
    assert!(
        (max / min - 64.0).abs() < 0.5,
        "25%..1600% = 64x range, got {}",
        max / min
    );
}

#[test]
fn pcb_canvas_zooms_about_its_centre() {
    let pcb = signex_app::pcb_canvas::PcbCanvas::new();
    pcb.set_viewport(VIEWPORT);
    let (ox0, oy0, s0) = pcb.live_camera();
    let w0 = ((400.0 - ox0) / s0, (300.0 - oy0) / s0);
    assert!(pcb.zoom_about_centre(true));
    let (ox1, oy1, s1) = pcb.live_camera();
    assert!(s1 > s0);
    let w1 = ((400.0 - ox1) / s1, (300.0 - oy1) / s1);
    assert!((w1.0 - w0.0).abs() < 1e-3 && (w1.1 - w0.1).abs() < 1e-3);
    assert!(pcb.zoom_about_centre(false));
}

#[test]
fn annotate_all_off_locks_every_listed_symbol_and_all_on_unlocks_them() {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let (mut app, _path) = app_with(vec![symbol(a, "R?", 0.0), symbol(b, "R?", 10.0)]);

    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(true)));
    assert!(app.ui_state.annotate_locked.contains(&a));
    assert!(app.ui_state.annotate_locked.contains(&b));

    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(false)));
    assert!(app.ui_state.annotate_locked.is_empty());
}

#[test]
fn annotate_all_on_off_keep_the_dialog_open() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R?", 0.0)]);
    let _ = app.update(Message::Annotate(AnnotateMsg::OpenDialog));
    assert!(app.ui_state.annotate_dialog_open);
    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(true)));
    assert!(app.ui_state.annotate_dialog_open);
    let _ = app.update(Message::Annotate(AnnotateMsg::SetAllLocked(false)));
    assert!(app.ui_state.annotate_dialog_open);
}

fn cursor_at(app: &mut Signex, x: f32, y: f32) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::CursorAt {
        x,
        y,
        zoom_pct: 100.0,
    }));
}

/// The world point at the centre of an 800 x 600 view.
fn view_centre((ox, oy, scale): (f32, f32, f32)) -> (f32, f32) {
    ((400.0 - ox) / scale, (300.0 - oy) / scale)
}

fn add_second_sheet(app: &mut Signex) -> PathBuf {
    let path = PathBuf::from("dead-controls-other.snxsch");
    let engine = signex_engine::Engine::new(sheet_with(Vec::new())).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "dead-controls-other".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    path
}

/// #525: Center View At Cursor recentres on the cursor and keeps the zoom.
/// It used to share the Fit All arm and zoomed out to the whole sheet. It
/// also cancels a fit still queued from before, which would otherwise land
/// on the next canvas tick and undo the centring.
#[test]
fn center_view_at_cursor_recentres_without_changing_zoom_and_cancels_a_queued_fit() {
    let (mut app, path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    let sheet = app.document_state.engines[&path].document().clone();
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    cursor_at(&mut app, 50.0, 30.0);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));
    assert!(
        app.interaction_state
            .active_canvas()
            .pending_fit
            .get()
            .is_some(),
        "precondition: a fit is queued"
    );
    let (_, _, s0) = app.interaction_state.active_canvas().live_camera();

    let _ = app.update(Message::CanvasEvent(CanvasEvent::CenterAtCursor));

    let camera = app.interaction_state.active_canvas().live_camera();
    assert!(
        (camera.2 - s0).abs() < 1e-6,
        "zoom must not change: {s0} -> {}",
        camera.2
    );
    let centre = view_centre(camera);
    assert!(
        (centre.0 - 50.0).abs() < 1e-3 && (centre.1 - 30.0).abs() < 1e-3,
        "cursor point must land at the view centre, got {centre:?}"
    );
    assert!(
        app.interaction_state
            .active_canvas()
            .pending_fit
            .get()
            .is_none(),
        "the queued fit must be cancelled"
    );
}

/// #525 review: the cursor point belongs to the document it was reported
/// over. Before any hover, or once another tab is active, there is no point
/// to centre on and the view stays put.
#[test]
fn center_view_at_cursor_ignores_a_point_from_another_document() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    let before = app.interaction_state.active_canvas().live_camera();

    let _ = app.update(Message::CanvasEvent(CanvasEvent::CenterAtCursor));
    assert_eq!(
        app.interaction_state.active_canvas().live_camera(),
        before,
        "no hover yet: nothing to centre on"
    );

    cursor_at(&mut app, 50.0, 30.0);
    let other = add_second_sheet(&mut app);
    // The tab switch: the point above was reported over the first sheet.
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(other);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::CenterAtCursor));
    assert_eq!(
        app.interaction_state.active_canvas().live_camera(),
        before,
        "a point from another sheet must not move this one"
    );
}

/// #862 review: a PCB tab has no `active_path`, so the cursor point is keyed
/// on the active tab there. Center View At Cursor recentres a board too.
#[test]
fn center_view_at_cursor_recentres_a_pcb() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        path: PathBuf::from("/tmp/does-not-matter/board.snxpcb"),
        title: "board".to_string(),
        result: Ok(Box::new(empty_pcb_board())),
    }));
    app.interaction_state.pcb_canvas.set_viewport(VIEWPORT);
    let (_, _, s0) = app.interaction_state.pcb_canvas.live_camera();
    cursor_at(&mut app, 50.0, 30.0);

    let _ = app.update(Message::CanvasEvent(CanvasEvent::CenterAtCursor));

    let camera = app.interaction_state.pcb_canvas.live_camera();
    assert!((camera.2 - s0).abs() < 1e-6, "zoom must not change");
    let centre = view_centre(camera);
    assert!(
        (centre.0 - 50.0).abs() < 1e-3 && (centre.1 - 30.0).abs() < 1e-3,
        "the board view must centre on the cursor point, got {centre:?}"
    );
}

fn empty_pcb_board() -> signex_types::pcb::PcbBoard {
    signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
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

/// #525 review: `Home` typed in an undocked sheet's window centres that
/// window's view on its own cursor point, and leaves the main window alone.
/// The plain `CanvasEvent` the command resolves to always targets the main
/// window, so the keystroke has to be routed to the window it came from.
#[test]
fn home_in_an_undocked_window_centres_that_window_not_the_main_one() {
    let (mut app, _path) = app_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]);
    app.interaction_state.active_canvas().set_viewport(VIEWPORT);
    let other = add_second_sheet(&mut app);
    let window_id = iced::window::Id::unique();
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: other,
        id: window_id,
    }));
    app.interaction_state.canvases[&window_id].set_viewport(VIEWPORT);
    let _ = app.update(Message::CanvasEventInWindow {
        window_id,
        event: CanvasEvent::CursorAt {
            x: 50.0,
            y: 30.0,
            zoom_pct: 100.0,
        },
    });
    let main_before = app.interaction_state.active_canvas().live_camera();

    let ShortcutTrigger::KeySequence(mut strokes) =
        ShortcutTrigger::parse("Home").expect("Home parses")
    else {
        panic!("Home is a key, not a gesture");
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: Some(window_id),
        stroke: strokes.remove(0),
    }));

    assert_eq!(
        app.interaction_state.active_canvas().live_camera(),
        main_before,
        "the main window must not move"
    );
    let centre = view_centre(app.interaction_state.canvases[&window_id].live_camera());
    assert!(
        (centre.0 - 50.0).abs() < 1e-3 && (centre.1 - 30.0).abs() < 1e-3,
        "the undocked view must centre on its cursor point, got {centre:?}"
    );
}

/// #525: Zoom to All Objects frames the objects, not the paper, so parts
/// bunched in one corner fill the view.
#[test]
fn zoom_to_all_objects_frames_object_extents_not_the_paper() {
    let (mut app, path) = app_with(vec![
        symbol(uuid::Uuid::new_v4(), "R1", 300.0),
        symbol(uuid::Uuid::new_v4(), "R2", 320.0),
    ]);
    // Stand in for the first draw, which builds the render cache the fit reads.
    let sheet = app.document_state.engines[&path].document().clone();
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitAll));

    let rect = app
        .interaction_state
        .active_canvas()
        .pending_fit
        .get()
        .expect("fit queued");
    // Object extents plus the 10 mm margin: x 290..330, far smaller than A4.
    assert!((rect.x - 290.0).abs() < 1e-3, "{rect:?}");
    assert!((rect.width - 40.0).abs() < 1e-3, "{rect:?}");
}
