//! #1050 — the Bus Entry tool through the real message path: the bus end
//! snaps onto a bus, and Space / Shift+Space / X / Y orient the entry both
//! while placing and once it is selected.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, Tool, UiMsg};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{
    Bus, BusEntry, Point, SCHEMATIC_BUS_ENTRY_SIZE_MM as K, SelectedItem, SelectedKind,
};
use tempfile::TempDir;
use uuid::Uuid;

/// A bus given as its two end points.
type BusEnds = ((f64, f64), (f64, f64));

fn app_with_buses(buses: &[BusEnds]) -> (Signex, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("root.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    let mut sheet = super::place_sheet_tool::empty_sheet();
    for (start, end) in buses {
        sheet.buses.push(Bus {
            uuid: Uuid::now_v7(),
            start: Point::new(start.0, start.1),
            end: Point::new(end.0, end.1),
        });
    }
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::BusEntry)));
    (app, tmp)
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn entries(app: &Signex) -> Vec<BusEntry> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .bus_entries
        .clone()
}

/// Snapped clicks land on exact decimal grid points, so compare tightly.
fn assert_at(e: &BusEntry, x: f64, y: f64) {
    let (dx, dy) = ((e.position.x - x).abs(), (e.position.y - y).abs());
    assert!(
        dx < 1e-9 && dy < 1e-9,
        "at ({}, {}), want ({x}, {y})",
        e.position.x,
        e.position.y
    );
}

fn press(app: &mut Signex, msg: EditMsg) {
    let _ = app.update(Message::Edit(msg));
}

#[test]
fn entry_snaps_onto_a_horizontal_bus() {
    let (mut app, _tmp) = app_with_buses(&[((0.0, 12.7), (50.8, 12.7))]);
    // One grid step below the bus: the bus end lands on the bus, same x.
    click(&mut app, 25.4, 13.97);
    let placed = entries(&app);
    assert_eq!(placed.len(), 1);
    assert_at(&placed[0], 25.4, 12.7);
    assert_eq!(placed[0].size, (K, K), "untouched orientation is today's");
}

#[test]
fn entry_snaps_onto_a_vertical_bus() {
    let (mut app, _tmp) = app_with_buses(&[((20.32, 0.0), (20.32, 50.8))]);
    click(&mut app, 21.59, 25.4);
    let placed = entries(&app);
    assert_at(&placed[0], 20.32, 25.4);
}

#[test]
fn entry_far_from_any_bus_lands_on_the_grid_point() {
    let (mut app, _tmp) = app_with_buses(&[((0.0, 12.7), (50.8, 12.7))]);
    click(&mut app, 25.4, 30.48);
    let placed = entries(&app);
    assert_at(&placed[0], 25.4, 30.48);
}

#[test]
fn rotate_and_mirror_orient_the_entry_before_it_is_placed() {
    let (mut app, _tmp) = app_with_buses(&[((0.0, 12.7), (50.8, 12.7))]);
    press(&mut app, EditMsg::RotateSelectedCounterclockwise);
    click(&mut app, 10.16, 12.7);
    press(&mut app, EditMsg::RotateSelectedClockwise);
    press(&mut app, EditMsg::RotateSelectedClockwise);
    click(&mut app, 20.32, 12.7);
    press(&mut app, EditMsg::MirrorSelectedY); // X key: left-right
    click(&mut app, 30.48, 12.7);
    press(&mut app, EditMsg::MirrorSelectedX); // Y key: top-bottom
    click(&mut app, 40.64, 12.7);

    let sizes: Vec<(f64, f64)> = entries(&app).iter().map(|e| e.size).collect();
    assert_eq!(
        sizes,
        vec![
            (K, -K), // one turn counter-clockwise from (K, K)
            (-K, K), // two clockwise turns more
            (K, K),  // X key flips x: (-K, K) -> (K, K)
            (K, -K), // Y key flips y
        ]
    );
}

#[test]
fn rotate_and_mirror_orient_a_selected_entry() {
    let (mut app, _tmp) = app_with_buses(&[((0.0, 12.7), (50.8, 12.7))]);
    click(&mut app, 25.4, 12.7);
    let uuid = entries(&app)[0].uuid;
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::BusEntry)];

    press(&mut app, EditMsg::RotateSelectedCounterclockwise);
    assert_eq!(entries(&app)[0].size, (K, -K));
    press(&mut app, EditMsg::MirrorSelectedY);
    assert_eq!(entries(&app)[0].size, (-K, -K));
    press(&mut app, EditMsg::MirrorSelectedX);
    assert_eq!(entries(&app)[0].size, (-K, K));
    assert_at(&entries(&app)[0], 25.4, 12.7); // the bus end stays put
}

/// Space / Shift+Space resolve through the real keymap and command bridge to
/// the bus-entry handlers while the tool is armed.
#[test]
fn space_and_shift_space_keys_orient_the_armed_entry() {
    let (mut app, _tmp) = app_with_buses(&[((0.0, 12.7), (50.8, 12.7))]);
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    let space = |app: &mut Signex, shift: bool| {
        let stroke = KeyStroke {
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            key: KeyToken::Space,
        };
        let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
            window: None,
            stroke,
        }));
    };
    space(&mut app, false);
    click(&mut app, 10.16, 12.7);
    space(&mut app, true);
    space(&mut app, true);
    click(&mut app, 20.32, 12.7);
    let sizes: Vec<(f64, f64)> = entries(&app).iter().map(|e| e.size).collect();
    assert_eq!(sizes, vec![(K, -K), (-K, K)]);
}
