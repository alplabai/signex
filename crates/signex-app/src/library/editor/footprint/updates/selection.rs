//! Footprint editor — selection update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! `apply` is a thin router; each `FootprintEditorMsg` variant delegates
//! to one named per-action fn below (object→action, ADR-0001 D2).

use crate::library::editor::footprint::pad_to_sketch;
use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;
use crate::library::messages::FootprintEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: FootprintEditorMsg) {
    match msg {
        FootprintEditorMsg::SelectActiveIdx(idx) => select_active_idx(editor, idx),
        FootprintEditorMsg::ToggleSelectionFilter(kind) => toggle_selection_filter(editor, kind),
        FootprintEditorMsg::SelectSilkF(sel) => select_silk_f(editor, sel),
        FootprintEditorMsg::DeleteSilkF => delete_silk_f(editor),
        FootprintEditorMsg::MovePad { idx, x_mm, y_mm } => move_pad(editor, idx, x_mm, y_mm),
        FootprintEditorMsg::CursorAt { x_mm, y_mm } => cursor_at(editor, x_mm, y_mm),
        FootprintEditorMsg::SelectPad(sel) => select_pad(editor, sel),
        FootprintEditorMsg::SelectPads(pads) => select_pads(editor, pads),
        FootprintEditorMsg::DeleteSelected => delete_selected(editor),
        FootprintEditorMsg::SetSelectionMode2d(mode) => set_selection_mode_2d(editor, mode),
        FootprintEditorMsg::SelectAllOnLayer => select_all_on_layer(editor),
        FootprintEditorMsg::LassoArm => lasso_arm(editor),
        FootprintEditorMsg::LassoAddVertex { x_mm, y_mm } => lasso_add_vertex(editor, x_mm, y_mm),
        FootprintEditorMsg::LassoCancel => lasso_cancel(editor),
        FootprintEditorMsg::LassoCommit => lasso_commit(editor),
        FootprintEditorMsg::TouchingLineArm => touching_line_arm(editor),
        FootprintEditorMsg::TouchingLineFirst { x_mm, y_mm } => {
            touching_line_first(editor, x_mm, y_mm)
        }
        FootprintEditorMsg::TouchingLineCancel => touching_line_cancel(editor),
        FootprintEditorMsg::TouchingLineCommit { x_mm, y_mm } => {
            touching_line_commit(editor, x_mm, y_mm)
        }
        FootprintEditorMsg::SelectOverlapped | FootprintEditorMsg::SelectNextOverlapped => {
            select_overlapped(editor, &msg)
        }
        FootprintEditorMsg::SelectOffGridPads => select_off_grid_pads(editor),
        _ => unreachable!("non-selection variant routed to selection::apply"),
    }
}

// v0.18.7 — switch the active footprint within the multi-
// footprint envelope. Resets the canvas pad list off the
// newly-active primitive, clears selection, refits the
// camera on the next frame so a different-sized footprint
// doesn't open at a stale zoom.
fn select_active_idx(editor: &mut crate::app::FootprintEditorState, idx: usize) {
    let last = editor.file.footprints.len().saturating_sub(1);
    let clamped = idx.min(last);
    if clamped == editor.active_idx {
        return;
    }
    editor.active_idx = clamped;
    // Re-derive the canvas-side state from the new active
    // primitive so pads / sketch / courtyard mirror what's
    // on disk for this footprint.
    editor.state = crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
        editor.primitive(),
    );
    editor.canvas_cache.clear();
}

// v0.18.14 — Selection Filter pill toggle from the unified
// active bar. The panel-side equivalent
// (`PanelMsg::FpEditorToggleSelectionFilter`) routes through
// a dedicated handler in `handlers/dock/sch_library`; this
// arm covers the active-bar dispatch path.
fn toggle_selection_filter(
    editor: &mut crate::app::FootprintEditorState,
    kind: crate::library::editor::footprint::state::SelectionFilterKind,
) {
    editor.state.selection_filter.toggle(kind);
    editor.canvas_cache.clear();
}

// v0.18.18 — silk-front graphic selection. Clears
// selected_pad symmetrically so the Properties panel
// doesn't try to render two selection-specific bodies at
// once.
fn select_silk_f(editor: &mut crate::app::FootprintEditorState, sel: Option<usize>) {
    editor.state.selected_silk_f = sel;
    if sel.is_some() {
        editor.state.selected_pad = None;
    }
    editor.canvas_cache.clear();
}

// v0.18.18 — delete the selected silk-front graphic.
// No-op when nothing is selected. Updates `editor.dirty`
// and clears the selection state.
fn delete_silk_f(editor: &mut crate::app::FootprintEditorState) {
    if let Some(idx) = editor.state.selected_silk_f {
        let primitive = editor.primitive_mut();
        if idx < primitive.silk_f.len() {
            primitive.silk_f.remove(idx);
            editor.dirty = true;
        }
        // HI-25: shared selection-adjustment helper — keep
        // `selected_silk_f` valid against the new vec length
        // instead of clearing unconditionally.
        editor.state.selected_silk_f =
            crate::library::editor::footprint::state::adjust_selection_after_remove(
                editor.state.selected_silk_f,
                idx,
            );
    }
    editor.canvas_cache.clear();
}

fn move_pad(editor: &mut crate::app::FootprintEditorState, idx: usize, x_mm: f64, y_mm: f64) {
    editor.with_parts(|state, primitive| {
        state.move_pad(idx, x_mm, y_mm);
        // v0.15 — mirror the move into the sketch.
        if let Some(pad) = state.pads.get(idx) {
            pad_to_sketch::mirror_move_pad_in_sketch(pad, primitive);
        }
        CanvasState::sync_pads_to_primitive(state, primitive);
    });
    editor.canvas_cache.clear();
    editor.dirty = true;
}

fn cursor_at(editor: &mut crate::app::FootprintEditorState, x_mm: f64, y_mm: f64) {
    editor.state.cursor_mm = Some((x_mm, y_mm));
}

fn select_pad(editor: &mut crate::app::FootprintEditorState, sel: Option<usize>) {
    editor.state.selected_pad = sel;
    // v0.27 — single-pad select replaces the multi-select
    // extras too. Multi-select uses FootprintSelectPads.
    editor.state.selected_pads_extra.clear();
    // v0.27 — record the click position for Select
    // overlapped / Select next so the dropdown can find
    // the stack at the last-clicked location.
    if sel.is_some() {
        editor.state.last_click_world_mm = editor.state.cursor_mm;
    }
    // v0.18.18 — pad and silk selection are mutually
    // exclusive in the Properties panel; clear the silk
    // selection when a pad is picked.
    if sel.is_some() {
        editor.state.selected_silk_f = None;
    }
    // v0.25 polish — clear verbatim numeric buffers on
    // selection change so a stale "0.1." buffer from one
    // pad doesn't follow the user to the next pad's input.
    editor.state.numeric_buffers.clear();
    editor.canvas_cache.clear();
}

fn select_pads(editor: &mut crate::app::FootprintEditorState, mut pads: Vec<usize>) {
    // v0.27 — Altium-parity multi-select. Empty list = clear.
    // First entry becomes the primary (drives Properties);
    // rest land in `selected_pads_extra` for highlight only.
    // Dedupe so a sloppy caller passing [3, 3, 7] still gets
    // [3, 7] selected.
    pads.sort_unstable();
    pads.dedup();
    if pads.is_empty() {
        editor.state.selected_pad = None;
        editor.state.selected_pads_extra.clear();
    } else {
        editor.state.selected_pad = Some(pads[0]);
        editor.state.selected_pads_extra = pads[1..].to_vec();
        editor.state.selected_silk_f = None;
        editor.state.last_click_world_mm = editor.state.cursor_mm;
    }
    editor.state.numeric_buffers.clear();
    editor.canvas_cache.clear();
}

fn delete_selected(editor: &mut crate::app::FootprintEditorState) {
    // v0.27 — Delete walks the full multi-select set, not
    // just the primary `selected_pad`. Rubber-band + Ctrl-
    // click extras get the same treatment as the primary so
    // pressing Delete after a rubber-band sweep clears the
    // whole region. Sketch-mode entities use the sketch
    // dispatcher so the solver re-converges without dangling
    // constraints.
    use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
    use crate::library::editor::footprint::sketch_mode::SketchEdit;
    use crate::library::editor::footprint::state::EditorMode;

    let did_delete = editor.with_parts(|state, primitive| {
        let mut any = false;

        // Sketch-mode deletion — primary + secondary + extras.
        if state.mode == EditorMode::Sketch {
            use std::collections::HashSet;
            let mut seen: HashSet<signex_sketch::id::SketchEntityId> = HashSet::new();
            let mut victims: Vec<signex_sketch::id::SketchEntityId> = Vec::new();
            let mut push_unique = |id: signex_sketch::id::SketchEntityId,
                                   vs: &mut Vec<signex_sketch::id::SketchEntityId>,
                                   seen: &mut HashSet<_>| {
                if seen.insert(id) {
                    vs.push(id);
                }
            };
            if let Some(id) = state.selected_sketch.take() {
                push_unique(id, &mut victims, &mut seen);
            }
            if let Some(id) = state.selected_sketch_secondary.take() {
                push_unique(id, &mut victims, &mut seen);
            }
            let extras: Vec<_> = state.selected_sketch_extra.drain(..).collect();
            for id in extras {
                push_unique(id, &mut victims, &mut seen);
            }
            for id in victims {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::DeleteEntity(id));
                any = true;
            }
        }

        // Pads (always — rubber-band can also select pads).
        let mut pad_victims: Vec<usize> = Vec::new();
        if let Some(idx) = state.selected_pad {
            pad_victims.push(idx);
        }
        pad_victims.extend(state.selected_pads_extra.iter().copied());
        pad_victims.sort_unstable();
        pad_victims.dedup();
        // Remove highest-index first so earlier indices stay
        // valid through the loop.
        pad_victims.sort_unstable_by(|a, b| b.cmp(a));
        for idx in pad_victims {
            if let Some(pad) = state.pads.get(idx) {
                pad_to_sketch::mirror_delete_pad_from_sketch(pad, primitive);
            }
            state.delete_pad(idx);
            any = true;
        }
        state.selected_pads_extra.clear();
        CanvasState::sync_pads_to_primitive(state, primitive);
        any
    });
    if did_delete {
        editor.canvas_cache.clear();
        editor.dirty = true;
    }
}

fn set_selection_mode_2d(
    editor: &mut crate::app::FootprintEditorState,
    mode: crate::library::editor::footprint::state::FpSelectionMode,
) {
    // v0.27 — active-bar Selection picker rows. The rubber-
    // band release picker reads this on commit so Inside /
    // Touching / Outside semantics apply.
    editor.state.selection_mode_2d = mode;
    editor.state.active_bar_menu = None;
}

fn select_all_on_layer(editor: &mut crate::app::FootprintEditorState) {
    // v0.27 — multi-select every pad on the active layer.
    // Active layer = layer of the currently-selected pad,
    // or F.Cu when nothing is selected.
    let layer = editor
        .state
        .selected_pad
        .and_then(|idx| editor.state.pads.get(idx))
        .map(|p| p.primary_layer())
        .unwrap_or(crate::library::editor::footprint::layers::FpLayer::FCu);
    let mut matches: Vec<usize> = editor
        .state
        .pads
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            if p.primary_layer() == layer {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if matches.is_empty() {
        editor.state.selected_pad = None;
        editor.state.selected_pads_extra.clear();
    } else {
        editor.state.selected_pad = Some(matches.remove(0));
        editor.state.selected_pads_extra = matches;
    }
    editor.state.active_bar_menu = None;
    editor.canvas_cache.clear();
}

fn lasso_arm(editor: &mut crate::app::FootprintEditorState) {
    editor.state.lasso_mode_active = true;
    editor.state.lasso_vertices.clear();
    editor.state.active_bar_menu = None;
    editor.canvas_cache.clear();
}

fn lasso_add_vertex(editor: &mut crate::app::FootprintEditorState, x_mm: f64, y_mm: f64) {
    if editor.state.lasso_mode_active {
        editor.state.lasso_vertices.push((x_mm, y_mm));
        editor.canvas_cache.clear();
    }
}

fn lasso_cancel(editor: &mut crate::app::FootprintEditorState) {
    editor.state.lasso_mode_active = false;
    editor.state.lasso_vertices.clear();
    editor.canvas_cache.clear();
}

fn lasso_commit(editor: &mut crate::app::FootprintEditorState) {
    // v0.27 — close the polygon, multi-select every pad whose
    // centre lies inside (even-odd ray casting). Anything
    // less than three vertices is a degenerate polygon and
    // commits as deselect-all so a stray click doesn't leave
    // the user stuck in lasso mode with no feedback.
    let verts: Vec<(f64, f64)> = std::mem::take(&mut editor.state.lasso_vertices);
    editor.state.lasso_mode_active = false;
    let in_poly = |px: f64, py: f64| -> bool {
        if verts.len() < 3 {
            return false;
        }
        let mut inside = false;
        let n = verts.len();
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = verts[i];
            let (xj, yj) = verts[j];
            let denom = yj - yi;
            if denom.abs() < 1e-10 {
                j = i;
                continue;
            }
            let intersect = ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / denom + xi);
            if intersect {
                inside = !inside;
            }
            j = i;
        }
        inside
    };
    let mut hits: Vec<usize> = editor
        .state
        .pads
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            if in_poly(p.position_mm.0, p.position_mm.1) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if hits.is_empty() {
        editor.state.selected_pad = None;
        editor.state.selected_pads_extra.clear();
    } else {
        editor.state.selected_pad = Some(hits.remove(0));
        editor.state.selected_pads_extra = hits;
    }
    editor.canvas_cache.clear();
}

fn touching_line_arm(editor: &mut crate::app::FootprintEditorState) {
    editor.state.touching_line_active = true;
    editor.state.touching_line_first = None;
    editor.state.active_bar_menu = None;
    editor.canvas_cache.clear();
}

fn touching_line_first(editor: &mut crate::app::FootprintEditorState, x_mm: f64, y_mm: f64) {
    if editor.state.touching_line_active {
        editor.state.touching_line_first = Some((x_mm, y_mm));
        editor.canvas_cache.clear();
    }
}

fn touching_line_cancel(editor: &mut crate::app::FootprintEditorState) {
    editor.state.touching_line_active = false;
    editor.state.touching_line_first = None;
    editor.canvas_cache.clear();
}

fn touching_line_commit(editor: &mut crate::app::FootprintEditorState, x_mm: f64, y_mm: f64) {
    // v0.27 — Touching Line: every pad whose bbox is
    // intersected by the segment from `touching_line_first`
    // → (x_mm, y_mm) becomes selected. Liang-Barsky-style
    // segment-vs-AABB clip.
    let Some((sx, sy)) = editor.state.touching_line_first.take() else {
        editor.state.touching_line_active = false;
        editor.canvas_cache.clear();
        return;
    };
    editor.state.touching_line_active = false;
    let dx = x_mm - sx;
    let dy = y_mm - sy;
    let segment_hits_aabb = |xmin: f64, ymin: f64, xmax: f64, ymax: f64| -> bool {
        // Both endpoints inside?
        let inside = |x: f64, y: f64| -> bool { x >= xmin && x <= xmax && y >= ymin && y <= ymax };
        if inside(sx, sy) || inside(x_mm, y_mm) {
            return true;
        }
        // Liang-Barsky parametric clip in [0, 1].
        let mut t_enter = 0.0_f64;
        let mut t_exit = 1.0_f64;
        let p = [-dx, dx, -dy, dy];
        let q = [sx - xmin, xmax - sx, sy - ymin, ymax - sy];
        for i in 0..4 {
            if p[i].abs() < 1e-12 {
                if q[i] < 0.0 {
                    return false;
                }
            } else {
                let t = q[i] / p[i];
                if p[i] < 0.0 {
                    if t > t_exit {
                        return false;
                    }
                    if t > t_enter {
                        t_enter = t;
                    }
                } else {
                    if t < t_enter {
                        return false;
                    }
                    if t < t_exit {
                        t_exit = t;
                    }
                }
            }
        }
        t_enter <= t_exit
    };
    let mut hits: Vec<usize> = editor
        .state
        .pads
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            // Rotated AABB, same as rubber-band select. Against
            // the un-rotated box a turned pad is scored on
            // copper it does not occupy, and missed on copper it
            // does — the hit-test defect, one message over.
            let (x0, y0, x1, y1) = p.rotated_aabb_mm();
            if segment_hits_aabb(x0, y0, x1, y1) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if hits.is_empty() {
        editor.state.selected_pad = None;
        editor.state.selected_pads_extra.clear();
    } else {
        editor.state.selected_pad = Some(hits.remove(0));
        editor.state.selected_pads_extra = hits;
    }
    editor.canvas_cache.clear();
}

// v0.27 — Cycle through pads stacked at the most recent click world
// position. `SelectOverlapped` goes to the previous pad in z-order;
// `SelectNextOverlapped` advances. Without a recorded click position
// there's no stack to cycle, so the action is a silent no-op.
fn select_overlapped(editor: &mut crate::app::FootprintEditorState, msg: &FootprintEditorMsg) {
    let forward = matches!(msg, FootprintEditorMsg::SelectNextOverlapped);
    let Some((wx, wy)) = editor.state.last_click_world_mm else {
        editor.state.active_bar_menu = None;
        return;
    };
    let stack: Vec<usize> = editor
        .state
        .pads
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            if p.contains_mm(wx, wy) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if stack.is_empty() {
        editor.state.active_bar_menu = None;
        return;
    }
    // Pick next/prev relative to the current primary selection.
    let cur_pos = editor
        .state
        .selected_pad
        .and_then(|s| stack.iter().position(|&i| i == s));
    let next_idx = match cur_pos {
        Some(p) => {
            if forward {
                (p + 1) % stack.len()
            } else {
                (p + stack.len() - 1) % stack.len()
            }
        }
        None => 0,
    };
    editor.state.selected_pad = Some(stack[next_idx]);
    editor.state.selected_pads_extra.clear();
    editor.state.active_bar_menu = None;
    editor.canvas_cache.clear();
}

fn select_off_grid_pads(editor: &mut crate::app::FootprintEditorState) {
    // v0.27 — pads whose centre falls between grid steps.
    // The active grid step lives on snap_options; defaults
    // to 1 mm. Tolerance is 1% of the step so pads exactly
    // on the grid (with floating-point noise) don't
    // false-positive.
    let step = editor.state.snap_options.grid_step_mm.max(1e-6);
    let tol = step * 0.01;
    let on_grid = |v: f64| -> bool {
        let r = (v / step).round() * step;
        (v - r).abs() <= tol
    };
    let mut matches: Vec<usize> = editor
        .state
        .pads
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            let (x, y) = p.position_mm;
            if !on_grid(x) || !on_grid(y) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if matches.is_empty() {
        editor.state.selected_pad = None;
        editor.state.selected_pads_extra.clear();
    } else {
        editor.state.selected_pad = Some(matches.remove(0));
        editor.state.selected_pads_extra = matches;
    }
    editor.state.active_bar_menu = None;
    editor.canvas_cache.clear();
}
