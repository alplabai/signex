//! Footprint sketch updates — tool-click state machine (ADR-0001 D1/D2).
//!
//! `apply` is a thin router (the one message here, `SketchToolClick`,
//! delegates to [`handle_tool_click`]). `handle_tool_click` resolves the
//! click into a [`ToolClickCtx`] (the sketch plane, the snapped-or-minted
//! click Point, the raw coords, the sticky construction / centerline
//! flags) through three named steps —
//! [`resolve_effective_click`] (numeric-placement-input override),
//! [`resolve_click_point`] (snap/mint the click into an entity id), and
//! [`try_consume_repick_polar_center`] (the Pattern re-pick intercept) —
//! then dispatches to the per-tool sub-modules. Bodies moved verbatim.

mod draw;
mod edit;
mod transform;

use crate::library::messages::FootprintEditorMsg;
use signex_sketch::entity::Entity;
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::PlaneId;

/// The per-click state the tool sub-modules read: the sketch plane, the
/// resolved (snapped or freshly-minted) click Point, the raw click position,
/// and the sticky construction / centerline flags applied to new entities.
pub(super) struct ToolClickCtx {
    plane_id: PlaneId,
    resolved_id: SketchEntityId,
    x_mm: f64,
    y_mm: f64,
    construction_mode: bool,
    centerline_mode: bool,
}

impl ToolClickCtx {
    /// Stamp the sticky construction / centerline flags onto a new entity.
    fn flag(&self, mut e: Entity) -> Entity {
        e.construction = self.construction_mode;
        e.centerline = self.centerline_mode;
        e
    }
}

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut crate::app::FootprintEditorState,
    msg: FootprintEditorMsg,
) {
    match msg {
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        } => handle_tool_click(editor, x_mm, y_mm, snap_id),
        _ => unreachable!("non-tool-click state machine sketch variant routed to sketch_tools.rs"),
    }
}

fn handle_tool_click(
    editor: &mut crate::app::FootprintEditorState,
    x_mm: f64,
    y_mm: f64,
    snap_id: Option<SketchEntityId>,
) {
    use crate::library::editor::footprint::state::SketchTool;

    // v0.14-footprint — TAB-pause is the single source of truth
    // for "suppress click-commit". The canvas layer also gates
    // on `placement_paused` before publishing this message, but
    // multi-click tools (Line / RoundedRectangle / Arc / …)
    // route BOTH their anchor click and their commit click
    // through this one handler, so the authoritative gate lives
    // here too: while paused, drop the click before it can
    // advance `tool_pending` or mint geometry. The Select tool
    // never reaches this arm, so re-anchoring stays possible.
    if editor.state.placement_paused {
        return;
    }

    // v0.16.1 — sticky construction flag captured once so each
    // newly-minted entity can be flagged in one place. Pads
    // (PadAttr-carrying centre Points minted via auto_mint /
    // mirror_add) intentionally bypass this; the bake skips
    // construction entities and a construction pad would
    // disappear from the rendered output.
    let construction_mode = editor.state.construction_mode;
    let centerline_mode = editor.state.centerline_mode;

    // Resolve the click into either an existing snap Point or a
    // freshly-minted Point. For multi-click tools (Line / Rect /
    // Circle / Arc), the dispatcher reuses the snap target's ID
    // so closed-loop detection (canvas.rs::draw_filled_closed_
    // loops) continues to recognise cycles by shared endpoint
    // ID. Otherwise it appends a Point at the click position
    // and uses that new ID for the active tool's gesture state.
    let plane_id = resolve_sketch_plane(editor);

    let (eff_x_mm, eff_y_mm, used_placement_input) = resolve_effective_click(editor, x_mm, y_mm);
    // When numeric input pinned the click, ignore the snap
    // hit (the user explicitly asked for a different
    // distance / angle).
    let effective_snap_id = if used_placement_input { None } else { snap_id };

    let resolved_id = resolve_click_point(
        editor,
        plane_id,
        eff_x_mm,
        eff_y_mm,
        effective_snap_id,
        construction_mode,
        centerline_mode,
    );

    if try_consume_repick_polar_center(editor, resolved_id) {
        editor.canvas_cache.clear();
        editor.dirty = true;
        return;
    }

    let ctx = ToolClickCtx {
        plane_id,
        resolved_id,
        x_mm,
        y_mm,
        construction_mode,
        centerline_mode,
    };

    // Per-tool state machine — advance `tool_pending` and emit the
    // gesture-completing AddEntity when ready. Exhaustive over
    // `SketchTool`; a new tool is a compile error until routed.
    let tool = editor.state.active_tool;
    match tool {
        SketchTool::Select
        | SketchTool::Point
        | SketchTool::Line
        | SketchTool::Circle
        | SketchTool::RoundedRectangle
        | SketchTool::Rectangle
        | SketchTool::Arc
        | SketchTool::TangentArc => draw::apply(editor, &ctx, tool),
        SketchTool::Mirror
        | SketchTool::Offset
        | SketchTool::RectPattern
        | SketchTool::CircularPattern => transform::apply(editor, &ctx, tool),
        // #372 — Break Track joins Fillet / Trim as a curve
        // edit: a single click hit-tests a Line and splits it.
        SketchTool::Fillet | SketchTool::Trim | SketchTool::BreakTrack => {
            edit::apply(editor, &ctx, tool)
        }
        // #361 — Drag Track End is not a click-to-place gesture:
        // it arms an endpoint drag on PRESS in the canvas
        // (`try_drag_track_end_grab`) and never publishes a
        // SketchToolClick, so this dispatch is unreachable for it.
        SketchTool::DragTrackEnd => {}
    }
    // v0.24 Track D — buffer is consumed once per click. The
    // user has to type again before the next gesture step,
    // mirroring Fusion. Always clear when the resolve step
    // honoured the buffer; leave alone otherwise so a stray
    // pre-tool-pending keystroke survives until the user
    // either commits or Esc-clears.
    if used_placement_input {
        editor.state.placement_input = None;
        // v0.14-footprint — drop every parked dimension field
        // too so the next gesture starts with a clean buffer.
        editor.state.placement_input_others.clear();
    }
    editor.canvas_cache.clear();
    editor.dirty = true;
}

// v0.22 Phase A1 — ensure the sketch has at least one plane so a fresh
// click has somewhere to mint its Point.
fn resolve_sketch_plane(editor: &mut crate::app::FootprintEditorState) -> PlaneId {
    use signex_sketch::plane::{Plane, PlaneKind};
    match editor.primitive().sketch.as_ref() {
        Some(s) if !s.planes.is_empty() => s.planes[0].id,
        _ => {
            let pid = PlaneId::new();
            let sketch = editor
                .primitive_mut()
                .sketch
                .get_or_insert_with(signex_sketch::SketchData::default);
            sketch.planes.push(Plane {
                id: pid,
                kind: PlaneKind::BoardTop,
            });
            pid
        }
    }
}

// v0.24 Track D — consume `state.placement_input` if it matches the
// active tool's pending state. The buffer is parsed as `f64` mm
// (length / radius) or degrees (sweep), translated into an effective
// click position overriding `x_mm` / `y_mm`. Returns the effective
// `(x, y)` and a flag whose `true` value means the click was
// geometry-pinned by a numeric input — used by the caller to (1) ignore
// `snap_id` and (2) clear `state.placement_input` after the gesture
// commits.
fn resolve_effective_click(
    editor: &crate::app::FootprintEditorState,
    x_mm: f64,
    y_mm: f64,
) -> (f64, f64, bool) {
    use crate::library::editor::footprint::state::{PlacementInputKind, SketchTool, ToolPending};
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let placement_input_kind = editor.state.placement_input.as_ref().map(|p| p.kind);
    let placement_input_value = editor
        .state
        .placement_input
        .as_ref()
        .and_then(|p| p.buffer.parse::<f64>().ok());
    // v0.14-footprint — multi-dimension tools (Line len/angle,
    // Rectangle w/h, Rounded-Rect w/h/radius) keep the focused
    // field in `placement_input` and the rest in
    // `placement_input_others`. Pull a field's parsed value out
    // of whichever slot holds it so the commit arms can honour
    // any combination regardless of which field has focus.
    let field_value = |kind: PlacementInputKind| -> Option<f64> {
        std::iter::once(editor.state.placement_input.as_ref())
            .chain(editor.state.placement_input_others.iter().map(Some))
            .flatten()
            .find(|p| p.kind == kind)
            .and_then(|p| p.buffer.parse::<f64>().ok())
    };
    let line_len_typed = field_value(PlacementInputKind::LineLength);
    let line_ang_typed = field_value(PlacementInputKind::LineAngle);
    let rect_w_typed = field_value(PlacementInputKind::RectWidth);
    let rect_h_typed = field_value(PlacementInputKind::RectHeight);
    let resolve_point_xy = |id: SketchEntityId,
                            primitive: &signex_library::primitive::footprint::Footprint|
     -> Option<(f64, f64)> {
        primitive
            .sketch
            .as_ref()
            .and_then(|s| s.entities.iter().find(|e| e.id == id))
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    match (
        placement_input_kind,
        placement_input_value,
        editor.state.active_tool,
        editor.state.tool_pending.clone(),
    ) {
        // Line second click — honour any typed length / angle.
        // v0.14-footprint:
        //   • length + angle → endpoint = first + (len @ angle°)
        //   • length only    → len along the cursor azimuth (legacy)
        //   • angle only     → azimuth pinned to angle°, length
        //                      taken from the cursor distance
        // The angle is degrees CCW from +X in world space, the
        // same convention the live ghost-preview pill displays
        // (draw_sketch.rs), so the committed segment matches the
        // number the user saw while placing.
        (_, _, SketchTool::Line, ToolPending::LineFirst { first })
            if line_len_typed.is_some() || line_ang_typed.is_some() =>
        {
            let primitive = editor.primitive();
            if let Some((fx, fy)) = resolve_point_xy(first, primitive) {
                let dx = x_mm - fx;
                let dy = y_mm - fy;
                let cursor_len = (dx * dx + dy * dy).sqrt();
                // World azimuth of the cursor relative to the
                // first endpoint; 0 when the cursor sits exactly
                // on `first` (no direction to read).
                let cursor_ang = if cursor_len > 1e-9 { dy.atan2(dx) } else { 0.0 };
                // Typed angle wins; else follow the cursor.
                let ang_rad = match line_ang_typed {
                    Some(a) => a.to_radians(),
                    None => cursor_ang,
                };
                // Typed (positive) length wins; else use the
                // cursor distance so an angle-only entry still
                // commits a sensibly-sized segment.
                let len = match line_len_typed {
                    Some(l) if l > 0.0 => l,
                    _ => cursor_len,
                };
                if len > 1e-9 {
                    (fx + len * ang_rad.cos(), fy + len * ang_rad.sin(), true)
                } else {
                    // Neither a typed length nor a usable cursor
                    // distance — fall back to the raw click.
                    (x_mm, y_mm, false)
                }
            } else {
                (x_mm, y_mm, false)
            }
        }
        // Circle second click — radius from centre, along
        // the cursor azimuth.
        (
            Some(PlacementInputKind::CircleRadius),
            Some(r),
            SketchTool::Circle,
            ToolPending::CircleCenter { center },
        ) if r > 0.0 => {
            let primitive = editor.primitive();
            if let Some((cx, cy)) = resolve_point_xy(center, primitive) {
                let dx = x_mm - cx;
                let dy = y_mm - cy;
                let cursor_len = (dx * dx + dy * dy).sqrt();
                if cursor_len > 1e-9 {
                    let ux = dx / cursor_len;
                    let uy = dy / cursor_len;
                    (cx + r * ux, cy + r * uy, true)
                } else {
                    // Cursor at centre → fall back; the user
                    // can re-position before clicking.
                    (x_mm, y_mm, false)
                }
            } else {
                (x_mm, y_mm, false)
            }
        }
        // Arc second click — start endpoint at exact radius
        // from centre, along cursor azimuth.
        (
            Some(PlacementInputKind::ArcRadius),
            Some(r),
            SketchTool::Arc,
            ToolPending::ArcCenter { center },
        ) if r > 0.0 => {
            let primitive = editor.primitive();
            if let Some((cx, cy)) = resolve_point_xy(center, primitive) {
                let dx = x_mm - cx;
                let dy = y_mm - cy;
                let cursor_len = (dx * dx + dy * dy).sqrt();
                if cursor_len > 1e-9 {
                    let ux = dx / cursor_len;
                    let uy = dy / cursor_len;
                    (cx + r * ux, cy + r * uy, true)
                } else {
                    (x_mm, y_mm, false)
                }
            } else {
                (x_mm, y_mm, false)
            }
        }
        // Arc third click — sweep from `start` by typed
        // degrees CCW around `center`. Radius is the
        // committed |centre, start| distance.
        (
            Some(PlacementInputKind::ArcSweep),
            Some(deg),
            SketchTool::Arc,
            ToolPending::ArcStart { center, start },
        ) => {
            let primitive = editor.primitive();
            let parts = (
                resolve_point_xy(center, primitive),
                resolve_point_xy(start, primitive),
            );
            if let (Some((cx, cy)), Some((sx, sy))) = parts {
                let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                if r > 1e-9 {
                    let start_ang = (sy - cy).atan2(sx - cx);
                    let end_ang = start_ang + deg.to_radians();
                    (cx + r * end_ang.cos(), cy + r * end_ang.sin(), true)
                } else {
                    (x_mm, y_mm, false)
                }
            } else {
                (x_mm, y_mm, false)
            }
        }
        // Rectangle / Rounded-Rectangle second click — pin the
        // opposite corner from typed width/height. Each axis is
        // independent: typed width fixes |Δx| (sign from the
        // cursor's quadrant), typed height fixes |Δy|; an
        // untyped axis follows the cursor. The per-tool commit
        // arm builds the box from `first` + this corner (and,
        // for Rounded-Rect, reads the corner radius itself).
        (_, _, SketchTool::Rectangle, ToolPending::RectangleFirst { first })
        | (_, _, SketchTool::RoundedRectangle, ToolPending::RoundedRectangleFirst { first })
            if rect_w_typed.is_some() || rect_h_typed.is_some() =>
        {
            let primitive = editor.primitive();
            if let Some((fx, fy)) = resolve_point_xy(first, primitive) {
                // Sign of the cursor offset picks the quadrant
                // the box grows into; default +1 when the cursor
                // sits exactly on a corner axis.
                let sx = if x_mm < fx { -1.0 } else { 1.0 };
                let sy = if y_mm < fy { -1.0 } else { 1.0 };
                let ex = match rect_w_typed {
                    Some(w) if w > 0.0 => fx + sx * w,
                    _ => x_mm,
                };
                let ey = match rect_h_typed {
                    Some(h) if h > 0.0 => fy + sy * h,
                    _ => y_mm,
                };
                (ex, ey, true)
            } else {
                (x_mm, y_mm, false)
            }
        }
        _ => (x_mm, y_mm, false),
    }
}

// Resolve `effective_snap_id` into the click's entity id: an existing
// snap Point (with, for the Point tool, an Auto-Coincident constraint —
// v0.22 Phase A1), or a freshly-minted Point at `(eff_x_mm, eff_y_mm)`.
// Multi-click tools deliberately keep shared-ID semantics (see the
// caller) — their endpoint ID is the bake's vertex identity and
// switching to constraint-merged points would silently break the
// closed-loop walker.
fn resolve_click_point(
    editor: &mut crate::app::FootprintEditorState,
    plane_id: PlaneId,
    eff_x_mm: f64,
    eff_y_mm: f64,
    effective_snap_id: Option<SketchEntityId>,
    construction_mode: bool,
    centerline_mode: bool,
) -> SketchEntityId {
    use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
    use crate::library::editor::footprint::sketch_mode::SketchEdit;
    use crate::library::editor::footprint::state::SketchTool;
    use signex_sketch::entity::EntityKind;

    let flag = |mut e: Entity| -> Entity {
        e.construction = construction_mode;
        e.centerline = centerline_mode;
        e
    };

    match effective_snap_id {
        Some(target) if matches!(editor.state.active_tool, SketchTool::Point) => {
            use signex_sketch::constraint::{Constraint, ConstraintKind};
            use signex_sketch::id::ConstraintId;

            let new_id = SketchEntityId::new();
            let entity = flag(Entity::new(
                new_id,
                plane_id,
                EntityKind::Point {
                    x: eff_x_mm,
                    y: eff_y_mm,
                },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(entity));
            });
            let constraint = Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Coincident {
                    p1: new_id,
                    p2: target,
                },
            };
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::AddConstraint(constraint),
                );
            });
            new_id
        }
        Some(id) => id,
        None => {
            let id = SketchEntityId::new();
            let entity = flag(Entity::new(
                id,
                plane_id,
                EntityKind::Point {
                    x: eff_x_mm,
                    y: eff_y_mm,
                },
            ));
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(entity));
            });
            id
        }
    }
}

// v0.23 — RepickPolarCenter intercept. Triggered by the Pattern
// sub-form's "Re-pick centre" button. The next click on a Point
// overwrites the array's `center`, independent of the active tool.
// `resolved_id` is either an existing Point (when snap hit) or a
// freshly-minted Point at the click location. Returns `true` when the
// click was consumed by the intercept (caller must skip the per-tool
// dispatch below).
fn try_consume_repick_polar_center(
    editor: &mut crate::app::FootprintEditorState,
    resolved_id: SketchEntityId,
) -> bool {
    use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
    use crate::library::editor::footprint::sketch_mode::SketchEdit;
    use crate::library::editor::footprint::state::ToolPending;

    let ToolPending::RepickPolarCenter { array_id } = editor.state.tool_pending else {
        return false;
    };
    if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
        if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
            if let signex_sketch::array::ArrayKind::Polar { center, .. } = &mut array.kind {
                *center = resolved_id;
            }
        }
    }
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.state.tool_pending = ToolPending::Idle;
    true
}
