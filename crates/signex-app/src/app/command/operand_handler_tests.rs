//! #2155 / #661: a command runs its operands only through its own operand
//! handler, every handler does what the click or button of the same id does,
//! and a command that declares operands without a handler is refused.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::schematic::{Point, SchematicSheet, Wire};
use signex_types::theme::Color;
use uuid::Uuid;

use super::operand_handlers::{handled_ids, operand_handler};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::keymap::{AppCommandId, OperandKind, all_command_ids, metadata_for_str};

const WIRE_A: u128 = 1;
const WIRE_B: u128 = 2;
const WIRE_FAR: u128 = 3;

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn wire(n: u128, from: (f64, f64), to: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::from_u128(n),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        ..Default::default()
    }
}

/// Two wires meeting at (10, 0) and one far away: a click on the first
/// colours the first two.
fn sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::from_u128(99),
        paper_size: "A4".to_string(),
        wires: vec![
            wire(WIRE_A, (0.0, 0.0), (10.0, 0.0)),
            wire(WIRE_B, (10.0, 0.0), (10.0, 10.0)),
            wire(WIRE_FAR, (30.0, 0.0), (40.0, 0.0)),
        ],
        ..Default::default()
    }
}

fn path() -> PathBuf {
    std::env::temp_dir().join("operands.snxsch")
}

/// An app with the sheet open and clean, snapping off so a click lands
/// exactly where it is aimed.
fn app() -> Signex {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = false;
    colours::add_sheet_with(&mut app, &path(), sheet());
    app.document_state.mark_document_clean(&path());
    app
}

fn colours_of(app: &Signex) -> HashMap<Uuid, Color> {
    app.document_state.engines[&path()]
        .document()
        .net_colors
        .clone()
}

fn depth(app: &Signex) -> usize {
    app.document_state.engines[&path()].undo_position()
}

fn parse(command: &str, args: &[&str]) -> CommandArgs {
    CommandArgs::parse_operands(&id(command), args).expect("operands parse")
}

fn click_with_pen(app: &mut Signex, at: (f64, f64), color: Color) {
    app.ui_state.pending_net_color = Some(color);
    let _ = app.update(crate::app::Message::CanvasEvent(
        crate::canvas::CanvasEvent::Clicked {
            world_x: at.0,
            world_y: at.1,
        },
    ));
}

const BLUE: Color = Color::new(0x33, 0x66, 0x99, 255);

#[test]
fn color_net_by_operands_is_the_pen_click() {
    let mut clicked = app();
    click_with_pen(&mut clicked, (5.0, 0.0), BLUE);

    let mut by_command = app();
    let command = id("color_net");
    let (outcome, _) =
        by_command.run_operands(&command, &parse("color_net", &["at=5,0", "color=#336699"]));

    assert_eq!(outcome, CommandOutcome::Ran);
    let painted = colours_of(&by_command);
    assert_eq!(painted.len(), 2, "the flood colours the whole net");
    assert!(!painted.contains_key(&Uuid::from_u128(WIRE_FAR)));
    assert_eq!(
        painted,
        colours_of(&clicked),
        "the click made the same edit"
    );
    assert_eq!(depth(&by_command), depth(&clicked));
    assert!(by_command.document_state.is_document_dirty(&path()));
    assert_eq!(
        by_command.document_state.is_document_dirty(&path()),
        clicked.document_state.is_document_dirty(&path())
    );
}

#[test]
fn the_colour_operand_changes_the_result() {
    let mut red = app();
    let mut blue = app();
    let command = id("color_net");
    let (_, _task) = red.run_operands(&command, &parse("color_net", &["at=5,0", "color=ff0000"]));
    let (_, _task) = blue.run_operands(&command, &parse("color_net", &["at=5,0", "color=#336699"]));
    assert_ne!(colours_of(&red), colours_of(&blue));
    assert_eq!(
        colours_of(&red).get(&Uuid::from_u128(WIRE_A)),
        Some(&Color::new(255, 0, 0, 255))
    );
}

#[test]
fn the_point_operand_picks_the_net() {
    let mut app = app();
    let command = id("color_net");
    let (_, _task) = app.run_operands(&command, &parse("color_net", &["at=35,0", "color=#336699"]));
    let painted = colours_of(&app);
    assert_eq!(painted.len(), 1);
    assert!(painted.contains_key(&Uuid::from_u128(WIRE_FAR)));
}

#[test]
fn color_net_is_one_undo_step_that_reads_clean_when_undone() {
    let mut app = app();
    let command = id("color_net");
    let (_, _task) = app.run_operands(&command, &parse("color_net", &["at=5,0", "color=#336699"]));
    assert!(app.document_state.is_document_dirty(&path()));

    app.handle_undo_requested();

    assert!(
        colours_of(&app).is_empty(),
        "one undo takes the whole flood"
    );
    assert!(
        !app.document_state.is_document_dirty(&path()),
        "back at save"
    );
}

#[test]
fn color_net_refuses_what_it_cannot_do_and_says_why() {
    let command = id("color_net");
    let refuse = |app: &mut Signex, args: &[&str]| {
        let (outcome, _) = app.run_operands(&command, &parse("color_net", args));
        match outcome {
            CommandOutcome::Disabled { reason } => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    };

    let mut app = app();
    let reason = refuse(&mut app, &["at=20,20", "color=#336699"]);
    assert!(reason.contains("no wire"), "{reason}");

    let elsewhere = std::env::temp_dir().join("elsewhere.snxsch");
    let given = format!("path={}", elsewhere.display());
    let reason = refuse(&mut app, &["at=5,0", "color=#336699", &given]);
    assert!(reason.contains("not the active document"), "{reason}");
    assert!(colours_of(&app).is_empty(), "a refusal changes nothing");

    let (_, _task) = app.run_operands(&command, &parse("color_net", &["at=5,0", "color=#336699"]));
    let reason = refuse(&mut app, &["at=5,0", "color=#336699"]);
    assert!(reason.contains("already"), "{reason}");
    assert_eq!(depth(&app), 1, "the repeat recorded no second step");
}

#[test]
fn color_net_on_a_read_only_sheet_is_refused() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = false;
    let mut read_only = sheet();
    read_only.unknown.newer_minor_source = Some("snxsch/1.7".into());
    colours::add_sheet_with(&mut app, &path(), read_only);

    let (outcome, _) = app.run_operands(
        &id("color_net"),
        &parse("color_net", &["at=5,0", "color=#336699"]),
    );

    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(colours_of(&app).is_empty());
}

#[test]
fn a_bad_colour_is_refused_when_the_operands_parse() {
    let command = id("color_net");
    for bad in [
        "color=red",
        "color=#12345",
        "color=#1234567",
        "color=#gg0000",
        "color=",
    ] {
        assert!(
            CommandArgs::parse_operands(&command, &["at=5,0", bad]).is_err(),
            "{bad}"
        );
    }
}

fn painted_app() -> Signex {
    let mut app = app();
    let (_, _task) = app.run_operands(
        &id("color_net"),
        &parse("color_net", &["at=5,0", "color=#336699"]),
    );
    app.document_state.mark_document_clean(&path());
    app
}

#[test]
fn clear_all_net_colors_by_operands_is_the_button() {
    let mut button = painted_app();
    let (outcome, _) =
        button.dispatch_command_reporting(&id("clear_all_net_colors"), CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);

    let mut by_command = painted_app();
    let given = format!("path={}", path().display());
    let (outcome, _) = by_command.run_operands(
        &id("clear_all_net_colors"),
        &parse("clear_all_net_colors", &[&given]),
    );

    assert_eq!(outcome, CommandOutcome::Ran);
    assert!(colours_of(&by_command).is_empty());
    assert_eq!(colours_of(&by_command), colours_of(&button));
    assert_eq!(depth(&by_command), depth(&button));
    assert!(by_command.document_state.is_document_dirty(&path()));
    assert_eq!(
        by_command.document_state.is_document_dirty(&path()),
        button.document_state.is_document_dirty(&path())
    );

    by_command.handle_undo_requested();
    assert_eq!(
        colours_of(&by_command).len(),
        2,
        "one undo restores the sheet"
    );
    assert!(!by_command.document_state.is_document_dirty(&path()));
}

#[test]
fn the_path_operand_picks_the_sheet_a_clear_all_acts_on() {
    let mut app = painted_app();
    let other = std::env::temp_dir().join("other.snxsch");
    colours::add_sheet_with(&mut app, &other, sheet());
    colours::paint_on(&mut app, &other, &[Uuid::from_u128(WIRE_A)], BLUE);
    let given = format!("path={}", other.display());

    let (outcome, _) = app.run_operands(
        &id("clear_all_net_colors"),
        &parse("clear_all_net_colors", &[&given]),
    );

    assert_eq!(outcome, CommandOutcome::Ran);
    assert!(
        app.document_state.engines[&other]
            .document()
            .net_colors
            .is_empty()
    );
    assert_eq!(
        colours_of(&app).len(),
        2,
        "the other sheet kept its colours"
    );

    let missing = std::env::temp_dir().join("never-opened.snxsch");
    let (outcome, _) = app.run_operands(
        &id("clear_all_net_colors"),
        &parse(
            "clear_all_net_colors",
            &[&format!("path={}", missing.display())],
        ),
    );
    assert!(matches!(outcome, CommandOutcome::Disabled { .. }));
}

#[test]
fn clear_all_net_colors_with_nothing_painted_says_so() {
    let mut app = app();
    let given = format!("path={}", path().display());
    let (outcome, _) = app.run_operands(
        &id("clear_all_net_colors"),
        &parse("clear_all_net_colors", &[&given]),
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("no net colours")),
        "{outcome:?}"
    );
}

/// One valid `key=value` per operand a command declares.
fn sample_operands(id: &str) -> Vec<String> {
    let meta = metadata_for_str(id).expect("catalog row");
    meta.operands
        .iter()
        .map(|op| {
            let value = match op.kind {
                OperandKind::Point => "1,2".to_string(),
                OperandKind::Angle => "0".to_string(),
                OperandKind::Text => "x".to_string(),
                OperandKind::Path => std::env::temp_dir().join("x.snxsch").display().to_string(),
                OperandKind::Bool => "true".to_string(),
                OperandKind::Color => "#336699".to_string(),
            };
            format!("{}={value}", op.name)
        })
        .collect()
}

/// A command that declares operands has a handler or is refused as not run
/// yet; a handler on a command that declares none is a handler nobody can
/// reach. Adding either without the other fails here.
#[test]
fn every_operand_command_has_a_handler_or_is_refused() {
    let mut handled = 0;
    for name in all_command_ids() {
        let declares = !metadata_for_str(name).expect("row").operands.is_empty();
        let command = id(name);
        assert_eq!(
            operand_handler(&command).is_some(),
            handled_ids().any(|h| h == name),
            "{name}"
        );
        if !declares {
            assert!(
                operand_handler(&command).is_none(),
                "{name} has an operand handler but declares no operands"
            );
            continue;
        }
        if operand_handler(&command).is_some() {
            handled += 1;
            continue;
        }
        let owned = sample_operands(name);
        let args: Vec<&str> = owned.iter().map(String::as_str).collect();
        let parsed = CommandArgs::parse_operands(&command, &args).expect("sample operands parse");
        let (mut app, _task) = Signex::new();
        let (outcome, _) = app.run_operands(&command, &parsed);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("not run yet")),
            "{name} declares operands, has no handler, and was not refused: {outcome:?}"
        );
    }
    assert_eq!(
        handled,
        handled_ids().count(),
        "a handler names no catalog row"
    );
    assert!(handled >= 2);
}
