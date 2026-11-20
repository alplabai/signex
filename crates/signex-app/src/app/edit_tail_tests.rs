//! #2445: what follows a schematic edit, an undo or an agent's edit list
//! (ERC epoch, dirty mark, page context, library drift check, the owed
//! repaint) is decided from the engine's `DocumentPatch`. These tests pin
//! those decisions and the projection to the render cache's invalidation.

use serde_json::json;
use signex_engine::{Command, DocumentPatch as P};
use signex_types::schematic::Point;

use crate::app::Signex;
use crate::app::command::queries::tests::{WIRE_A, app_with, path, uid};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::keymap::AppCommandId;
use crate::schematic_runtime::{RenderInvalidation as R, SchematicRenderCache};

fn project(patch: P) -> R {
    R::from(patch)
}

/// Every single-bit patch and the render bits it projects to.
const TABLE: [(P, R); 18] = [
    (P::SYMBOLS, R::SYMBOLS),
    (P::WIRES, R::WIRES),
    (P::LABELS, R::LABELS),
    (P::TEXT_NOTES, R::TEXT_NOTES),
    (P::BUSES, R::BUSES),
    (P::BUS_ENTRIES, R::BUS_ENTRIES),
    (P::JUNCTIONS, R::JUNCTIONS),
    (P::NO_CONNECTS, R::NO_CONNECTS),
    (P::CHILD_SHEETS, R::CHILD_SHEETS),
    (P::DRAWINGS, R::DRAWINGS),
    (P::LIB_SYMBOLS, R::LIB_SYMBOLS),
    (P::PAPER, R::PAPER),
    (P::NO_ERC_DIRECTIVES, R::NO_ERC_DIRECTIVES),
    (P::FONT_FAMILY, R::NONE), // four bits, checked on its own below
    (P::NET_COLORS, R::NET_COLORS),
    (P::DIRECTIVES, R::DIRECTIVES),
    (P::HARNESS, R::HARNESS),
    (P::UNIONS, R::UNIONS),
];

#[test]
fn the_projection_maps_every_bit_and_or_commutes_with_it() {
    for (patch, render) in TABLE {
        if patch == P::FONT_FAMILY {
            continue;
        }
        assert_eq!(project(patch), render, "{patch:?}");
    }
    assert_eq!(
        project(P::FONT_FAMILY),
        R::SYMBOLS | R::LABELS | R::TEXT_NOTES | R::CHILD_SHEETS,
        "the sheet font is inherited by every kind of text"
    );
    assert_eq!(project(P::NONE), R::NONE);
    assert_eq!(project(P::FULL), R::FULL);
    for (a, _) in TABLE {
        for (b, _) in TABLE {
            assert_eq!(project(a | b), project(a) | project(b), "{a:?} | {b:?}");
        }
    }
    // Every bit together is FULL, which projects to FULL and not to the OR
    // of the per-bit projections: the one place OR does not commute.
    let all = TABLE.iter().fold(P::NONE, |acc, (patch, _)| acc | *patch);
    assert_eq!(all, P::FULL);
    assert_eq!(project(all), R::FULL);
}

fn epoch(app: &Signex) -> u64 {
    app.document_state.sheet_edit_epoch
}

#[test]
fn undo_and_redo_of_a_colour_step_leave_the_erc_epoch_alone() {
    let (mut app, path) = colours::app_with_sheet("tail-colour.snxsch");
    let wire = colours::wire_of(&app, &path);
    colours::paint(&mut app, &[wire], colours::color(1));
    let before = epoch(&app);

    app.handle_undo_requested();
    assert_eq!(colours::colour_of(&app, wire), None, "the colour is undone");
    assert_eq!(epoch(&app), before, "an undone colour changes no finding");
    app.handle_redo_requested();
    assert_eq!(colours::colour_of(&app, wire), Some(colours::color(1)));
    assert_eq!(epoch(&app), before, "a redone colour changes no finding");
}

#[test]
fn a_font_family_change_bumps_the_erc_epoch() {
    let (mut app, _path) = colours::app_with_sheet("tail-font.snxsch");
    let before = epoch(&app);
    assert!(app.apply_engine_command(
        Command::SetDocumentFontFamily {
            font_family: Some("Inter".to_string()),
        },
        true,
        true,
    ));
    assert_eq!(epoch(&app), before + 1, "a font change is not display-only");
}

/// The page the canvas draws (width, height), as the PAPER refresh sets it.
fn drawn_page(app: &Signex) -> (f32, f32) {
    let canvas = app.interaction_state.active_canvas();
    (canvas.paper_width_mm, canvas.paper_height_mm)
}

#[test]
fn a_paper_change_and_its_undo_reach_the_panel_context_and_the_drawn_page() {
    let (mut app, _path) = colours::app_with_sheet("tail-paper.snxsch");
    assert_eq!(app.document_state.panel_ctx.paper_size, "A4");
    let a4 = drawn_page(&app);
    assert!(app.apply_engine_command(
        Command::SetPaperSize {
            paper_size: "A3".to_string(),
        },
        true,
        true,
    ));
    assert_eq!(app.document_state.panel_ctx.paper_size, "A3");
    assert_eq!(drawn_page(&app), crate::panels::paper_dimensions("A3"));
    assert_ne!(drawn_page(&app), a4);
    assert!(app.apply_engine_undo(true));
    assert_eq!(app.document_state.panel_ctx.paper_size, "A4");
    assert_eq!(drawn_page(&app), a4);
}

/// Every kind of edit leaves the sheet dirty, so closing it asks to save:
/// a colour (display-only), a move, a font and a paper change.
#[test]
fn an_edit_of_any_kind_leaves_the_sheet_dirty() {
    let edits: [fn(&mut Signex, &std::path::Path); 4] = [
        |app, path| {
            let wire = colours::wire_of(app, path);
            colours::paint(app, &[wire], colours::color(2));
        },
        |app, path| {
            let symbol = colours::symbol_of(app, path);
            assert!(app.apply_engine_command(
                Command::MoveSelection {
                    items: vec![signex_types::schematic::SelectedItem::new(
                        symbol,
                        signex_types::schematic::SelectedKind::Symbol,
                    )],
                    dx: 10.0,
                    dy: 0.0,
                    keep_wires: false,
                    wiring: signex_types::corner::DragWiringMode::Auto,
                },
                true,
                true,
            ));
        },
        |app, _| {
            assert!(app.apply_engine_command(
                Command::SetDocumentFontFamily {
                    font_family: Some("Inter".to_string()),
                },
                true,
                true,
            ));
        },
        |app, _| {
            assert!(app.apply_engine_command(
                Command::SetPaperSize {
                    paper_size: "A3".to_string(),
                },
                true,
                true,
            ));
        },
    ];
    for (index, edit) in edits.iter().enumerate() {
        let (mut app, path) = colours::app_with_sheet(&format!("tail-dirty-{index}.snxsch"));
        app.document_state
            .engines
            .get_mut(&path)
            .unwrap()
            .mark_saved();
        app.document_state.mark_document_clean(&path);
        edit(&mut app, &path);
        assert!(
            app.document_state.is_document_dirty(&path),
            "edit {index} must leave the sheet dirty"
        );
    }
}

#[test]
fn a_colour_set_by_path_on_a_parked_sheet_leaves_it_dirty() {
    let mut app = app_with(&[("a.snxsch", 100), ("b.snxsch", 101)]);
    let a = path("a.snxsch");
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(3));
    let before = epoch(&app);
    assert_eq!(
        app.execute_on_sheet(&a, Command::SetNetColors { colors }),
        Ok(true)
    );
    assert!(app.document_state.is_document_dirty(&a));
    assert_eq!(epoch(&app), before, "a colour changes no finding");
}

fn run(app: &mut Signex, name: &str, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new(name).expect("a valid id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands parse");
    app.run_operands_settled(&command, &args).0
}

fn apply_edits(app: &mut Signex, file: &str, edits: serde_json::Value) -> serde_json::Value {
    let revision = app.document_state.engines[&path(file)].revision();
    match run(
        app,
        "apply_edits",
        &[
            format!("document={}", path(file).display()),
            format!("expect_revision={revision}"),
            format!("edits={edits}"),
        ],
    ) {
        CommandOutcome::Answered(value) => value,
        other => panic!("apply_edits was not answered: {other:?}"),
    }
}

/// Make the active canvas's cached `WIRE_A` disagree with the engine while
/// the cache stays level with its revision, so only a full re-clone fixes it.
pub(in crate::app) fn stale_wire_in_cache(app: &mut Signex, file: &str) -> Point {
    let engine = &app.document_state.engines[&path(file)];
    let mut sheet = engine.document().clone();
    let stale = Point::new(-77.0, -77.0);
    sheet
        .wires
        .iter_mut()
        .find(|wire| wire.uuid == uid(WIRE_A))
        .expect("fixture wire")
        .end = stale;
    let cache = SchematicRenderCache::from_sheet_at(&sheet, engine.revision());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(cache));
    stale
}

pub(in crate::app) fn cached_wire_end(app: &Signex) -> Point {
    let cache = app.interaction_state.canvas.render_cache.as_ref().unwrap();
    cache
        .snapshot()
        .wires
        .iter()
        .find(|wire| wire.uuid == uid(WIRE_A))
        .expect("cached wire")
        .end
}

#[test]
fn an_agents_edit_list_repaints_only_what_it_changed() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let stale = stale_wire_in_cache(&mut app, "a.snxsch");
    let answer = apply_edits(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_junction", "args": {"at": [50.8, 50.8]}}]),
    );
    assert_eq!(answer["changed"], true);
    let cache = app.interaction_state.canvas.render_cache.as_ref().unwrap();
    assert_eq!(
        cache.snapshot().junctions.len(),
        app.document_state.engines[&path("a.snxsch")]
            .document()
            .junctions
            .len(),
        "the new junction is shown"
    );
    assert_eq!(cached_wire_end(&app), stale, "the wires were not re-cloned");
}

/// One `place_junction` naming its document, as a headless run makes it
/// (`run_on_document`); the agent socket's one-step run is pinned in
/// `agent/tests/owed_repaint.rs`.
#[test]
fn a_single_placement_naming_its_document_repaints_only_what_it_changed() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let stale = stale_wire_in_cache(&mut app, "a.snxsch");
    let before = app.document_state.engines[&path("a.snxsch")]
        .document()
        .junctions
        .len();
    assert_eq!(
        run(
            &mut app,
            "place_junction",
            &[
                format!("document={}", path("a.snxsch").display()),
                "at=50.8,50.8".to_string(),
            ],
        ),
        CommandOutcome::Ran
    );
    let cache = app.interaction_state.canvas.render_cache.as_ref().unwrap();
    assert_eq!(cache.snapshot().junctions.len(), before + 1);
    assert_eq!(cached_wire_end(&app), stale, "the wires were not re-cloned");
}

fn drift_pending(app: &Signex) -> bool {
    app.library.drift.check_pending
}

#[test]
fn an_engine_undo_sets_the_library_drift_check() {
    let (mut app, path) = colours::app_with_sheet("tail-drift-undo.snxsch");
    let wire = colours::wire_of(&app, &path);
    colours::paint(&mut app, &[wire], colours::color(1));
    app.library.drift.check_pending = false;
    assert!(app.apply_engine_undo(true));
    assert!(drift_pending(&app));
}

#[test]
fn a_step_of_a_sheet_by_path_sets_the_library_drift_check() {
    let mut app = app_with(&[("a.snxsch", 100), ("b.snxsch", 101)]);
    let a = path("a.snxsch");
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(3));
    assert_eq!(
        app.execute_on_sheet(&a, Command::SetNetColors { colors }),
        Ok(true)
    );
    app.library.drift.check_pending = false;
    assert_eq!(app.step_sheet_on(&a, true), Ok(true));
    assert!(drift_pending(&app));
}

#[test]
fn an_agents_revert_sets_the_library_drift_check() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let group = apply_edits(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_junction", "args": {"at": [50.8, 50.8]}}]),
    )["group"]
        .as_u64()
        .unwrap();
    app.library.drift.check_pending = false;
    let outcome = run(
        &mut app,
        "revert",
        &[
            format!("document={}", path("a.snxsch").display()),
            format!("group={group}"),
        ],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert!(drift_pending(&app));
}

/// Pins today's behaviour, not a wish: a grouped step (here an agent's step,
/// undone by the person) does not set the drift check. Tracked in #2476; a
/// fix flips this assertion rather than deleting the test.
#[test]
fn a_grouped_undo_does_not_set_the_library_drift_check_today() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    apply_edits(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_junction", "args": {"at": [50.8, 50.8]}}]),
    );
    app.library.drift.check_pending = false;
    let before = app.document_state.engines[&path("a.snxsch")].undo_position();
    app.handle_undo_requested();
    assert_eq!(
        app.document_state.engines[&path("a.snxsch")].undo_position(),
        before - 1,
        "the agent's step was undone"
    );
    assert!(!drift_pending(&app));
}
