//! #3058, #3057 (mockup rows 87, 88): a part outside its room run through
//! `Signex::update` is counted in the DRC panel's "other" tile and named by
//! the part and the room in Messages, as `check_drc` names it; the Run DRC
//! label sits in the middle of its button.

use std::path::PathBuf;

use serde_json::{Value, json};
use signex_drc::ViolationKind;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, PcbBoard, Room, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::app::command::queries::check_drc_tests::drc_serial;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::layout_probe::{LaidOutText, find, lay_out_texts};
use crate::panels::{DrcCategory, PanelKind, PanelMsg};

const BOARD: &str = "room.snxpcb";

/// R2, a 0.5 mm square pad part of sheet 0xA, at (11, 5): its body reaches
/// x = 11.25, 1.75 mm past the inside of room dcdc (0..10 mm, 0.5 mm
/// margin), which keeps the sheet's parts inside.
fn room_board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    let part: Footprint = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(2), "reference": "R2", "layer": "TopCopper",
        "position": {"x": 11.0, "y": 5.0},
        "pads": [{
            "uuid": Uuid::from_u128(20), "number": "1", "pad_type": "smd",
            "shape": "rect", "position": {"x": 0.0, "y": 0.0},
            "size": {"x": 0.5, "y": 0.5}, "drill": null,
            "layers": ["TopCopper"], "net": null
        }],
        "schematic_link": {"sheet": [Uuid::from_u128(0xA)], "units": [Uuid::from_u128(9)]}
    }))
    .expect("footprint");
    board.footprints.push(part);
    let mut room = Room::drawn(
        Uuid::from_u128(100),
        "dcdc".into(),
        "#A058C8",
        RoomOutline::Rect {
            x_mm: 0.0,
            y_mm: 0.0,
            w_mm: 10.0,
            h_mm: 10.0,
        },
    );
    room.source = Some(RoomSource {
        sheet: InstancePath::from_chain(vec![Uuid::from_u128(0xA)]),
        channel: None,
    });
    room.keep_parts_inside = true;
    room.margin_mm = 0.5;
    board.rooms.push(room);
    board
}

/// The board written and open in its tab, in the project.
fn room_app(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    // The built-in severities, whatever the shared test prefs hold.
    app.ui_state.machine_drc_rules = Default::default();
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    let path = dir.path().join(BOARD);
    let text = SnxPcb::new(room_board()).write_string().expect("encode");
    std::fs::write(&path, text).expect("write board");
    app.open_pcb_tab(path.clone(), BOARD.into(), room_board());
    (app, path)
}

/// Run DRC pressed in the DRC panel, settled; the Messages lines it wrote.
fn press_run_drc(app: &mut Signex) -> Vec<String> {
    let _ = crate::diagnostics::init_logging();
    let since = crate::diagnostics::newest_entry_id().unwrap_or(0);
    let task = app.update(Message::Dock(DockMessage::Panel(PanelMsg::RunDrc)));
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the run lands");
    app.refresh_panel_ctx();
    crate::diagnostics::recent_entries()
        .into_iter()
        .filter(|e| e.id > since && e.message.contains("DRC"))
        .map(|e| e.message)
        .collect()
}

/// The DRC panel laid out in a 300 px dock, bidi marks dropped.
fn panel_texts(app: &Signex) -> Vec<LaidOutText> {
    let view = crate::panels::view_panel(PanelKind::Drc, &app.document_state.panel_ctx);
    lay_out_texts(view, iced::Size::new(300.0, 2000.0))
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

fn centre_y(text: &LaidOutText) -> f32 {
    text.bounds.y + text.bounds.height / 2.0
}

/// #3058: the room finding is counted in the "other" tile under the four,
/// its row is listed, and the "other" tile filters to it.
#[test]
fn a_room_finding_is_counted_in_the_other_tile() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = room_app(&dir);
    press_run_drc(&mut app);
    let done = app.interaction_state.pcb_drc.done_for(&path).expect("ran");
    let kinds: Vec<ViolationKind> = done.report.violations.iter().map(|v| v.kind).collect();
    assert!(kinds.contains(&ViolationKind::PartOutsideRoom), "{kinds:?}");
    let others = kinds
        .iter()
        .filter(|k| DrcCategory::of(**k) == DrcCategory::Other)
        .count();
    let panel = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    assert_eq!(panel.other, others);
    assert_eq!(
        panel.clearance + panel.short + panel.unconnected + panel.keepout + panel.other,
        kinds.len(),
        "every finding is in a tile"
    );

    let laid = panel_texts(&app);
    let other = find(&laid, "other");
    let keepout = find(&laid, "keepout");
    // Under the four, at the left, one tile wide.
    assert!(other.bounds.y > keepout.bounds.y, "{laid:#?}");
    assert!(other.bounds.x < keepout.bounds.x, "{other:?} {keepout:?}");
    let count = others.to_string();
    assert!(
        laid.iter()
            .any(|t| t.text == count && (t.bounds.x - other.bounds.x).abs() < 1.0),
        "the tile reads {count}: {laid:#?}"
    );

    let pressed = crate::layout_probe::messages_on_click(
        crate::panels::view_panel(PanelKind::Drc, &app.document_state.panel_ctx),
        iced::Size::new(300.0, 2000.0),
        "other",
    );
    for msg in pressed {
        let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
    }
    app.refresh_panel_ctx();
    let panel = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    assert_eq!(panel.view.category, Some(DrcCategory::Other));
    let texts: Vec<String> = panel_texts(&app).into_iter().map(|t| t.text).collect();
    assert!(
        texts.iter().any(|t| t == "Showing other violations only"),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.starts_with("Part outside its room")),
        "{texts:?}"
    );
}

/// #3058: Messages names the part and the room, never "net 0 / net 0",
/// and `check_drc` answers the same words and count headless.
#[test]
fn a_room_finding_is_named_by_its_part_and_room_in_messages_and_check_drc() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = room_app(&dir);
    let lines = press_run_drc(&mut app);
    let room_lines: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("outside room"))
        .collect();
    assert_eq!(room_lines.len(), 1, "{lines:?}");
    assert!(
        room_lines[0].starts_with("DRC: R2 is 1.75 mm outside room dcdc at ("),
        "{lines:?}"
    );
    assert!(!room_lines[0].contains("net 0"), "{lines:?}");

    let id = AppCommandId::new("check_drc").expect("a catalog id");
    let document = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&id, &[document.as_str()]).expect("operands parse");
    let answer = match app.run_query(&id, &args).expect("a query") {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    };
    let report = match answer {
        CommandOutcome::Answered(report) => report,
        other => panic!("not answered: {other:?}"),
    };
    let rooms: Vec<&Value> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == ViolationKind::PartOutsideRoom.rule_id())
        .collect();
    assert_eq!(rooms.len(), 1, "{report}");
    assert_eq!(rooms[0]["message"], "R2 is 1.75 mm outside room dcdc");
}

/// #3057 (mockup row 88): after a run the Run DRC label is level with
/// Rules, the button beside it, in the middle of its button. Before a run:
/// `panels::drc::tests::the_run_drc_label_is_centred_in_its_button`.
#[test]
fn the_run_drc_label_is_level_with_rules_after_a_run() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = room_app(&dir);
    press_run_drc(&mut app);
    let laid = panel_texts(&app);
    let (run, rules) = (find(&laid, "Run DRC"), find(&laid, "Rules"));
    assert!(
        (centre_y(run) - centre_y(rules)).abs() < 1.0,
        "Run DRC at {}, Rules at {}",
        centre_y(run),
        centre_y(rules)
    );
}
