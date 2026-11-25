//! #2542: a wire placed into a named document snaps against that document's
//! hotspots, never against the active tab's. Sheet `a` is active and `b` is
//! named; each holds an off-grid wire end the other does not.

use signex_i18n::RenderTranslatable;
use signex_types::schematic::{Point, Wire};

use super::queries::tests::{path, sheet};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

const A: u128 = 100;
const B: u128 = 101;
/// `b`'s off-grid wire ends, on the 2.54 mm grid plus a little.
const B_END: (f64, f64) = (51.1, 50.9);
const B_OTHER_END: (f64, f64) = (61.1, 50.9);
/// `a`'s off-grid wire end, near `b`'s but further from the probe.
const A_END: (f64, f64) = (51.3, 51.3);
/// A hair off `B_END`: within the capture radius of both sheets' ends.
const PROBE: (f64, f64) = (51.25, 51.0);

fn upright_wire(at: (f64, f64)) -> Wire {
    Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(at.0, at.1),
        end: Point::new(at.0, at.1 - 10.0),
        ..Default::default()
    }
}

/// `b` opened first, then `a`, so `a` is the active tab with its own render
/// snapshot; snapping is on with both the grid and the electrical grid.
fn app() -> Signex {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    let mut b = sheet(B);
    b.wires.push(upright_wire(B_END));
    b.wires.push(upright_wire(B_OTHER_END));
    let mut a = sheet(A);
    a.wires.push(upright_wire(A_END));
    for (name, sheet) in [("b.snxsch", b), ("a.snxsch", a)] {
        fixture::add_sheet_with(&mut app, &path(name), sheet);
        app.document_state.mark_document_clean(&path(name));
    }
    fixture::activate(&mut app, &path("a.snxsch"));
    app.ui_state.snap_enabled = true;
    app.ui_state.snap_hotspots = true;
    app.ui_state.grid_size_mm = 2.54;
    app
}

/// `id` from `start` to `end` on the named sheet `b`.
fn run(app: &mut Signex, id: &str, start: (f64, f64), end: (f64, f64)) -> CommandOutcome {
    run_on(app, id, start, end, Some("b.snxsch"))
}

/// `id` from `start` to `end`, naming `document` or, with none, on the active sheet.
fn run_on(
    app: &mut Signex,
    id: &str,
    start: (f64, f64),
    end: (f64, f64),
    document: Option<&str>,
) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a valid id");
    let mut operands = vec![
        format!("start={},{}", start.0, start.1),
        format!("end={},{}", end.0, end.1),
    ];
    operands.extend(document.map(|file| format!("document={}", path(file).display())));
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands parse");
    app.run_operands_settled(&command, &args).0
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn wires(app: &Signex, file: &str) -> Vec<(Point, Point)> {
    app.document_state.engines[&path(file)]
        .document()
        .wires
        .iter()
        .map(|w| (w.start, w.end))
        .collect()
}

#[test]
fn a_wire_into_a_named_sheet_snaps_to_that_sheets_hotspots_not_the_active_tabs() {
    for id in ["place_wire", "draw_wire"] {
        let mut app = app();
        // The active tab's snapshot is `a` and pulls the probe onto `a`'s end,
        // so an answer from it would name `A_END`.
        let active = app.active_render_snapshot().expect("a's render snapshot");
        let near_a = signex_types::schematic::snap_to_hotspot(
            active,
            Point::new(PROBE.0, PROBE.1),
            signex_types::schematic::hotspot_capture_radius(2.54),
        );
        assert_eq!(near_a, Some(Point::new(A_END.0, A_END.1)), "{id}");
        let (a_before, b_before) = (wires(&app, "a.snxsch"), wires(&app, "b.snxsch"));

        // A hair off `b`'s end: a click on `b` lands on `b`'s end.
        let reason = refusal(run(&mut app, id, PROBE, B_OTHER_END));
        assert!(
            reason.contains(&format!("lands on {},{}", B_END.0, B_END.1)),
            "{id}: {reason}"
        );
        // `a`'s end is nothing on `b`: off the grid there, so refused.
        let reason = refusal(run(&mut app, id, A_END, B_OTHER_END));
        assert!(reason.contains("not where a click lands"), "{id}: {reason}");
        assert_eq!(wires(&app, "b.snxsch"), b_before, "{id}");

        // On `b`'s two ends exactly, the wire goes onto `b`, and `a` is untouched.
        let outcome = run(&mut app, id, B_END, B_OTHER_END);
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{id}: {outcome:?}"
        );
        let added = Point::new(B_END.0, B_END.1);
        let other = Point::new(B_OTHER_END.0, B_OTHER_END.1);
        assert!(
            wires(&app, "b.snxsch")
                .iter()
                .any(|&(s, e)| (s, e) == (added, other) || (s, e) == (other, added)),
            "{id}: {:?}",
            wires(&app, "b.snxsch")
        );
        assert_eq!(wires(&app, "a.snxsch"), a_before, "{id}");
    }
}

/// With no `document`, the snap reads the active sheet's engine, not its
/// render snapshot: geometry only the snapshot holds (as a drag preview would
/// draw it) is no hotspot, and the committed end is one.
#[test]
fn a_wire_on_the_active_sheet_snaps_to_its_committed_ends_not_the_drawn_copy() {
    /// Where the snapshot alone draws `a`'s off-grid wire, 5 mm to the right.
    const DRAWN_END: (f64, f64) = (A_END.0 + 5.0, A_END.1);
    const NEAR_DRAWN: (f64, f64) = (PROBE.0 + 5.0, PROBE.1);
    for id in ["place_wire", "draw_wire"] {
        let mut app = app();
        let mut drawn = app.document_state.engines[&path("a.snxsch")]
            .document()
            .clone();
        let moved = drawn
            .wires
            .iter_mut()
            .find(|w| (w.start.x, w.start.y) == A_END)
            .expect("a's off-grid wire");
        moved.start.x += 5.0;
        moved.end.x += 5.0;
        app.interaction_state
            .active_canvas_mut()
            .set_render_cache(Some(
                crate::schematic_runtime::SchematicRenderCache::from_sheet(&drawn),
            ));
        let before = wires(&app, "a.snxsch");

        // Near the drawn-only end: not a hotspot, so not pulled onto it.
        let reason = refusal(run_on(&mut app, id, NEAR_DRAWN, B_OTHER_END, None));
        assert!(
            !reason.contains(&format!("lands on {},{}", DRAWN_END.0, DRAWN_END.1)),
            "{id}: {reason}"
        );
        let reason = refusal(run_on(&mut app, id, DRAWN_END, B_OTHER_END, None));
        assert!(
            reason.contains("`start` is not where a click lands"),
            "{id}: {reason}"
        );

        // Near the committed end: pulled onto it.
        let reason = refusal(run_on(&mut app, id, PROBE, B_OTHER_END, None));
        assert!(
            reason.contains(&format!("lands on {},{}", A_END.0, A_END.1)),
            "{id}: {reason}"
        );
        assert_eq!(wires(&app, "a.snxsch"), before, "{id}");
    }
}
