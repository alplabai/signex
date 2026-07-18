//! Footprint editor — geometry update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! The router delegates all geometry `FootprintEditorMsg` variants here;
//! bodies are verbatim, so each arm keeps its own inner `use`s.

use crate::library::editor::footprint::pad_to_sketch;
use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;
use crate::library::messages::FootprintEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: FootprintEditorMsg) {
    match msg {
        // v0.18.7 — append a fresh empty footprint to the envelope
        // and switch onto it. Names the new sibling `Footprint N`
        // where N counts existing siblings + 1; the user can rename
        // via the Properties panel.
        FootprintEditorMsg::AddNewSibling => {
            let next_n = editor.file.footprints.len() + 1;
            let new_fp = signex_library::Footprint::empty(format!("Footprint {next_n}"));
            editor.file.footprints.push(new_fp);
            editor.active_idx = editor.file.footprints.len() - 1;
            editor.state =
                crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                    editor.primitive(),
                );
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::AddPad { x_mm, y_mm } => {
            // v0.15 — bidirectional Pads → Sketch mirror. The new
            // pad gets a backing sketch Point + PadAttr (when the
            // sketch already has any other backed entity, i.e. the
            // user has been in Sketch mode at least once).
            // v0.18.6 — split-borrow at the top of the arm so
            // `state.pads.get_mut(...)` and `primitive` coexist; both
            // halves originate from disjoint editor fields.
            editor.with_parts(|state, primitive| {
                let idx = state.add_pad_at(x_mm, y_mm);
                if let Some(pad) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(pad, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::AddVia { x_mm, y_mm } => {
            // v0.27 — vias are a small Round plated-through pad. The
            // canonical via geometry is fixed (0.6 mm copper / 0.3 mm
            // drill / Multi-Layer F.Cu+B.Cu+masks) so the user gets a
            // proper via regardless of what `next_pad_defaults` looks
            // like. Bypasses `add_pad_at` (which inherits Pads-mode
            // defaults) and constructs the EditorPad directly.
            use crate::library::editor::footprint::state::EditorPad;
            use signex_library::{LayerId, PadKind, PadShape};
            const VIA_DIAMETER_MM: f64 = 0.6;
            const VIA_DRILL_MM: f64 = 0.3;
            editor.with_parts(|state, primitive| {
                let number = state.next_pad_number();
                let mut pad = EditorPad::new_default(number, (x_mm, y_mm));
                pad.size_mm = (VIA_DIAMETER_MM, VIA_DIAMETER_MM);
                pad.shape = PadShape::Round;
                pad.kind = PadKind::Tht;
                pad.drill_diameter_mm = Some(VIA_DRILL_MM);
                pad.layers = vec![
                    LayerId::new("F.Cu"),
                    LayerId::new("F.Mask"),
                    LayerId::new("B.Cu"),
                    LayerId::new("B.Mask"),
                ];
                state.pads.push(pad);
                let idx = state.pads.len() - 1;
                state.selected_pad = Some(idx);
                if let Some(p) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(p, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.18.15.1 — Place Track 2-click gesture. First click
        // stashes the start in `state.track_first`; second click
        // commits the line to silk_f and chains by re-stashing the
        // second click as the next gesture's start.
        FootprintEditorMsg::TrackClick { x_mm, y_mm } => {
            match editor.state.track_first {
                None => {
                    editor.state.track_first = Some((x_mm, y_mm));
                }
                Some((sx, sy)) => {
                    let primitive = editor.primitive_mut();
                    primitive
                        .silk_f
                        .push(signex_library::primitive::footprint::FpGraphic {
                            kind: signex_library::primitive::footprint::FpGraphicKind::Line {
                                from: [sx, sy],
                                to: [x_mm, y_mm],
                            },
                            stroke_width: 0.15,
                            filled: false,
                        });
                    // Chain — the second click becomes the next
                    // segment's start, matching Altium's stroke-a-
                    // polyline workflow.
                    editor.state.track_first = Some((x_mm, y_mm));
                    editor.dirty = true;
                }
            }
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::TrackCancel => {
            editor.state.track_first = None;
            editor.canvas_cache.clear();
        }
        // v0.18.15.3 — Place Arc 3-click gesture (centre / radius
        // start / sweep end). Idle → Center → Start → commit. After
        // commit the gesture resets to Idle (no chain — arcs
        // typically aren't strung together).
        FootprintEditorMsg::ArcClick { x_mm, y_mm } => {
            use crate::library::editor::footprint::state::PlaceArcPending;
            let next = match editor.state.place_arc_pending {
                PlaceArcPending::Idle => PlaceArcPending::Center {
                    center: (x_mm, y_mm),
                },
                PlaceArcPending::Center { center } => PlaceArcPending::Start {
                    center,
                    start: (x_mm, y_mm),
                },
                PlaceArcPending::Start { center, start } => {
                    let (cx, cy) = center;
                    let (sx, sy) = start;
                    let radius = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                    if radius > 1e-6 {
                        let start_deg = (sy - cy).atan2(sx - cx).to_degrees();
                        let end_deg = (y_mm - cy).atan2(x_mm - cx).to_degrees();
                        let primitive = editor.primitive_mut();
                        primitive
                            .silk_f
                            .push(signex_library::primitive::footprint::FpGraphic {
                                kind: signex_library::primitive::footprint::FpGraphicKind::Arc {
                                    center: [cx, cy],
                                    radius,
                                    start_deg,
                                    end_deg,
                                },
                                stroke_width: 0.15,
                                filled: false,
                            });
                        editor.dirty = true;
                    }
                    PlaceArcPending::Idle
                }
            };
            editor.state.place_arc_pending = next;
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::ArcCancel => {
            editor.state.place_arc_pending =
                crate::library::editor::footprint::state::PlaceArcPending::Idle;
            editor.canvas_cache.clear();
        }
        // v0.18.15.4 — Place Polygon multi-click gesture. Each
        // click appends a vertex; commit happens on tool switch /
        // Esc via `FootprintPolygonCommit`.
        FootprintEditorMsg::PolygonClick { x_mm, y_mm } => {
            editor.state.place_polygon_vertices.push((x_mm, y_mm));
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::PolygonCommit => {
            let verts = std::mem::take(&mut editor.state.place_polygon_vertices);
            // v0.18.17 — emit one `Polygon` FpGraphic (instead of
            // N Lines). `filled` follows the active tool —
            // `PlacePolygon` = stroked outline, `PlaceRegion` =
            // solid fill.
            let filled = matches!(
                editor.state.pads_tool,
                crate::library::editor::footprint::state::PadsTool::PlaceRegion
            );
            if verts.len() >= 3 {
                let vertices: Vec<[f64; 2]> = verts.iter().map(|(x, y)| [*x, *y]).collect();
                let primitive = editor.primitive_mut();
                primitive
                    .silk_f
                    .push(signex_library::primitive::footprint::FpGraphic {
                        kind: signex_library::primitive::footprint::FpGraphicKind::Polygon {
                            vertices,
                        },
                        stroke_width: if filled { 0.0 } else { 0.15 },
                        filled,
                    });
                editor.dirty = true;
            }
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::PolygonCancel => {
            editor.state.place_polygon_vertices.clear();
            editor.canvas_cache.clear();
        }
        // v0.18.15 — Place String tool. Appends a silk-layer text
        // label `FpGraphic { kind: Text { position, content: "TEXT",
        // size: 1.0 }, stroke_width: 0.0 }` to the active footprint's
        // `silk_f`. The user edits the content via the Properties
        // panel later (Properties wiring is queued).
        FootprintEditorMsg::AddText { x_mm, y_mm } => {
            let primitive = editor.primitive_mut();
            primitive
                .silk_f
                .push(signex_library::primitive::footprint::FpGraphic {
                    kind: signex_library::primitive::footprint::FpGraphicKind::Text {
                        position: [x_mm, y_mm],
                        content: "TEXT".to_string(),
                        size: 1.0,
                        frame: None,
                    },
                    stroke_width: 0.0,
                    filled: false,
                });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.14 — Place Text Frame press-drag-release commit (item
        // ③). Fires once, on release, with the anchor (min corner)
        // and drag size already resolved by the canvas. Pushes its
        // own history snapshot — see `mutates_footprint_state`,
        // classified alongside the 3D Body mint variants — because
        // the intermediate press/drag ticks never reach the
        // dispatcher (unlike Track's 2-click gesture), so there's
        // no risk of double-stacking.
        FootprintEditorMsg::AddTextFrame {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        } => {
            editor.push_history();
            editor.with_parts(|_state, primitive| {
                crate::library::editor::footprint::text_frame::add_text_frame(
                    primitive, x_mm, y_mm, w_mm, h_mm,
                );
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.18.12 — Place Hole tool. Drops a non-plated through
        // hole at the cursor (no copper, drill from `next_pad_defaults`).
        FootprintEditorMsg::AddHole { x_mm, y_mm } => {
            editor.with_parts(|state, primitive| {
                let idx = state.add_hole_at(x_mm, y_mm);
                if let Some(pad) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(pad, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }

        FootprintEditorMsg::MintBody3d => {
            editor.push_history();
            editor.with_parts(|_state, primitive| {
                crate::library::editor::footprint::body3d_mint::mint_box_from_courtyard(primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::MintExtrudedBody3d => {
            editor.push_history();
            editor.with_parts(|_state, primitive| {
                crate::library::editor::footprint::body3d_mint::mint_extruded_from_fab(primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        _ => unreachable!("non-geometry variant routed to geometry::apply"),
    }
}

/// v0.15 — gate the Pads → Sketch mirror on whether the footprint already
/// has a sketch (i.e. the user has visited Sketch mode at least once OR
/// auto-mint has already fired). Mirroring into a non-existent sketch would
/// create one silently, which is undesirable for users who only ever work
/// in Pads mode.
fn footprint_sketch_is_active(fp: &signex_library::primitive::footprint::Footprint) -> bool {
    match fp.sketch.as_ref() {
        Some(s) => !s.entities.is_empty(),
        None => false,
    }
}
