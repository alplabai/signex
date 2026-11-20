//! Decisions 3 and 4 (mockup row 179d, mockup row 190, Caner 2026-11-20): the router tool of the
//! project's fab tier reaches the board sketch, whose inside-corner check
//! warns at a corner tighter than the tool and never stops Bake; an open
//! outline whose gap is within the 0.05 mm heal tolerance offers Close gap,
//! which joins the ends as one undo step so the outline bakes; a wider gap
//! offers no button and says why; a footprint with an open profile offers
//! Close gap too. Driven through `Signex::update`.

use signex_app::app::{
    BoardSketchMsg, EditMsg, FootprintDocument, LoadedProject, Message, PcbMsg, TabInfo, TabKind,
};
use signex_app::library::editor::footprint::{CheckLine, CheckStatus};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_app::panels::{BoardSketchAttrMsg, BoardSketchProps, PanelMsg, SketchFixMsg};
use signex_i18n::{RenderTranslatable, Translatable, ids};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::attr::CourtyardAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::project::{FabTemplate, ProjectData};

use crate::board_sketch::{Fixture, in_sketch, live, open_board, panel, send};
use crate::menu_support::menu_row;

/// Open beside the board the project `P` that names it, made with the
/// Standard fab tier: it stores the tier's router tool.
fn board_of_standard_project() -> Fixture {
    let mut f = open_board();
    let dir = f.path.parent().unwrap().to_path_buf();
    let mut data: ProjectData = serde_json::from_value(serde_json::json!({
        "name": "P",
        "dir": dir.to_string_lossy(),
        "schematic_root": null,
        "pcb_file": "board.snxpcb",
    }))
    .unwrap();
    data.set_router_tool_diameter(Some(
        FabTemplate::Standard.capabilities().router_tool_diameter,
    ));
    let id = f.app.document_state.mint_project_id();
    f.app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    f.app.document_state.active_project = Some(id);
    f
}

/// A sketch on the board-top plane: a polyline through `points` (closed
/// back to the first point when `closed`), its edges tagged as outline
/// except the first, which the caller tags through Properties so the
/// session solves it. Returns the sketch, the points and the edges.
fn polyline(
    points: &[(f64, f64)],
    closed: bool,
) -> (SketchData, Vec<SketchEntityId>, Vec<SketchEntityId>) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let mut add = |kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let p: Vec<SketchEntityId> = points
        .iter()
        .map(|&(x, y)| add(EntityKind::Point { x, y }))
        .collect();
    let n = p.len();
    let count = if closed { n } else { n - 1 };
    let edges: Vec<SketchEntityId> = (0..count)
        .map(|i| {
            add(EntityKind::Line {
                start: p[i],
                end: p[(i + 1) % n],
            })
        })
        .collect();
    for entity in &mut sketch.entities {
        if edges[1..].contains(&entity.id) {
            entity.board_outline = true;
        }
    }
    (sketch, p, edges)
}

/// Enter sketch mode on the board and put `sketch` in the working copy;
/// tagging `first` as outline from Properties solves it.
fn sketch_in(f: &mut Fixture, sketch: SketchData, first: SketchEntityId) {
    send(f, menu_row("board_sketch"));
    assert!(in_sketch(f));
    let session = f.app.interaction_state.board_sketch.as_mut().unwrap();
    session.editor.primitive_mut().sketch = Some(sketch);
    panel(
        f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled {
            id: first,
            on: true,
        }),
    );
}

fn props(f: &Fixture) -> BoardSketchProps {
    f.app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .and_then(|fp| fp.board.clone())
        .expect("board sketch Properties")
}

fn english(t: &Translatable) -> String {
    t.render_en_us().replace(['\u{2068}', '\u{2069}'], "")
}

fn texts(lines: &[CheckLine]) -> Vec<String> {
    lines.iter().map(|l| english(&l.text)).collect()
}

/// The rectangle outline (0, 0) - (40, 30) whose last edge stops `gap`
/// above the first corner.
fn open_rectangle(gap: f64) -> (SketchData, Vec<SketchEntityId>, Vec<SketchEntityId>) {
    polyline(
        &[
            (0.0, 0.0),
            (40.0, 0.0),
            (40.0, 30.0),
            (0.0, 30.0),
            (0.0, gap),
        ],
        false,
    )
}

#[test]
fn a_standard_tier_project_reads_its_router_tool_and_runs_the_corner_check() {
    let mut f = board_of_standard_project();
    let l_board = [
        (0.0, 0.0),
        (20.0, 0.0),
        (20.0, 10.0),
        (10.0, 10.0),
        (10.0, 20.0),
        (0.0, 20.0),
    ];
    let (sketch, _, edges) = polyline(&l_board, true);
    sketch_in(&mut f, sketch, edges[0]);

    let board = props(&f);
    assert_eq!(board.router_tool_diameter, Some(2.0));
    let fab = Translatable::new(ids::BOARD_SKETCH_FAB_ROUTER).arg("diameter", "2.00");
    assert_eq!(english(&fab), "Router tool Ø 2.00 · from profile");

    let failed: Vec<CheckLine> = board
        .outline_checks
        .iter()
        .filter(|l| l.status == CheckStatus::Failed)
        .cloned()
        .collect();
    assert_eq!(
        texts(&failed),
        ["Inside corner R 0.00 < R 1.00 at (10.00, 10.00)"],
        "{:#?}",
        texts(&board.outline_checks)
    );
    assert!(failed[0].place.is_some(), "a click selects the corner");

    // Decision 3: the check warns only; Bake still applies the sketch.
    send(
        &mut f,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakeRequested)),
    );
    assert!(!in_sketch(&f), "Bake applies and leaves");
    assert_eq!(live(&f).outline.len(), 6, "the L outline reached the board");
}

#[test]
fn without_a_project_router_tool_the_corner_check_says_why() {
    let mut f = open_board();
    let (sketch, _, edges) = polyline(&[(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)], true);
    sketch_in(&mut f, sketch, edges[0]);
    let board = props(&f);
    assert_eq!(board.router_tool_diameter, None);
    assert_eq!(
        texts(&board.outline_checks)[3],
        "Inside corners: not checked, the project's fab profile names no router tool"
    );
}

#[test]
fn a_small_outline_gap_offers_close_gap_and_closing_is_one_undo_step() {
    let mut f = board_of_standard_project();
    let (sketch, p, edges) = open_rectangle(0.03);
    sketch_in(&mut f, sketch, edges[0]);

    let lines = props(&f).outline_checks;
    assert_eq!(lines[0].status, CheckStatus::Failed, "{:#?}", texts(&lines));
    let offer = lines[0]
        .close_gap
        .expect("the open outline offers Close gap");
    assert!(offer.closes());
    assert_eq!(
        english(&offer.text()),
        "Gap 0.03 mm, closes under the 0.05 mm heal tolerance"
    );
    let ends = [offer.keep, offer.merge];
    assert!(ends.contains(&p[0]) && ends.contains(&p[4]), "{offer:?}");

    // The bake preview refuses the open outline.
    send(
        &mut f,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakePreviewToggled)),
    );
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(matches!(
        session.preview_error,
        Some(signex_bake::BoardBakeError::OpenProfile { .. })
    ));
    let before = session.working_sketch();

    panel(
        &mut f,
        PanelMsg::SketchFix(SketchFixMsg::CloseGapClicked {
            keep: offer.keep,
            merge: offer.merge,
        }),
    );

    let lines = props(&f).outline_checks;
    assert_eq!(
        english(&lines[0].text),
        "Closed: one loop",
        "{:#?}",
        texts(&lines)
    );
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(session.preview_error.is_none(), "the outline bakes now");

    send(&mut f, Message::Edit(EditMsg::Undo));
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert_eq!(session.working_sketch(), before, "one undo step reopens it");
    send(&mut f, Message::Edit(EditMsg::Redo));

    send(
        &mut f,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakeRequested)),
    );
    assert!(!in_sketch(&f), "Bake applies and leaves");
    assert_eq!(
        live(&f).outline.len(),
        4,
        "the closed outline reached the board"
    );
}

#[test]
fn a_wide_outline_gap_offers_no_button_and_says_why() {
    let mut f = board_of_standard_project();
    let (sketch, _, edges) = open_rectangle(0.2);
    sketch_in(&mut f, sketch, edges[0]);

    let lines = props(&f).outline_checks;
    let offer = lines[0].close_gap.expect("the gap is still named");
    assert!(!offer.closes(), "no Close gap button");
    assert_eq!(
        english(&offer.text()),
        "Gap 0.20 mm is wider than the 0.05 mm heal tolerance"
    );

    // Asked anyway (a stale button), nothing is joined.
    let before = f
        .app
        .interaction_state
        .board_sketch
        .as_ref()
        .unwrap()
        .working_sketch();
    panel(
        &mut f,
        PanelMsg::SketchFix(SketchFixMsg::CloseGapClicked {
            keep: offer.keep,
            merge: offer.merge,
        }),
    );
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert_eq!(session.working_sketch(), before);
}

#[test]
fn a_footprint_with_an_open_profile_offers_close_gap_and_closes_it() {
    let mut f = open_board();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("open.snxfpt");
    let (mut sketch, p, edges) = polyline(
        &[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0), (0.0, 0.02)],
        false,
    );
    for entity in &mut sketch.entities {
        if edges.contains(&entity.id) {
            entity.board_outline = false;
            entity.courtyard = Some(CourtyardAttr {});
        }
    }
    let params = signex_sketch::parameter::resolve(&sketch.parameters).unwrap();
    let solve = signex_sketch::Solver::default()
        .solve(&sketch, &params)
        .unwrap();
    let mut footprint = Footprint::empty("open");
    footprint.sketch = Some(sketch);
    let mut editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(footprint));
    editor.state.last_solve = Some(solve);
    let docs = &mut f.app.document_state;
    docs.footprint_editors.insert(path.clone(), editor);
    docs.tabs.push(TabInfo {
        title: "open".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    docs.active_tab = docs.tabs.len() - 1;

    let editor = &f.app.document_state.footprint_editors[&path];
    let offer = editor
        .state
        .gap_offer(editor.primitive().sketch.as_ref())
        .expect("the bake preview offers Close gap");
    assert!(offer.closes());
    let ends = [offer.keep, offer.merge];
    assert!(ends.contains(&p[0]) && ends.contains(&p[4]), "{offer:?}");

    send(
        &mut f,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchCloseGapRequested {
                keep: offer.keep,
                merge: offer.merge,
            }),
        }),
    );

    let editor = &f.app.document_state.footprint_editors[&path];
    let sketch = editor.primitive().sketch.as_ref().unwrap();
    assert!(sketch.entities.iter().all(|e| e.id != offer.merge));
    assert_eq!(
        editor.state.gap_offer(Some(sketch)),
        None,
        "the profile is closed"
    );
    assert!(
        !editor.primitive().courtyard.points.is_empty(),
        "the courtyard bakes"
    );
}
