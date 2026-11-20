//! mockup 179d / mockup 190 - board sketch outline checks and the bake preview: the
//! checks are live in Properties after every edit (a Properties edit too),
//! a failing one names its place and does not stop Bake, and the open bake
//! preview hands the canvas what Bake writes. Driven through
//! `Signex::update`.

use signex_app::app::{BoardSketchMsg, Message, PcbMsg};
use signex_app::library::editor::footprint::state::{CheckLine, CheckStatus};
use signex_app::library::messages::RoleTag;
use signex_app::panels::PanelMsg;
use signex_i18n::RenderTranslatable;
use signex_sketch::id::SketchEntityId;

use crate::board_sketch::{Fixture, in_sketch, live, open_board, panel, rectangle, send};
use crate::menu_support::menu_row;

fn sketch(f: &mut Fixture, event: BoardSketchMsg) {
    send(f, Message::Pcb(PcbMsg::Sketch(event)));
}

/// The Outline checks lines Properties shows for the board in view.
fn check_lines(f: &Fixture) -> Vec<CheckLine> {
    f.app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .and_then(|fp| fp.board.as_ref())
        .expect("board sketch Properties")
        .outline_checks
        .clone()
}

/// A line in en-US, without Fluent's isolation marks.
fn english(line: &CheckLine) -> String {
    line.text
        .render_en_us()
        .replace(['\u{2068}', '\u{2069}'], "")
}

fn set_role(f: &mut Fixture, edges: &[SketchEntityId], role: RoleTag) {
    for &id in edges {
        panel(f, PanelMsg::FpEditorSetRole { id, role });
    }
}

/// An outline drawn with the Rectangle tool and tagged from the role
/// picker: Properties reads its checks at once.
fn board_with_outline(f: &mut Fixture) -> Vec<SketchEntityId> {
    send(f, menu_row("board_sketch"));
    assert!(in_sketch(f));
    let edges = rectangle(f, (0.0, 0.0), (40.0, 30.0));
    set_role(f, &edges, RoleTag::BoardOutline);
    edges
}

#[test]
fn the_checks_follow_the_role_picker_and_read_ok_for_a_rectangle() {
    let mut f = open_board();
    send(&mut f, menu_row("board_sketch"));
    let before = check_lines(&f);
    assert!(
        before.iter().all(|l| l.status != CheckStatus::Passed),
        "nothing passes before there is an outline: {before:#?}"
    );

    let edges = rectangle(&mut f, (0.0, 0.0), (40.0, 30.0));
    set_role(&mut f, &edges, RoleTag::BoardOutline);

    let lines = check_lines(&f);
    let texts: Vec<String> = lines.iter().map(english).collect();
    assert_eq!(texts[0], "Closed: one loop", "{texts:#?}");
    assert_eq!(texts[1], "No self-intersection", "{texts:#?}");
    assert_eq!(
        texts[2], "Holes, cutouts and keepouts inside the outline",
        "{texts:#?}"
    );
    assert_eq!(lines[3].status, CheckStatus::NotChecked, "the router check");
}

#[test]
fn a_cutout_outside_fails_with_its_place_and_bake_still_applies() {
    let mut f = open_board();
    board_with_outline(&mut f);
    // Apart from the outline, so no snap joins the two profiles.
    let stray = rectangle(&mut f, (60.0, 0.0), (70.0, 10.0))[0];
    set_role(&mut f, &[stray], RoleTag::Cutout);

    let failed: Vec<CheckLine> = check_lines(&f)
        .into_iter()
        .filter(|l| l.status == CheckStatus::Failed)
        .collect();
    assert_eq!(failed.len(), 1, "{failed:#?}");
    assert_eq!(english(&failed[0]), "Cutout outside the outline");
    assert_eq!(failed[0].place, Some(stray), "the line names the cutout");

    // A click on the line selects its place.
    panel(&mut f, PanelMsg::FpEditorSelectSketchEntity { id: stray });
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert_eq!(session.editor.state.selected_sketch, Some(stray));

    // A failing check warns only (pending the router decision).
    sketch(&mut f, BoardSketchMsg::BakeRequested);
    assert!(!in_sketch(&f), "Bake applies and leaves");
    assert_eq!(live(&f).outline.len(), 4, "the outline reached the board");
}

#[test]
fn the_open_preview_hands_the_canvas_what_bake_writes() {
    let mut f = open_board();
    board_with_outline(&mut f);

    sketch(&mut f, BoardSketchMsg::BakePreviewToggled);
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    let baked = session
        .editor
        .state
        .sketch_exit
        .board_result
        .as_deref()
        .expect("the canvas draws the bake while the preview is open");
    assert_eq!(baked.outline.as_ref().map(Vec::len), Some(4));
    assert!(session.preview_error.is_none(), "the sketch bakes");

    sketch(&mut f, BoardSketchMsg::BakePreviewToggled);
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(
        session.editor.state.sketch_exit.board_result.is_none(),
        "Back to sketch drops the drawn result"
    );
}
