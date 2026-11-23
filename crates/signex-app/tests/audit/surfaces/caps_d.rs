//! Board sketch states of mockup rows m179, m179c, m179d and m179e, driven
//! through real editor messages on a parametric 60 x 40 mm board. `open`
//! fails when the state was not reached.
//!
//! Not built, so no row here: the enclosure keep-in of m179e, and the
//! separate check page and overlap merge of m179c.

use super::helpers_d::reached;
use super::helpers_f as h;
use super::states_sketch;
use super::view_click::click_view;
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::surface::{OpenFn, Surface, WindowSel};
use iced::mouse::Button;
use signex_app::app::{BoardSketchMsg, DxfImportMsg, Message, PcbMsg, Signex, UiMsg};
use signex_app::library::LibraryMessage;
use signex_app::library::editor::footprint::SketchTool;
use signex_app::library::editor::footprint::sketch_mode::SketchCanvasMsg;
use signex_app::library::messages::{
    FootprintEditorMsg, PrimitiveEdit, RoleTag, SketchConstraintTag,
};
use signex_bake::board::CheckState;
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use std::path::PathBuf;

/// Where the Import DXF card sits: the canvas's bottom right.
const DXF_CARD: Crop = Crop {
    x: 600,
    y: 400,
    w: 640,
    h: 490,
};

fn row(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    Surface {
        id,
        title,
        open,
        window: WindowSel::Main,
        checks: &[],
    }
}

pub fn rows() -> Vec<Surface> {
    vec![
        row("m179-slot", "Board sketch: Slot selected", open_slot),
        row("m179-full", "Board sketch: outline constrained", open_full),
        row("m179-over", "Board sketch: rejected conflict", open_over),
        row("m179-bake", "Board sketch: bake preview", open_bake),
        row("m179c-check", "Import DXF: open profile", |app, r, fx| {
            open_dxf(app, r, fx, 0.2)?;
            let (healed, open) = dxf_counts(app)?;
            reached(
                healed == 0 && open > 0,
                "a 0.2 mm gap did not read as open ends",
            )
        }),
        row("m179c-healed", "Import DXF: gap healed", |app, r, fx| {
            open_dxf(app, r, fx, 0.04)?;
            let (healed, open) = dxf_counts(app)?;
            reached(
                healed == 1 && open == 0,
                "a 0.04 mm gap did not read as one healed gap",
            )
        }),
        row("m179c-imported", "Import DXF: imported", open_dxf_imported),
        row("m179d-fixed", "Outline checks: fillets added", open_fixed),
        row("m179e-pattern", "Holes on a driven pattern", open_pattern),
    ]
}

/// Redraw the canvas words: an idle pointer move reaches the sync that
/// board sketch messages skip, so a pattern glyph carries its kind letter.
pub(super) fn sync_canvas_text(app: &mut Signex) {
    let (x, y) = app.interaction_state.last_mouse_pos;
    let _ = app.update(Message::Ui(UiMsg::DragMove(x, y)));
}

fn board_path(app: &Signex) -> Result<PathBuf, String> {
    app.interaction_state
        .board_sketch
        .as_ref()
        .map(|session| session.board_path.clone())
        .ok_or_else(|| "the board is not in sketch mode".to_string())
}

/// One sketch editor event for the board in sketch mode, with its tasks.
fn send(app: &mut Signex, msg: FootprintEditorMsg) -> Result<(), String> {
    let path = board_path(app)?;
    h::settle(
        app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(msg),
        }),
    )
}

/// The parametric board: outline, its dimensions and a 2 x 2 hole pattern.
struct ParamBoard {
    sketch: SketchData,
    plane: PlaneId,
    corners: [SketchEntityId; 4],
    hole: SketchEntityId,
    pattern: ArrayId,
}

fn add(sketch: &mut SketchData, plane: PlaneId, kind: EntityKind) -> SketchEntityId {
    let id = SketchEntityId::new();
    sketch.entities.push(Entity::new(id, plane, kind));
    id
}

fn constrain(sketch: &mut SketchData, kind: ConstraintKind) {
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind,
    });
}

/// A closed rectangle of lines through `at`, in order.
fn closed_loop(
    sketch: &mut SketchData,
    plane: PlaneId,
    at: [(f64, f64); 4],
) -> ([SketchEntityId; 4], [SketchEntityId; 4]) {
    let corners = at.map(|(x, y)| add(sketch, plane, EntityKind::Point { x, y }));
    let lines = [0, 1, 2, 3].map(|i| {
        add(
            sketch,
            plane,
            EntityKind::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
            },
        )
    });
    (corners, lines)
}

fn param_board() -> ParamBoard {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    sketch.parameters.insert("board_w", "60mm");
    sketch.parameters.insert("board_h", "40mm");
    sketch.parameters.insert("hole_inset", "3.5mm");
    let (corners, lines) = closed_loop(
        &mut sketch,
        plane,
        [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)],
    );
    for entity in &mut sketch.entities {
        if lines.contains(&entity.id) {
            entity.board_outline = true;
        }
    }
    let shape = [
        ConstraintKind::Fixed { point: corners[0] },
        ConstraintKind::Horizontal { line: lines[0] },
        ConstraintKind::Vertical { line: lines[1] },
        ConstraintKind::Horizontal { line: lines[2] },
        ConstraintKind::Vertical { line: lines[3] },
        ConstraintKind::DistancePtPt {
            p1: corners[0],
            p2: corners[1],
            target: DimTarget::Expr("board_w".into()),
        },
        ConstraintKind::DistancePtPt {
            p1: corners[1],
            p2: corners[2],
            target: DimTarget::Expr("board_h".into()),
        },
    ];
    for kind in shape {
        constrain(&mut sketch, kind);
    }
    let center = add(&mut sketch, plane, EntityKind::Point { x: 3.5, y: 3.5 });
    let hole = add(
        &mut sketch,
        plane,
        EntityKind::Circle {
            center,
            radius: 1.6,
        },
    );
    let pattern = ArrayId::new();
    sketch.arrays.push(Array {
        id: pattern,
        kind: ArrayKind::Grid {
            source: hole,
            nx_expr: "2".into(),
            ny_expr: "2".into(),
            dx_expr: "board_w - 2 * hole_inset".into(),
            dy_expr: "board_h - 2 * hole_inset".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    ParamBoard {
        sketch,
        plane,
        corners,
        hole,
        pattern,
    }
}

/// The board open in sketch mode with its hole a mounting hole.
fn open_param(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    sketch: SketchData,
    hole: SketchEntityId,
) -> Result<(), String> {
    states_sketch::open_board_sketch(app, r, fx, sketch)?;
    send(
        app,
        FootprintEditorMsg::SketchSetRole {
            id: hole,
            role: RoleTag::MountingHole,
        },
    )
}

/// Draw a slot with the Slot tool's three clicks; returns one of its lines.
fn draw_slot(app: &mut Signex) -> Result<SketchEntityId, String> {
    let sketch_ids = |app: &Signex| -> Result<Vec<SketchEntityId>, String> {
        let session = app
            .interaction_state
            .board_sketch
            .as_ref()
            .ok_or("the board is not in sketch mode")?;
        Ok(session
            .working_sketch()
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
            .map(|e| e.id)
            .collect())
    };
    let before = sketch_ids(app)?;
    send(app, FootprintEditorMsg::SketchSetTool(SketchTool::Slot))?;
    for (x_mm, y_mm) in [(20.0, 12.0), (34.0, 12.0), (27.0, 13.5)] {
        send(
            app,
            FootprintEditorMsg::SketchToolClick {
                x_mm,
                y_mm,
                snap_id: None,
            },
        )?;
    }
    send(app, FootprintEditorMsg::SketchSetTool(SketchTool::Select))?;
    sketch_ids(app)?
        .into_iter()
        .find(|id| !before.contains(id))
        .ok_or_else(|| "the Slot tool drew no line".to_string())
}

/// Slot selected, with an antenna keepout and the hole pattern beside it.
fn open_slot(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let mut board = param_board();
    let (_, keepout) = closed_loop(
        &mut board.sketch,
        board.plane,
        [(40.0, 26.0), (52.0, 26.0), (52.0, 34.0), (40.0, 34.0)],
    );
    open_param(app, r, fx, board.sketch, board.hole)?;
    send(
        app,
        FootprintEditorMsg::SketchProfileRolePicked {
            seed: keepout[0],
            role: RoleTag::Keepout,
        },
    )?;
    let slot = draw_slot(app)?;
    send(
        app,
        FootprintEditorMsg::SketchProfileRolePicked {
            seed: slot,
            role: RoleTag::Slot,
        },
    )?;
    send(
        app,
        FootprintEditorMsg::SketchSelect {
            id: Some(slot),
            shift: false,
        },
    )?;
    sync_canvas_text(app);
    let selected = app
        .interaction_state
        .board_sketch
        .as_ref()
        .is_some_and(|s| {
            s.editor.state.selected_sketch == Some(slot)
                && s.working_sketch()
                    .entities
                    .iter()
                    .any(|e| e.id == slot && e.board_slot.is_some())
        });
    reached(
        selected,
        "the drawn slot is not selected with the Slot role",
    )
}

fn open_full(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let board = param_board();
    open_param(app, r, fx, board.sketch, board.hole)?;
    sync_canvas_text(app);
    let clean = app
        .interaction_state
        .board_sketch
        .as_ref()
        .is_some_and(|s| s.editor.state.conflict.is_empty());
    reached(clean, "the constrained outline holds a conflict")
}

/// A 41 mm distance on the right edge, which `board_h` already fixes at 40:
/// the solver rejects it and the sketch keeps its last good solve.
fn open_over(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let board = param_board();
    let edge = vec![board.corners[1], board.corners[2]];
    open_param(app, r, fx, board.sketch, board.hole)?;
    // The rejection's warning would stay in the log of later rows.
    h::unlogged(|| {
        send(app, FootprintEditorMsg::SketchSelectMany(edge))?;
        send(app, FootprintEditorMsg::SketchDimensionInput("41".into()))?;
        send(
            app,
            FootprintEditorMsg::SketchAddConstraintForSelection(SketchConstraintTag::DistancePtPt),
        )
    })?;
    sync_canvas_text(app);
    let rejected = app
        .interaction_state
        .board_sketch
        .as_ref()
        .is_some_and(|s| !s.editor.state.conflict.is_empty());
    reached(rejected, "the conflicting distance was not rejected")
}

fn open_bake(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let board = param_board();
    open_param(app, r, fx, board.sketch, board.hole)?;
    h::settle(
        app,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakePreviewToggled)),
    )?;
    sync_canvas_text(app);
    let baked = app
        .interaction_state
        .board_sketch
        .as_ref()
        .is_some_and(|s| s.preview.is_some() && s.preview_error.is_none());
    reached(baked, "the bake preview did not open on a clean bake")
}

/// A 60 x 40 mm lid on BOARD with a `gap` mm break at its top right, four
/// holes on HOLES and a note on TEXT.
fn lid_dxf(gap: f64) -> String {
    let mut text = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    let edges = [
        (0.0, 0.0, 60.0, 0.0),
        (60.0, 0.0, 60.0, 40.0 - gap),
        (60.0, 40.0, 0.0, 40.0),
        (0.0, 40.0, 0.0, 0.0),
    ];
    for (x1, y1, x2, y2) in edges {
        text.push_str(&format!(
            "0\nLINE\n8\nBOARD\n10\n{x1}\n20\n{y1}\n11\n{x2}\n21\n{y2}\n"
        ));
    }
    for (x, y) in [(3.5, 3.5), (56.5, 3.5), (56.5, 36.5), (3.5, 36.5)] {
        text.push_str(&format!("0\nCIRCLE\n8\nHOLES\n10\n{x}\n20\n{y}\n40\n1.6\n"));
    }
    text.push_str("0\nTEXT\n8\nTEXT\n10\n0\n20\n0\n0\nENDSEC\n0\nEOF\n");
    text
}

/// The Import DXF card open on [`lid_dxf`] over the fixture board's sketch.
fn open_dxf(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    gap: f64,
) -> Result<(), String> {
    fixture::open_board(app, r, fx);
    h::command(app, "board_sketch")?;
    let _ = render::draw_main(r, app);
    let board = board_path(app)?;
    let source =
        signex_engine::pcb::dxf::DxfSource::read(&lid_dxf(gap)).map_err(|e| e.to_string())?;
    h::settle(
        app,
        Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::SketchRead {
            board,
            path: "enclosure-lid.dxf".into(),
            source: Ok(std::sync::Arc::new(source)),
        })),
    )?;
    let _ = render::draw_main(r, app);
    Ok(())
}

/// The open card's healed gaps and open ends.
fn dxf_counts(app: &Signex) -> Result<(usize, usize), String> {
    let dialog = app
        .interaction_state
        .board_sketch
        .as_ref()
        .and_then(|s| s.dxf_dialog.as_ref())
        .ok_or("the Import DXF card did not open")?;
    match &dialog.check {
        Ok(check) => Ok((check.healed, check.open_ends)),
        Err(problem) => Err(format!("the card refuses the import: {problem:?}")),
    }
}

/// The healed lid after the card's Import button, whose message type the
/// crate does not export.
fn open_dxf_imported(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_dxf(app, r, fx, 0.04)?;
    click_view(app, r, DXF_CARD, Button::Left, |message| {
        matches!(
            message,
            Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::SketchDialog(event)))
                if format!("{event:?}") == "Confirmed"
        )
    })?;
    sync_canvas_text(app);
    let session = app
        .interaction_state
        .board_sketch
        .as_ref()
        .ok_or("the board left sketch mode")?;
    let sketch = session.working_sketch();
    let outline = sketch.entities.iter().filter(|e| e.board_outline).count();
    let holes = sketch
        .entities
        .iter()
        .filter(|e| e.mounting_hole.is_some())
        .count();
    reached(
        session.dxf_dialog.is_none() && outline == 4 && holes == 4,
        "Import did not land one outline and four holes",
    )
}

/// The failing inside corner of m179d after Add fillets.
fn open_fixed(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    states_sketch::open_outline_checks(app, r, fx)?;
    send(app, FootprintEditorMsg::SketchAddFilletsRequested)?;
    sync_canvas_text(app);
    let session = app
        .interaction_state
        .board_sketch
        .as_ref()
        .ok_or("the board left sketch mode")?;
    let sketch = session.working_sketch();
    let passed = session
        .editor
        .state
        .board_checks(Some(&sketch))
        .is_some_and(|checks| matches!(checks.inside_corners, CheckState::Passed));
    reached(passed, "Add fillets left the inside corner failing")
}

/// The hole pattern selected, its spacing driven by the size parameters.
fn open_pattern(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let board = param_board();
    let pattern = board.pattern;
    open_param(app, r, fx, board.sketch, board.hole)?;
    send(
        app,
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::PatternSelected(Some(pattern))),
    )?;
    sync_canvas_text(app);
    let selected = app
        .interaction_state
        .board_sketch
        .as_ref()
        .is_some_and(|s| s.editor.state.sketch_overlay.selected_pattern == Some(pattern));
    reached(selected, "the hole pattern is not selected")
}
