//! Sketch states on fixtures with real geometry: patterns, mirrors,
//! dimensions, roles, a broken outline and a detached panel. `open` fails
//! when the state was not reached.
//!
//! Not built, so no row here: linked board features and the enclosure
//! keep-in of m179b and m179e (no Create > Project tool or enclosure type).

use super::helpers_c;
use super::helpers_d::{self, reached};
use super::helpers_f as h;
use super::lib;
use crate::fixture::{self, Fixture};
use crate::render;
use crate::surface::{OpenFn, Surface, WindowSel};
use signex_app::app::{
    AssignFootprintsMsg, BoardSketchMsg, Message, PcbMsg, Signex, UpdatePcbMsg, WindowMsg,
};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::library::LibraryMessage;
use signex_app::library::editor::footprint::EditorMode;
use signex_app::library::editor::footprint::sketch_mode::SketchCanvasMsg;
use signex_app::library::messages::{FootprintEditorMsg, PrimitiveEdit, RoleTag};
use signex_app::panels::PanelKind;
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::format::SnxPcb;
use signex_types::pcb_sketch::BoardSketch;
use signex_types::sketch::pattern_links::PatternMirror;
use std::path::{Path, PathBuf};

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
    let mut rows = footprint_rows();
    rows.extend(board_rows());
    rows.extend(panel_rows());
    rows
}

fn footprint_rows() -> Vec<Surface> {
    vec![
        row("m126-keepout", "Properties: Keepout", open_keepout),
        row("m187-dimensions", "Sketch with dimensions", |app, r, fx| {
            let (draft, ..) = dimensioned_body();
            open_sketch(app, r, fx, draft.sketch).map(|_| ())
        }),
        row("m188-menu", "Constraint menu", open_constraint_menu),
        row("m190-roles", "Bake: roles", |app, r, fx| {
            open_roles(app, r, fx, 4).map(|_| ())
        }),
        row("m190-errors", "Bake: open courtyard", |app, r, fx| {
            let path = open_roles(app, r, fx, 3)?;
            edit(app, &path, FootprintEditorMsg::SketchBakePreviewToggled)
        }),
        row("m190-baked", "Bake: result", |app, r, fx| {
            let path = open_roles(app, r, fx, 4)?;
            edit(app, &path, FootprintEditorMsg::SketchBakeRequested)
        }),
        row("m191-linear", "Linear pattern", |app, r, fx| {
            let (draft, _) = patterned(false);
            open_sketch(app, r, fx, draft.sketch).map(|_| ())
        }),
        row("m191-circular", "Circular pattern", |app, r, fx| {
            let (draft, _) = patterned(true);
            open_sketch(app, r, fx, draft.sketch).map(|_| ())
        }),
        row("m191-mirror", "Mirror about a centerline", |app, r, fx| {
            let (mut draft, array) = patterned(false);
            let (a, b) = (draft.point(0.0, -4.0), draft.point(0.0, 4.0));
            let line = draft.line(a, b);
            if let Some(entity) = draft.sketch.entities.iter_mut().find(|e| e.id == line) {
                entity.centerline = true;
            }
            draft.sketch.pattern_mirrors.push(PatternMirror {
                id: ArrayId::new(),
                pattern: array,
                line,
            });
            open_sketch(app, r, fx, draft.sketch).map(|_| ())
        }),
        row("m192-driving", "Pad's driving geometry", open_pad_geometry),
    ]
}

fn board_rows() -> Vec<Surface> {
    vec![
        row(
            "m179-outline",
            "Board outline, dimensioned",
            |app, r, fx| {
                let (mut draft, lines, corners) = rectangle(0.0, 0.0, 50.0, 40.0);
                mark_outline(&mut draft, &lines);
                draft.distance(corners[0], corners[1], 50.0);
                draft.distance(corners[1], corners[2], 40.0);
                open_board_sketch(app, r, fx, draft.sketch)
            },
        ),
        row("m179d-failing", "Outline crossing itself", |app, r, fx| {
            let mut draft = Draft::new();
            let corners = [
                draft.point(0.0, 0.0),
                draft.point(40.0, 30.0),
                draft.point(40.0, 0.0),
                draft.point(0.0, 30.0),
            ];
            let lines: Vec<_> = (0..4)
                .map(|i| draft.line(corners[i], corners[(i + 1) % 4]))
                .collect();
            mark_outline(&mut draft, &lines);
            open_board_sketch(app, r, fx, draft.sketch)?;
            h::settle(
                app,
                Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakePreviewToggled)),
            )
        }),
        row(
            "m179e-holes",
            "Mounting holes on a pattern",
            |app, r, fx| {
                let (mut draft, lines, _) = rectangle(0.0, 0.0, 50.0, 40.0);
                mark_outline(&mut draft, &lines);
                let center = draft.point(5.0, 5.0);
                let hole = draft.circle(center, 1.6);
                draft.sketch.arrays.push(Array {
                    id: ArrayId::new(),
                    kind: ArrayKind::Linear {
                        source: hole,
                        count_expr: "4".into(),
                        dx_expr: "13mm".into(),
                        dy_expr: "0mm".into(),
                    },
                    numbering: NumberingScheme::default(),
                });
                open_board_sketch(app, r, fx, draft.sketch)?;
                h::fp_edit(
                    app,
                    &fx.pcb.with_file_name("sketched.snxpcb"),
                    FootprintEditorMsg::SketchSetRole {
                        id: hole,
                        role: RoleTag::MountingHole,
                    },
                );
                Ok(())
            },
        ),
    ]
}

fn panel_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m8-detached",
            title: "Panel in its own window",
            open: open_detached_panel,
            window: WindowSel::DetachedPanel,
            checks: &[],
        },
        row("m60-wiki", "Wiki panel", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            helpers_c::show_panel(app, PanelKind::Wiki)
        }),
        Surface {
            window: WindowSel::Auto,
            ..row("m80-ticked", "Assign Footprints: ticked", |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                helpers_d::sample_project(app, fx);
                helpers_d::command(app, "assign_footprints")?;
                reached(
                    app.ui_state.assign_footprints.is_some(),
                    "assign_footprints opened no dialog",
                )?;
                for msg in [
                    AssignFootprintsMsg::AllToggled(true),
                    AssignFootprintsMsg::RowPreviewed(0),
                ] {
                    h::settle(app, Message::from(msg))?;
                }
                Ok(())
            })
        },
        Surface {
            window: WindowSel::Auto,
            ..row("m81-validated", "Update PCB: validated", |app, r, fx| {
                helpers_d::sample_with_board(app, r, fx)?;
                helpers_d::command(app, "update_pcb_from_schematic")?;
                reached(
                    app.ui_state.update_pcb.is_some(),
                    "update_pcb_from_schematic opened no dialog",
                )?;
                h::settle(app, Message::from(UpdatePcbMsg::ValidateRequested))
            })
        },
    ]
}

/// A sketch being built on one board-top plane.
struct Draft {
    sketch: SketchData,
    plane: PlaneId,
}

impl Draft {
    fn new() -> Self {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        Self { sketch, plane }
    }

    fn add(&mut self, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.sketch.entities.push(Entity::new(id, self.plane, kind));
        id
    }

    fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        self.add(EntityKind::Point { x, y })
    }

    fn line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
        self.add(EntityKind::Line { start, end })
    }

    fn circle(&mut self, center: SketchEntityId, radius: f64) -> SketchEntityId {
        self.add(EntityKind::Circle { center, radius })
    }

    fn constrain(&mut self, kind: ConstraintKind) -> ConstraintId {
        let id = ConstraintId::new();
        self.sketch.constraints.push(Constraint { id, kind });
        id
    }

    fn distance(&mut self, p1: SketchEntityId, p2: SketchEntityId, mm: f64) -> ConstraintId {
        self.constrain(ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        })
    }
}

/// A closed rectangle: the draft, its four edges and its four corners.
fn rectangle(
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> (Draft, Vec<SketchEntityId>, [SketchEntityId; 4]) {
    let mut draft = Draft::new();
    let corners = [
        draft.point(x0, y0),
        draft.point(x1, y0),
        draft.point(x1, y1),
        draft.point(x0, y1),
    ];
    let lines = (0..4)
        .map(|i| draft.line(corners[i], corners[(i + 1) % 4]))
        .collect();
    (draft, lines, corners)
}

fn mark_outline(draft: &mut Draft, lines: &[SketchEntityId]) {
    for entity in &mut draft.sketch.entities {
        if lines.contains(&entity.id) {
            entity.board_outline = true;
        }
    }
}

/// A 4 x 4 mm body with its width and height dimensioned.
fn dimensioned_body() -> (Draft, Vec<SketchEntityId>, ConstraintId) {
    let (mut draft, lines, corners) = rectangle(-2.0, -2.0, 2.0, 2.0);
    let width = draft.distance(corners[0], corners[1], 4.0);
    draft.distance(corners[1], corners[2], 4.0);
    (draft, lines, width)
}

/// A pad-sized circle repeated four times, in a row or round a centre.
fn patterned(polar: bool) -> (Draft, ArrayId) {
    let mut draft = Draft::new();
    let seed = draft.point(2.0, 0.0);
    let source = draft.circle(seed, 0.3);
    let kind = if polar {
        ArrayKind::Polar {
            source,
            center: draft.point(0.0, 0.0),
            count_expr: "4".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        }
    } else {
        ArrayKind::Linear {
            source,
            count_expr: "4".into(),
            dx_expr: "1.27mm".into(),
            dy_expr: "0mm".into(),
        }
    };
    let id = ArrayId::new();
    draft.sketch.arrays.push(Array {
        id,
        kind,
        numbering: NumberingScheme::default(),
    });
    (draft, id)
}

/// One footprint-editor event for `path`, with every task it starts.
fn edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) -> Result<(), String> {
    h::settle(
        app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        }),
    )
}

/// `sketch` in a footprint editor tab, in sketch mode.
fn open_sketch(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    sketch: SketchData,
) -> Result<PathBuf, String> {
    let path = fx.footprint.with_file_name("audit-sketch.snxfpt");
    let mut footprint = signex_library::Footprint::empty("QFN-16");
    footprint.sketch = Some(sketch);
    lib::open_footprint_document(app, &path, footprint);
    let _ = render::draw_main(r, app);
    edit(app, &path, FootprintEditorMsg::SetMode(EditorMode::Sketch))?;
    let _ = render::draw_main(r, app);
    Ok(path)
}

/// The primitives board carrying `sketch`, in board sketch mode.
fn open_board_sketch(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    sketch: SketchData,
) -> Result<(), String> {
    let mut board = helpers_d::primitives_board()?;
    board.sketch = Some(BoardSketch {
        sketch,
        ..Default::default()
    });
    let path = fx.pcb.with_file_name("sketched.snxpcb");
    let text = SnxPcb::new(board)
        .write_string()
        .map_err(|e| format!("encode board: {e:?}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write board: {e}"))?;
    h::open(app, &path)?;
    h::show_board(app, r);
    h::command(app, "board_sketch")?;
    let _ = render::draw_main(r, app);
    reached(
        app.interaction_state.board_sketch.is_some(),
        "board_sketch did not enter sketch mode",
    )
}

/// The keepout line selected in sketch mode, where its sub-form shows.
fn open_keepout(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let path = fx.footprint.with_file_name("audit-keepout.snxfpt");
    let (footprint, line) = lib::keepout_footprint();
    lib::open_footprint_document(app, &path, footprint);
    let _ = render::draw_main(r, app);
    edit(app, &path, FootprintEditorMsg::SetMode(EditorMode::Sketch))?;
    edit(
        app,
        &path,
        FootprintEditorMsg::SketchSelect {
            id: Some(line),
            shift: false,
        },
    )
}

/// A pattern's source selected: Properties shows the Pattern sub-form.
pub(super) fn open_pattern_selected(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let (draft, array) = patterned(false);
    let source = draft
        .sketch
        .arrays
        .iter()
        .find(|a| a.id == array)
        .map(|a| a.kind.source())
        .ok_or("the fixture has no pattern")?;
    let path = open_sketch(app, r, fx, draft.sketch)?;
    edit(
        app,
        &path,
        FootprintEditorMsg::SketchSelect {
            id: Some(source),
            shift: false,
        },
    )
}

fn open_constraint_menu(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let (draft, _, width) = dimensioned_body();
    let path = open_sketch(app, r, fx, draft.sketch)?;
    edit(
        app,
        &path,
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::ConstraintMenuOpened {
            constraint: width,
            x: 420.0,
            y: 300.0,
        }),
    )
}

/// A body with `courtyard_edges` of its edges given the courtyard role: all
/// four close the profile, three leave it open.
fn open_roles(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    courtyard_edges: usize,
) -> Result<PathBuf, String> {
    let (draft, lines, _) = dimensioned_body();
    let path = open_sketch(app, r, fx, draft.sketch)?;
    for id in lines.into_iter().take(courtyard_edges) {
        edit(
            app,
            &path,
            FootprintEditorMsg::SketchSetRole {
                id,
                role: RoleTag::Courtyard,
            },
        )?;
    }
    Ok(path)
}

/// A stock footprint in sketch mode with the entity driving a pad selected.
fn open_pad_geometry(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    h::open(app, &fx.footprint)?;
    let _ = render::draw_main(r, app);
    edit(
        app,
        &fx.footprint,
        FootprintEditorMsg::SetMode(EditorMode::Sketch),
    )?;
    let pad = app
        .document_state
        .footprint_editors
        .get(&fx.footprint)
        .and_then(|editor| editor.primitive().sketch.as_ref())
        .and_then(|sketch| sketch.entities.iter().find(|e| e.pad.is_some()))
        .map(|entity| entity.id)
        .ok_or("the stock footprint's sketch drives no pad")?;
    edit(
        app,
        &fx.footprint,
        FootprintEditorMsg::SketchSelect {
            id: Some(pad),
            shift: false,
        },
    )
}

/// The board's Nets panel pulled out of its dock into its own window.
fn open_detached_panel(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_board(app, r, fx);
    helpers_c::show_panel(app, PanelKind::PcbNets)?;
    let (position, index) = [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ]
    .into_iter()
    .find_map(|position| {
        app.document_state
            .dock
            .panel_kinds(position)
            .iter()
            .position(|k| *k == PanelKind::PcbNets)
            .map(|index| (position, index))
    })
    .ok_or("the PCB Nets panel is not docked")?;
    let _ = app.update(Message::Dock(DockMessage::UndockPanel(position, index)));
    reached(
        !app.document_state.dock.floating.is_empty(),
        "the panel did not float",
    )?;
    let _ = app.update(Message::Window(WindowMsg::DetachFloatingPanel(0)));
    reached(
        app.document_state.dock.floating.is_empty(),
        "the floating panel did not detach",
    )
}
