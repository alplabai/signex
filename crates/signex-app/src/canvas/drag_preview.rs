//! The ghost shown while a selection is dragged or floated (click-click
//! Move): the selection shifted by the cursor delta and, when "Keep wires
//! connected when moving" is on, the attached wires rubber-banded exactly as
//! the engine will commit them (#963).

use super::drag_layers::{DragFrame, moved_item_ids, split_ghost};
use crate::schematic_runtime::SchematicRenderSnapshot;
use signex_types::schematic::SelectedItem;

/// The drag delta after grid snapping: the PRIMARY selected item's
/// connection point (its stored `position`) lands on a grid dot after the
/// move, not just the delta. Snapping only the delta would preserve an
/// off-grid origin; users expect the endpoint to be on-grid. The commit and
/// the drag ghost both call this, so the preview shows exactly the
/// topology the drop will produce.
///
/// With `hotspot_radius` set (electrical grid on, #1246) a moved pin tip or
/// wire end that would land within that radius of a non-moved hotspot is
/// pulled exactly onto it, after the grid snap.
pub(crate) fn snapped_move_delta(
    snap: Option<&SchematicRenderSnapshot>,
    selected: &[SelectedItem],
    dx: f64,
    dy: f64,
    snap_enabled: bool,
    grid_mm: f64,
    hotspot_radius: Option<f64>,
) -> (f64, f64) {
    let (dx, dy) = if snap_enabled {
        let primary = selected
            .first()
            .and_then(|item| snap.and_then(|snap| primary_anchor_world(snap, item)));
        match primary {
            Some((px, py)) => {
                let snapped_x = ((px + dx) / grid_mm).round() * grid_mm;
                let snapped_y = ((py + dy) / grid_mm).round() * grid_mm;
                (snapped_x - px, snapped_y - py)
            }
            None => (
                (dx / grid_mm).round() * grid_mm,
                (dy / grid_mm).round() * grid_mm,
            ),
        }
    } else {
        (dx, dy)
    };
    match (snap, hotspot_radius) {
        (Some(sheet), Some(radius)) if !selected.is_empty() => {
            let points = moved_connection_points(sheet, selected);
            let ids: Vec<_> = moved_item_ids(sheet, selected).into_iter().collect();
            signex_types::schematic::snap_move_to_hotspot(sheet, &points, &ids, (dx, dy), radius)
        }
        _ => (dx, dy),
    }
}

/// The points of the selected items that make electrical connections: every
/// pin tip of a symbol, both ends of a wire, the anchor of anything else.
fn moved_connection_points(
    sheet: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
) -> Vec<signex_types::schematic::Point> {
    use signex_types::schematic::{Point, SelectedKind, SymbolTransform, instance_pins};
    let mut points = Vec::new();
    for item in selected {
        match item.kind {
            SelectedKind::Symbol => {
                let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) else {
                    continue;
                };
                let Some(lib) = sheet.resolve_lib_symbol(sym) else {
                    continue;
                };
                let xform = SymbolTransform::from_symbol(sym);
                points.extend(instance_pins(&lib, sym).map(|lp| xform.apply(lp.pin.position)));
            }
            SelectedKind::Wire => {
                if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                    points.extend([w.start, w.end]);
                }
            }
            _ => points.extend(primary_anchor_world(sheet, item).map(|(x, y)| Point::new(x, y))),
        }
    }
    points
}

/// Resolve a selected item's primary anchor — the world point that should
/// snap to the grid (connection point for labels/wires/symbols, etc.).
fn primary_anchor_world(
    snap: &crate::schematic_runtime::SchematicRenderSnapshot,
    item: &signex_types::schematic::SelectedItem,
) -> Option<(f64, f64)> {
    use signex_types::schematic::SelectedKind;
    match item.kind {
        SelectedKind::Label => snap
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)
            .map(|l| (l.position.x, l.position.y)),
        SelectedKind::Symbol => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .map(|s| (s.position.x, s.position.y)),
        SelectedKind::Wire => snap
            .wires
            .iter()
            .find(|w| w.uuid == item.uuid)
            .map(|w| (w.start.x, w.start.y)),
        SelectedKind::Bus => snap
            .buses
            .iter()
            .find(|b| b.uuid == item.uuid)
            .map(|b| (b.start.x, b.start.y)),
        SelectedKind::Junction => snap
            .junctions
            .iter()
            .find(|j| j.uuid == item.uuid)
            .map(|j| (j.position.x, j.position.y)),
        SelectedKind::NoConnect => snap
            .no_connects
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(|n| (n.position.x, n.position.y)),
        SelectedKind::NoErcDirective => snap
            .no_erc_directives
            .iter()
            .find(|d| d.uuid == item.uuid)
            .map(|d| (d.position.x, d.position.y)),
        SelectedKind::TextNote => snap
            .text_notes
            .iter()
            .find(|t| t.uuid == item.uuid)
            .map(|t| (t.position.x, t.position.y)),
        SelectedKind::ChildSheet => snap
            .child_sheets
            .iter()
            .find(|c| c.uuid == item.uuid)
            .map(|c| (c.position.x, c.position.y)),
        SelectedKind::SheetPin => snap
            .child_sheets
            .iter()
            .find_map(|c| c.pins.iter().find(|pin| pin.uuid == item.uuid))
            .map(|pin| (pin.position.x, pin.position.y)),
        SelectedKind::SymbolRefField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.ref_text.as_ref().map(|rt| (rt.position.x, rt.position.y))),
        SelectedKind::SymbolValField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.val_text.as_ref().map(|vt| (vt.position.x, vt.position.y))),
        _ => None,
    }
}

/// Most selected items the rubber-banded ghost is computed for; beyond that
/// the plain shift stands in, because the pass costs about a netlist build
/// per round and a drag must stay fluid.
pub(super) const MAX_RUBBER_BAND_GHOST_ITEMS: usize = 60;

/// With snap off the ghost moves in steps of this many screen pixels, so the
/// pass does not rerun for every pixel of cursor travel.
const GHOST_STEP_PX: f64 = 2.0;

/// The delta the ghost is computed at. With snap on the delta already puts
/// the selection on the grid and stays exact, so the ghost is the sheet the
/// commit makes; with snap off it is rounded to [`GHOST_STEP_PX`] at the
/// current zoom (`scale` in px per mm). The trade-off: with snap off the
/// ghost can sit up to half a step (±1/3 mm at 100 % zoom) from the exact
/// delta the commit uses, so it may show a route, or miss a fallback, that
/// the commit does not.
pub(super) fn ghost_delta(dx: f64, dy: f64, snap_enabled: bool, scale: f32) -> (f64, f64) {
    if snap_enabled || scale <= 0.0 || scale.is_nan() {
        return (dx, dy);
    }
    let step = GHOST_STEP_PX / f64::from(scale);
    ((dx / step).round() * step, (dy / step).round() * step)
}

/// What a ghost was computed from. The render revision changes with every
/// edit, undo and redo (even when the snapshot is updated in place), and the
/// item kind keeps a symbol and its reference field apart.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GhostKey {
    revision: u64,
    selection: Vec<SelectedItem>,
    delta: (u64, u64),
}

#[derive(Debug)]
struct DragGhost {
    key: GhostKey,
    snapshot: Rc<SchematicRenderSnapshot>,
    /// What the ghost changes against the source sheet, found when the ghost
    /// is computed so a frame only draws that (#1335).
    frame: DragFrame,
}

/// The rubber-banded drag ghost and the pre-move nets it is built against,
/// kept in the canvas `Program::State`: `update` fills them when the cursor
/// moves, `draw` only reads. The pre-move reference is built once per drag,
/// not once per cursor step.
#[derive(Debug, Default)]
pub struct GhostState {
    session: Option<(u64, Rc<signex_engine::MovePreview>)>,
    ghost: Option<DragGhost>,
}

fn key_of(revision: u64, selection: &[SelectedItem], dx: f64, dy: f64) -> GhostKey {
    GhostKey {
        revision,
        selection: selection.to_vec(),
        delta: (dx.to_bits(), dy.to_bits()),
    }
}

impl GhostState {
    fn ready_ghost(
        &self,
        revision: u64,
        selection: &[SelectedItem],
        dx: f64,
        dy: f64,
    ) -> Option<&DragGhost> {
        let key = key_of(revision, selection, dx, dy);
        self.ghost.as_ref().filter(|ghost| ghost.key == key)
    }

    /// The ghost already computed for exactly this document revision,
    /// selection and delta (see [`ghost_delta`]).
    #[cfg(test)]
    pub(super) fn ready(
        &self,
        revision: u64,
        selection: &[SelectedItem],
        dx: f64,
        dy: f64,
    ) -> Option<Rc<SchematicRenderSnapshot>> {
        self.ready_ghost(revision, selection, dx, dy)
            .map(|ghost| ghost.snapshot.clone())
    }

    /// What the ready ghost changes: the moving items to draw and the ids the
    /// static layer leaves out.
    pub(super) fn ready_frame(
        &self,
        revision: u64,
        selection: &[SelectedItem],
        dx: f64,
        dy: f64,
    ) -> Option<DragFrame> {
        self.ready_ghost(revision, selection, dx, dy)
            .map(|ghost| ghost.frame.clone())
    }

    /// Compute the rubber-banded ghost for `selection` moved by `(dx, dy)`:
    /// the very sheet the engine would commit at that delta (see
    /// [`signex_engine::MovePreview`]). Callers pass [`ghost_delta`].
    pub(super) fn refresh(
        &mut self,
        snap: &SchematicRenderSnapshot,
        revision: u64,
        selection: &[SelectedItem],
        dx: f64,
        dy: f64,
    ) -> Rc<SchematicRenderSnapshot> {
        if let Some(ready) = self.ready_ghost(revision, selection, dx, dy) {
            return ready.snapshot.clone();
        }
        let session = match &self.session {
            Some((rev, session)) if *rev == revision => session.clone(),
            _ => {
                let fresh = Rc::new(signex_engine::MovePreview::new(snap));
                self.session = Some((revision, fresh.clone()));
                fresh
            }
        };
        let key = key_of(revision, selection, dx, dy);
        let moves: Vec<_> = selection.iter().map(|item| (*item, dx, dy)).collect();
        let snapshot = Rc::new(session.moved(&moves, true));
        let frame = split_ghost(snap, &snapshot, &moved_item_ids(snap, selection));
        self.ghost = Some(DragGhost {
            key,
            snapshot: snapshot.clone(),
            frame,
        });
        snapshot
    }

    /// Drop the ghost and the pre-move reference (a drag ended).
    pub(super) fn clear(&mut self) {
        self.session = None;
        self.ghost = None;
    }
}

use std::rc::Rc;

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedKind,
        Symbol, Wire,
    };

    fn sheet_with_wired_resistor() -> (SchematicSheet, SelectedItem) {
        let mut sheet = SchematicSheet::default();
        let pin = LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(-5.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        };
        sheet.lib_symbols.insert(
            "R".to_string(),
            LibSymbol {
                id: "R".to_string(),
                reference: String::new(),
                value: String::new(),
                footprint: String::new(),
                datasheet: String::new(),
                description: String::new(),
                keywords: String::new(),
                fp_filters: String::new(),
                in_bom: true,
                on_board: true,
                in_pos_files: true,
                duplicate_pin_numbers_are_jumpers: false,
                graphics: Vec::new(),
                pins: vec![pin],
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
        let uuid = uuid::Uuid::now_v7();
        sheet.symbols.push(Symbol {
            uuid,
            lib_id: "R".to_string(),
            position: Point::new(50.0, 50.0),
            ..Default::default()
        });
        sheet.wires.push(Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(30.0, 50.0),
            end: Point::new(45.0, 50.0),
            ..Default::default()
        });
        (sheet, SelectedItem::new(uuid, SelectedKind::Symbol))
    }

    #[test]
    fn the_snapped_delta_puts_the_primary_anchor_on_the_grid() {
        let (sheet, item) = sheet_with_wired_resistor();
        // Symbol origin (50, 50) on a 1.27 grid: 50 / 1.27 is not integral.
        let (dx, dy) = snapped_move_delta(Some(&sheet), &[item], 0.3, 3.1, true, 1.27, None);
        let (x, y) = (50.0 + dx, 50.0 + dy);
        for v in [x, y] {
            let cells = v / 1.27;
            assert!((cells - cells.round()).abs() < 1e-9, "{v} is off-grid");
        }
        assert_eq!(
            snapped_move_delta(Some(&sheet), &[], 0.3, 3.1, false, 1.27, None),
            (0.3, 3.1),
            "snap off leaves the delta alone"
        );
    }

    #[test]
    fn the_ghost_rubber_bands_the_wires_when_the_preference_is_on() {
        let (sheet, item) = sheet_with_wired_resistor();
        let mut state = GhostState::default();
        let ghost = state.refresh(&sheet, 1, &[item], 0.0, 10.0);
        assert_eq!(ghost.symbols[0].position, Point::new(50.0, 60.0));
        assert_eq!(ghost.wires.len(), 2, "a bend wire reaches the new pin tip");
        assert!(
            ghost
                .wires
                .iter()
                .any(|w| w.end == Point::new(45.0, 60.0) || w.start == Point::new(45.0, 60.0)),
            "{:?}",
            ghost.wires
        );
        assert_eq!(sheet.wires.len(), 1, "the source snapshot is untouched");
    }

    #[test]
    fn the_ghost_is_exactly_what_the_commit_leaves() {
        let (sheet, item) = sheet_with_wired_resistor();
        let mut state = GhostState::default();
        let ghost = state.refresh(&sheet, 1, &[item], 2.5, 10.0);
        let mut engine = signex_engine::Engine::new(sheet).expect("engine");
        engine
            .execute(signex_engine::Command::MoveSelection {
                items: vec![item],
                dx: 2.5,
                dy: 10.0,
                keep_wires: true,
            })
            .expect("move");
        let committed: Vec<_> = engine
            .document()
            .wires
            .iter()
            .map(|w| (w.start, w.end))
            .collect();
        let shown: Vec<_> = ghost.wires.iter().map(|w| (w.start, w.end)).collect();
        assert_eq!(shown, committed);
    }

    #[test]
    fn an_unchanged_delta_reuses_the_ghost() {
        let (sheet, item) = sheet_with_wired_resistor();
        let mut state = GhostState::default();
        let first = state.refresh(&sheet, 1, &[item], 0.0, 10.0);
        let again = state.refresh(&sheet, 1, &[item], 0.0, 10.0);
        assert!(Rc::ptr_eq(&first, &again));
        let moved = state.refresh(&sheet, 1, &[item], 0.0, 12.5);
        assert!(!Rc::ptr_eq(&first, &moved));
    }

    /// Snap off: cursor travel under one step (2 px, so 2/3 mm at 100 % zoom)
    /// keeps the ghost delta and the pass does not rerun. Snap on: the delta
    /// that puts the anchor on the grid is kept exact, never rounded off it.
    #[test]
    fn the_ghost_delta_steps_by_screen_pixels_only_with_snap_off() {
        let zoom = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE;
        let step = GHOST_STEP_PX / f64::from(zoom);
        let at = |dy: f64| ghost_delta(0.0, dy, false, zoom).1;
        assert_eq!(at(9.0 * step), at(9.3 * step), "sub-step travel");
        assert_ne!(at(9.0 * step), at(10.0 * step), "one step");
        let fine = ghost_delta(0.0, 9.3 * step, false, zoom * 16.0).1;
        assert_ne!(fine, at(9.3 * step), "zoomed in, the step is finer");
        assert_eq!(ghost_delta(0.3, 1.27, true, zoom), (0.3, 1.27));
    }

    /// An edit, undo or redo updates the snapshot in place and bumps the
    /// revision; a ghost from before it must not answer for after it.
    #[test]
    fn a_new_revision_or_item_kind_never_gets_a_stale_ghost() {
        let (sheet, item) = sheet_with_wired_resistor();
        let mut state = GhostState::default();
        let first = state.refresh(&sheet, 1, &[item], 0.0, 10.0);
        assert!(state.ready(2, &[item], 0.0, 10.0).is_none(), "revision");
        let field = SelectedItem::new(item.uuid, SelectedKind::SymbolRefField);
        assert!(state.ready(1, &[field], 0.0, 10.0).is_none(), "kind");

        // The committed sheet, synced in place under a new revision.
        let mut moved = sheet.clone();
        moved.symbols[0].position = Point::new(50.0, 60.0);
        let second = state.refresh(&moved, 2, &[item], 0.0, 10.0);
        assert!(!Rc::ptr_eq(&first, &second));
        assert_eq!(second.symbols[0].position, Point::new(50.0, 70.0));
        assert_eq!(first.symbols[0].position, Point::new(50.0, 60.0));
    }

    /// The review's probe, with the real render cache: build a ghost, commit
    /// the same move, sync the cache in place, drag at the same delta again -
    /// and again after an undo. The ghost must be of the sheet on screen.
    #[test]
    fn a_second_drag_at_the_same_delta_after_a_commit_or_undo_is_recomputed() {
        use crate::schematic_runtime::{RenderInvalidation, SchematicRenderCache};
        let (sheet, item) = sheet_with_wired_resistor();
        let mut engine = signex_engine::Engine::new(sheet).expect("engine");
        let mut cache = SchematicRenderCache::from_sheet_at(engine.document(), engine.revision());
        let mut ghosts = GhostState::default();
        let y_of = |g: &SchematicRenderSnapshot| g.symbols[0].position.y;

        let first = ghosts.refresh(cache.snapshot(), cache.revision(), &[item], 0.0, 10.0);
        assert_eq!(y_of(&first), 60.0);

        engine
            .execute(signex_engine::Command::MoveSelection {
                items: vec![item],
                dx: 0.0,
                dy: 10.0,
                keep_wires: true,
            })
            .expect("commit");
        cache.update_from_sheet(
            engine.document(),
            RenderInvalidation::FULL,
            engine.base_revision(),
            engine.revision(),
        );
        assert!(ghosts.ready(cache.revision(), &[item], 0.0, 10.0).is_none());
        let second = ghosts.refresh(cache.snapshot(), cache.revision(), &[item], 0.0, 10.0);
        assert_eq!(y_of(&second), 70.0, "the symbol is at 60 now; +10 is 70");

        engine.undo().expect("undo");
        cache.update_from_sheet(
            engine.document(),
            RenderInvalidation::FULL,
            engine.base_revision(),
            engine.revision(),
        );
        assert!(ghosts.ready(cache.revision(), &[item], 0.0, 10.0).is_none());
        let third = ghosts.refresh(cache.snapshot(), cache.revision(), &[item], 0.0, 10.0);
        assert_eq!(y_of(&third), 60.0, "after undo the symbol is back at 50");
    }

    #[test]
    fn the_ghost_leaves_wires_alone_when_the_preference_is_off() {
        let (sheet, item) = sheet_with_wired_resistor();
        let ghost = std::rc::Rc::new(super::super::shift_snapshot_for_selection(
            &sheet,
            &[item],
            0.0,
            10.0,
        ));
        assert_eq!(ghost.symbols[0].position, Point::new(50.0, 60.0));
        assert_eq!(ghost.wires.len(), 1);
        assert_eq!(ghost.wires[0].end, Point::new(45.0, 50.0));
    }
}
