//! Mockup row 210, "Re-sync from sheets: review the changes": the tab's
//! Re-sync applies added and moved blocks at once and opens the review only
//! for a block that would become unlinked; Apply selected is one undo step;
//! an unchecked row comes back next time. The headless `unlink` operand.
//! The GUI path is `Signex::update` with the tab's messages and the layout
//! probe of the side sheet; the previews are read from the review, their
//! tags and the hover outline also from the drawn canvas.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_types::block_diagram::{LinkState, read_block_diagram};

use super::tests::{app_on, write_hierarchy};
use crate::app::block_diagram::{BlockDiagramMsg, Preview, ReviewEvent, TabEvent, previews};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::core::block_diagram_panes::ResyncReview;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::layout_probe::{draw_pixels, lay_out_texts, messages_for_moves, messages_on_click};
use crate::styles::ti;
use crate::styles::tokens::resolve;
use crate::task_driver::TaskDriver;

const WINDOW: Size = Size::new(1600.0, 900.0);

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    TaskDriver::new().settle(app, task).expect("settles");
    outcome
}

/// Alpha and Beta, generated to `ProjectA.snxblk`.
fn generated(dir: &Path) -> (Signex, PathBuf) {
    write_hierarchy(dir, &[("Alpha", "a.snxsch"), ("Beta", "b.snxsch")]);
    let mut app = app_on(dir);
    let project = format!("document={}", dir.join("ProjectA.snxprj").display());
    let outcome = run(&mut app, "generate_block_diagram", &[&project]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    (app, dir.join("ProjectA.snxblk"))
}

fn send(app: &mut Signex, path: &Path, event: TabEvent) {
    let task = app.update(Message::from(BlockDiagramMsg::Tab {
        path: path.to_path_buf(),
        event,
    }));
    TaskDriver::new().settle(app, task).expect("settles");
}

fn open(app: &mut Signex, path: &Path) {
    let task = app.open_block_diagram_file(path.to_path_buf());
    TaskDriver::new().settle(app, task).expect("settles");
}

fn texts(app: &Signex, path: &Path) -> Vec<String> {
    lay_out_texts(app.view_block_diagram_tab(path.to_path_buf()), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn block_state(app: &Signex, path: &Path, name: &str) -> LinkState {
    let diagram = &app.document_state.block_diagrams.editors[path].diagram;
    let block = diagram.blocks.iter().find(|b| b.name == name).expect(name);
    block.sheet.as_ref().expect("linked").state
}

/// Beta's sheet leaves the hierarchy and Gamma's comes in.
fn change_hierarchy(dir: &Path) {
    write_hierarchy(dir, &[("Alpha", "a.snxsch"), ("Gamma", "c.snxsch")]);
}

#[test]
fn resync_applies_the_new_block_and_reviews_the_unlink() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    change_hierarchy(dir.path());
    send(&mut app, &file, TabEvent::ResyncPressed);

    let editor = app.open_diagram(&file).expect("open");
    assert_eq!(editor.doc.undo.len(), 1, "Gamma came in as one step");
    assert!(editor.doc.diagram.blocks.iter().any(|b| b.name == "Gamma"));
    assert_eq!(
        block_state(&app, &file, "Beta"),
        LinkState::Linked,
        "not yet"
    );
    let review = editor.tab.review.as_ref().expect("the review opened");
    assert_eq!(review.unlink.len(), 1);
    assert_eq!(review.added.len(), 1);
    let shown = previews(review);
    assert!(shown.iter().any(|(_, p)| *p == Preview::Added), "{shown:?}");
    assert!(
        shown.iter().any(|(_, p)| *p == Preview::Unlinked),
        "{shown:?}"
    );

    let shown = texts(&app, &file);
    for expected in [
        "Re-sync from sheets",
        "Unlinked 1",
        "Beta",
        "1 of 1 selected · one undo step",
    ] {
        assert!(shown.iter().any(|t| t == expected), "{expected}: {shown:?}");
    }
    let beta = review.unlink[0].block;
    send(
        &mut app,
        &file,
        TabEvent::Review(ReviewEvent::RowToggled(beta, false)),
    );
    let review = app.diagram_view(&file).review.as_ref().expect("open");
    let shown = previews(review);
    assert!(
        !shown.iter().any(|(_, p)| *p == Preview::Unlinked),
        "unticked: no hatch"
    );
    assert!(
        texts(&app, &file)
            .iter()
            .any(|t| t == "0 of 1 selected · one undo step")
    );
    send(
        &mut app,
        &file,
        TabEvent::Review(ReviewEvent::RowToggled(beta, true)),
    );

    let clicked: Vec<TabEvent> = messages_on_click(
        app.view_block_diagram_tab(file.clone()),
        WINDOW,
        "Apply selected",
    )
    .into_iter()
    .filter_map(|m| match m {
        Message::BlockDiagram(BlockDiagramMsg::Tab { event, .. }) => Some(event),
        _ => None,
    })
    .collect();
    assert_eq!(clicked, [TabEvent::Review(ReviewEvent::ApplyPressed)]);
    send(&mut app, &file, TabEvent::Review(ReviewEvent::ApplyPressed));
    let editor = app.open_diagram(&file).expect("open");
    assert!(editor.tab.review.is_none());
    assert_eq!(editor.doc.undo.len(), 2, "Apply selected is one undo step");
    assert_eq!(block_state(&app, &file, "Beta"), LinkState::Unlinked);
    send(&mut app, &file, TabEvent::UndoPressed);
    assert_eq!(block_state(&app, &file, "Beta"), LinkState::Linked);
}

#[test]
fn an_unchecked_row_is_listed_again_next_time_and_nothing_to_change_closes() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    change_hierarchy(dir.path());
    send(&mut app, &file, TabEvent::ResyncPressed);
    let beta = app
        .diagram_view(&file)
        .review
        .as_ref()
        .expect("open")
        .unlink[0]
        .block;
    send(
        &mut app,
        &file,
        TabEvent::Review(ReviewEvent::RowToggled(beta, false)),
    );
    send(&mut app, &file, TabEvent::Review(ReviewEvent::ApplyPressed));
    assert_eq!(block_state(&app, &file, "Beta"), LinkState::Linked);

    send(&mut app, &file, TabEvent::ResyncPressed);
    let review = app
        .diagram_view(&file)
        .review
        .clone()
        .expect("listed again");
    assert_eq!(review.unlink.len(), 1);
    assert!(review.unlink[0].checked, "checked again");

    // Beta's sheet comes back while the review is open.
    write_hierarchy(
        dir.path(),
        &[
            ("Alpha", "a.snxsch"),
            ("Beta", "b.snxsch"),
            ("Gamma", "c.snxsch"),
        ],
    );
    send(&mut app, &file, TabEvent::ResyncPressed);
    assert!(texts(&app, &file).iter().any(|t| t == "Nothing to change"));
    send(&mut app, &file, TabEvent::Review(ReviewEvent::Closed));
    assert!(app.diagram_view(&file).review.is_none());

    // With nothing to unlink no review opens.
    send(&mut app, &file, TabEvent::ResyncPressed);
    assert!(app.diagram_view(&file).review.is_none());
}

#[test]
fn headless_regenerate_skips_or_applies_the_unlink() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    change_hierarchy(dir.path());
    let project = format!("document={}", dir.path().join("ProjectA.snxprj").display());
    let skip = run(
        &mut app,
        "regenerate_block_diagram",
        &[&project, "apply=true", "unlink=skip"],
    );
    let CommandOutcome::Answered(answer) = skip else {
        panic!("{skip:?}");
    };
    assert_eq!(
        answer["skipped"].as_array().map(Vec::len),
        Some(1),
        "{answer}"
    );
    assert_eq!(answer["skipped"][0]["change"], "sheet_removed", "{answer}");
    assert!(
        !answer["changes"]
            .as_array()
            .expect("changes")
            .iter()
            .any(|c| c["change"] == "sheet_removed"),
        "{answer}"
    );
    let read = read_block_diagram(&file).expect("written").diagram;
    let beta = read.blocks.iter().find(|b| b.name == "Beta").expect("Beta");
    assert_eq!(beta.sheet.as_ref().unwrap().state, LinkState::Linked);
    assert!(
        read.blocks.iter().any(|b| b.name == "Gamma"),
        "the rest applied"
    );

    let apply = run(
        &mut app,
        "regenerate_block_diagram",
        &[&project, "apply=true", "unlink=apply"],
    );
    let CommandOutcome::Answered(answer) = apply else {
        panic!("{apply:?}");
    };
    assert_eq!(answer["skipped"], serde_json::json!([]), "{answer}");
    let read = read_block_diagram(&file).expect("written").diagram;
    let beta = read.blocks.iter().find(|b| b.name == "Beta").expect("Beta");
    assert_eq!(beta.sheet.as_ref().unwrap().state, LinkState::Unlinked);

    let bad = run(
        &mut app,
        "regenerate_block_diagram",
        &[&project, "unlink=maybe"],
    );
    assert!(matches!(bad, CommandOutcome::Disabled { .. }), "{bad:?}");
}

#[test]
fn previewed_blocks_carry_their_tags_in_their_colours() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    change_hierarchy(dir.path());
    send(&mut app, &file, TabEvent::ResyncPressed);

    let tokens = app.document_view.panel_ctx.tokens;
    let editor = app.open_diagram(&file).expect("open");
    let id_of = |name: &str| {
        editor
            .doc
            .diagram
            .blocks
            .iter()
            .find(|b| b.name == name)
            .expect(name)
            .id
    };
    let (gamma, beta) = (id_of("Gamma"), id_of("Beta"));
    let tags = app.block_diagram_review_tags(&file);
    let tag = |block| tags.iter().find(|(id, ..)| *id == block).cloned();
    let (_, word, ink) = tag(gamma).expect("Gamma is tagged");
    assert_eq!((word.as_str(), ink), ("new", ti(tokens.success)));
    let (_, word, ink) = tag(beta).expect("Beta is tagged");
    assert_eq!((word.as_str(), ink), ("unlinked", ti(tokens.warning)));
    assert_eq!(tags.len(), 2, "only the previewed blocks: {tags:?}");

    // The canvas draws the words: blank words draw a different diagram.
    let canvas = Size::new(1000.0, 700.0);
    let worded = draw_pixels(app.block_diagram_canvas_in_test(&file, true), canvas);
    let blank = draw_pixels(app.block_diagram_canvas_in_test(&file, false), canvas);
    assert_ne!(worded, blank, "the tags are drawn on the canvas");

    // Unticking Beta's row takes its tag away with its hatch.
    send(
        &mut app,
        &file,
        TabEvent::Review(ReviewEvent::RowToggled(beta, false)),
    );
    let tags = app.block_diagram_review_tags(&file);
    assert!(tags.iter().all(|(id, ..)| *id != beta), "{tags:?}");

    // A renamed sheet file reads "renamed", a moved one "moved", both in
    // the moved colour.
    let review = ResyncReview {
        moved: vec![gamma, beta],
        renamed: vec![gamma],
        ..ResyncReview::default()
    };
    let shown = previews(&review);
    assert_eq!(shown, [(gamma, Preview::Renamed), (beta, Preview::Moved)]);
    app.diagram_view_mut(&file).review = Some(review);
    let tags = app.block_diagram_review_tags(&file);
    let moved = resolve(&tokens).info;
    assert_eq!(tags[0].1, "renamed");
    assert_eq!(tags[1].1, "moved");
    assert!(tags.iter().all(|(_, _, ink)| *ink == moved), "{tags:?}");
}

#[test]
fn hovering_a_row_outlines_its_block() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    change_hierarchy(dir.path());
    send(&mut app, &file, TabEvent::ResyncPressed);
    let beta = app
        .diagram_view(&file)
        .review
        .as_ref()
        .expect("open")
        .unlink[0]
        .block;

    let laid = lay_out_texts(app.view_block_diagram_tab(file.clone()), WINDOW);
    let row = laid
        .iter()
        .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Beta")
        .expect("Beta's row")
        .bounds;
    let over = row.center();
    let away = iced::Point::new(over.x, WINDOW.height - 2.0);
    let events: Vec<TabEvent> = messages_for_moves(
        app.view_block_diagram_tab(file.clone()),
        WINDOW,
        &[over, away],
    )
    .into_iter()
    .filter_map(|m| match m {
        Message::BlockDiagram(BlockDiagramMsg::Tab { event, .. }) => Some(event),
        _ => None,
    })
    .collect();
    assert_eq!(
        events,
        [
            TabEvent::Review(ReviewEvent::RowHovered(Some(beta))),
            TabEvent::Review(ReviewEvent::RowHovered(None)),
        ]
    );

    let hovered = |app: &Signex| {
        app.diagram_view(&file)
            .review
            .as_ref()
            .expect("open")
            .hovered
    };
    send(&mut app, &file, events[0].clone());
    assert_eq!(hovered(&app), Some(beta), "Beta is outlined");
    let canvas = Size::new(1000.0, 700.0);
    let outlined = draw_pixels(app.block_diagram_canvas_in_test(&file, true), canvas);
    send(&mut app, &file, events[1].clone());
    assert_eq!(hovered(&app), None, "the outline goes with the pointer");
    let plain = draw_pixels(app.block_diagram_canvas_in_test(&file, true), canvas);
    assert_ne!(outlined, plain, "the hover outline is drawn");
}

/// A sheet with no document id in its file refuses the tab's Re-sync: no
/// undo step, the diagram and its file as they were, and the review stays
/// closed.
#[test]
fn resync_refuses_a_sheet_without_an_id_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let (mut app, file) = generated(dir.path());
    open(&mut app, &file);
    let bytes = std::fs::read(&file).expect("bytes");
    let before = app.document_state.block_diagrams.editors[&file]
        .diagram
        .clone();
    let leaf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/schematic/hierarchy-child-leaf.snxsch");
    std::fs::copy(leaf, dir.path().join("b.snxsch")).expect("Beta without an id");
    send(&mut app, &file, TabEvent::ResyncPressed);

    let editor = app.open_diagram(&file).expect("open");
    assert!(editor.doc.undo.is_empty(), "no undo step");
    assert_eq!(editor.doc.diagram, before, "the diagram as it was");
    assert!(editor.tab.review.is_none());
    assert_eq!(
        std::fs::read(&file).expect("bytes"),
        bytes,
        "nothing written"
    );
}
