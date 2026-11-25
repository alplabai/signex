//! #2578 pour corners and #2579 hatched fill through `Signex::update` (the
//! area panel's Corners and Fill Mode rows, the placement form) and through
//! the command path (`set_pour_corners`, `set_pour_fill`): each edit is one
//! undo step and the fill the panel reports follows it.

use signex_types::zone_pour_shape::{PourCorners, PourFillMode};

use super::form_tests::{form, panel};
use super::pour_cut_tests::draw_pour;
use super::tests::{app_with_board, board_of, click, escape, run, undo};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget, ZoneFacts,
};

fn area_panel(app: &Signex) -> ZoneFacts {
    match form(app) {
        PcbPrimitiveFacts::Zone(f) => f,
        other => panic!("the area panel, not {other:?}"),
    }
}

/// Draws the pour and picks it with a click inside it, off the track.
fn picked_pour(app: &mut Signex) -> PrimitiveTarget {
    draw_pour(app);
    click(app, 5.0, 3.0);
    area_panel(app).common.target
}

fn copper_area(app: &Signex) -> f64 {
    area_panel(app).copper.expect("the fill is computed").0
}

fn typed_and_entered(
    app: &mut Signex,
    target: &PrimitiveTarget,
    field: PrimitiveField,
    text: &str,
) {
    panel(
        app,
        PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.into(),
        },
    );
    panel(
        app,
        PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        },
    );
}

/// GUI path, #2578: Chamfer, then a 1 mm Corner Size, one undo step each;
/// the copper shrinks by the corners cut off; undo puts the pour back.
#[test]
fn the_area_panel_chamfers_a_pours_corners_one_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let target = picked_pour(&mut app);
    let sharp = copper_area(&app);
    assert_eq!(area_panel(&app).corners, Some(PourCorners::Sharp));

    panel(
        &mut app,
        PcbPrimitiveMsg::PourCornersPicked {
            target: target.clone(),
            corners: PourCorners::Chamfer,
        },
    );
    assert_eq!(board_of(&app, &path).zones[0].corners, PourCorners::Chamfer);
    assert_eq!(copper_area(&app), sharp, "no size yet, no cut");
    typed_and_entered(&mut app, &target, PrimitiveField::CornerSize, "1");
    assert_eq!(board_of(&app, &path).zones[0].corner_size, 1.0);
    let chamfered = copper_area(&app);
    // Four corners lose a 0.5 mm² triangle each.
    assert!(
        (sharp - chamfered - 2.0).abs() < 1e-6,
        "{sharp} -> {chamfered}"
    );
    assert_eq!(area_panel(&app).corner_size, Some(1.0));

    typed_and_entered(&mut app, &target, PrimitiveField::CornerSize, "-1");
    assert_eq!(
        board_of(&app, &path).zones[0].corner_size,
        1.0,
        "a negative size is refused"
    );

    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones[0].corner_size, 0.0);
    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones[0].corners, PourCorners::Sharp);
    assert_eq!(copper_area(&app), sharp);
}

/// GUI path, #2579: Hatched, then a typed gap, one undo step each; the fill
/// holds less copper than solid and the panel shows the hatch.
#[test]
fn the_area_panel_hatches_a_pour_one_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let target = picked_pour(&mut app);
    let solid = copper_area(&app);

    panel(
        &mut app,
        PcbPrimitiveMsg::PourFillPicked {
            target: target.clone(),
            fill: PourFillMode::Hatched,
        },
    );
    assert_eq!(
        board_of(&app, &path).zones[0].fill_mode,
        PourFillMode::Hatched
    );
    let f = area_panel(&app);
    assert_eq!(f.fill_mode, Some(PourFillMode::Hatched));
    assert_eq!(
        (f.hatch_width, f.hatch_gap),
        (Some(0.25), Some(0.5)),
        "the defaults"
    );
    let hatched = copper_area(&app);
    assert!(hatched < solid, "{hatched} < {solid}");

    typed_and_entered(&mut app, &target, PrimitiveField::HatchGap, "1");
    assert_eq!(board_of(&app, &path).zones[0].hatch_gap, 1.0);
    assert!(copper_area(&app) < hatched, "a wider gap holds less copper");

    undo(&mut app);
    assert_eq!(board_of(&app, &path).zones[0].hatch_gap, 0.0);
    undo(&mut app);
    assert_eq!(
        board_of(&app, &path).zones[0].fill_mode,
        PourFillMode::Solid
    );
    assert_eq!(copper_area(&app), solid);
}

/// The placement form: Corners and Fill Mode picked while the Polygon Pour
/// tool is armed are the values the next pour is placed with.
#[test]
fn the_placement_form_places_a_rounded_hatched_pour() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_polygon_pour");
    let target = PrimitiveTarget::placement(path.clone());
    panel(
        &mut app,
        PcbPrimitiveMsg::PourCornersPicked {
            target: target.clone(),
            corners: PourCorners::Rounded,
        },
    );
    panel(
        &mut app,
        PcbPrimitiveMsg::PourFillPicked {
            target: target.clone(),
            fill: PourFillMode::Hatched,
        },
    );
    escape(&mut app);
    draw_pour(&mut app);
    let zone = &board_of(&app, &path).zones[0];
    assert_eq!(
        (zone.corners, zone.fill_mode),
        (PourCorners::Rounded, PourFillMode::Hatched)
    );
}

fn command(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("an id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

/// Command path: `set_pour_corners` and `set_pour_fill` on the named board,
/// one undo step each, answer with the pour's settings and its fill; a bad
/// token, a negative size or an id that is no pour is refused and nothing
/// moves.
#[test]
fn set_pour_corners_and_set_pour_fill_answer_with_the_pours() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.ui_state.first_run_tour_open = false;
    draw_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].uuid;
    let document = format!("document={}", path.display());
    let ids = format!("ids={pour}");

    let CommandOutcome::Answered(answer) = command(
        &mut app,
        "set_pour_corners",
        &[&document, &ids, "corners=rounded", "corner_size=0.5"],
    ) else {
        panic!("set_pour_corners was not answered");
    };
    let held = &answer["pours"][0];
    assert_eq!(held["zone"], pour.to_string());
    assert_eq!(
        (held["corners"].as_str(), held["corner_size"].as_f64()),
        (Some("rounded"), Some(0.5))
    );
    assert_eq!(held["filled"], true);

    let CommandOutcome::Answered(answer) = command(
        &mut app,
        "set_pour_fill",
        &[&document, &ids, "fill=hatched", "width=0.3", "gap=0.9"],
    ) else {
        panic!("set_pour_fill was not answered");
    };
    let held = &answer["pours"][0];
    assert_eq!(held["fill"], "hatched");
    assert_eq!(
        (held["hatch_width"].as_f64(), held["hatch_gap"].as_f64()),
        (Some(0.3), Some(0.9))
    );
    let zone = board_of(&app, &path).zones[0].clone();
    assert_eq!((zone.hatch_width, zone.hatch_gap), (0.3, 0.9));

    let before = serde_json::to_string(board_of(&app, &path)).unwrap();
    for refused in [
        vec![document.as_str(), ids.as_str(), "corners=round"],
        vec![
            document.as_str(),
            ids.as_str(),
            "corners=chamfer",
            "corner_size=-1",
        ],
    ] {
        let outcome = command(&mut app, "set_pour_corners", &refused);
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{refused:?}: {outcome:?}"
        );
    }
    let outcome = command(&mut app, "set_pour_fill", &[&document, &ids, "fill=none"]);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    let not_a_pour = format!("ids={}", uuid::Uuid::from_u128(0xdead));
    let outcome = command(
        &mut app,
        "set_pour_fill",
        &[&document, &not_a_pour, "fill=solid"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        serde_json::to_string(board_of(&app, &path)).unwrap(),
        before,
        "nothing moved"
    );

    undo(&mut app);
    assert_eq!(
        board_of(&app, &path).zones[0].fill_mode,
        PourFillMode::Solid,
        "one step"
    );
    undo(&mut app);
    assert_eq!(
        board_of(&app, &path).zones[0].corners,
        PourCorners::Sharp,
        "one step"
    );
}
