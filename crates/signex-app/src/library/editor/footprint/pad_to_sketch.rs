//! Mint sketch entities for literal pads — first foundational step
//! toward bidirectional sketch ↔ pads sync.
//!
//! When the user enters Sketch mode for a footprint that has literal
//! pads (created in Pads mode) but no sketch entities yet, this
//! module auto-creates a `Point` + `PadAttr` for every pad. The
//! resulting sketch bakes back into the same pad set, so the
//! round-trip is identity-preserving.
//!
//! Future work (v0.15 bidirectional sync):
//! - Pads-mode edits (move / resize / delete) mirror into the
//!   backing sketch entity.
//! - Drag a sketch Point in Sketch mode → pad position updates.
//! - Editing a pad's `PadAttr` from the Properties panel updates
//!   the matching sketch entity.

use signex_library::primitive::footprint::{
    Footprint, PadKind as LibPadKind, PadShape as LibPadShape,
};
use signex_sketch::attr::{
    ChamferedCorners as SkChamferedCorners, CustomPadShape, PadAttr, PadKind as SkPadKind,
    PadShape as SkPadShape, PadSide, PasteAperturePattern,
};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

use super::state::EditorPad;

/// When the user transitions into Sketch mode for the first time on
/// a footprint that has literal pads but an empty sketch, mint a
/// `Point` + `PadAttr` for each pad. Writes the minted sketch entity
/// IDs back into each `EditorPad.sketch_entity_id` so subsequent
/// Pads-mode edits can mirror through the link. Returns the number
/// of entities minted (zero if the sketch already had content or no
/// literal pads existed).
///
/// The minted sketch produces the same pad set when re-baked through
/// `signex_bake::bake_pads`, so the bake immediately after this call
/// re-emits the original pads — no visual change for the user, but
/// every pad now has a sketch backing they can edit.
pub fn auto_mint_for_literal_pads(pads: &mut [EditorPad], footprint: &mut Footprint) -> usize {
    if pads.is_empty() {
        return 0;
    }
    // Skip if the sketch already has any non-construction entities —
    // assume the user has already started authoring sketch content.
    if let Some(sketch) = footprint.sketch.as_ref() {
        let has_real_entity = sketch.entities.iter().any(|e| !e.construction);
        if has_real_entity {
            return 0;
        }
    }

    let plane_id = ensure_board_top_plane(footprint);
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);

    let mut minted = 0usize;
    for pad in pads.iter_mut() {
        let entity_id = SketchEntityId::new();
        let mut entity = Entity::new(
            entity_id,
            plane_id,
            EntityKind::Point {
                x: pad.position_mm.0,
                y: pad.position_mm.1,
            },
        );
        entity.pad = Some(pad_attr_from_editor_pad(pad));
        sketch.entities.push(entity);
        // v0.15 — link the editor pad to its backing sketch entity.
        pad.sketch_entity_id = Some(entity_id);
        // v0.16 — also mint 4 outline-corner Points + 4 Lines as
        // construction so the user sees the pad outline as
        // primitives in Sketch mode. `bake_pads` ignores construction
        // entities so this stays purely visual.
        let corners = mint_pad_corner_outline(sketch, plane_id, pad);
        pad.corner_entity_ids = Some(corners);
        minted += 1;
    }
    minted
}

/// v0.15 — when a pad is added in Pads mode (canvas click, etc.),
/// mirror the new pad into the sketch as a `Point` + `PadAttr`.
/// Stores the minted sketch entity ID back on the editor pad so
/// later moves / deletes can mirror through.
///
/// v0.24 Track A — when the pad shape is `Round`, also mint a
/// dedicated `Circle` entity referencing the centre Point and a
/// `diameter_<slug>` sketch parameter. `pad.shape_params` records
/// `"diameter" → param_name` so the Properties row (A2) can find
/// the bound parameter on later edits. Other shapes keep the v0.16
/// 4-Line bbox outline.
pub fn mirror_add_pad_to_sketch(pad: &mut EditorPad, footprint: &mut Footprint) {
    // No-op when the sketch already has a backing entity for this
    // pad (e.g. caller already wired it up).
    if pad.sketch_entity_id.is_some() {
        return;
    }
    let plane_id = ensure_board_top_plane(footprint);
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
    let entity_id = SketchEntityId::new();
    let mut entity = Entity::new(
        entity_id,
        plane_id,
        EntityKind::Point {
            x: pad.position_mm.0,
            y: pad.position_mm.1,
        },
    );
    entity.pad = Some(pad_attr_from_editor_pad(pad));
    sketch.entities.push(entity);
    pad.sketch_entity_id = Some(entity_id);

    // v0.24 Track A — branch on pad shape. Round mints a Circle +
    // `diameter_<slug>` parameter; other shapes keep the v0.16
    // 4-Line bbox outline.
    match &pad.shape {
        LibPadShape::Round => {
            mint_round_pad_geometry(sketch, plane_id, pad, entity_id);
            // Round pads have no rectangular outline — leave
            // corner_entity_ids unset so move/delete mirrors skip
            // bbox-corner repositioning.
            pad.corner_entity_ids = None;
        }
        _ => {
            // v0.16 — outline-corner Points + Lines, construction-only.
            let corners = mint_pad_corner_outline(sketch, plane_id, pad);
            pad.corner_entity_ids = Some(corners);
        }
    }
}

/// v0.15 — when a pad moves in Pads mode (drag), update its backing
/// sketch `Point`'s coordinates so the sketch stays in sync. No-op
/// when the pad has no backing sketch entity yet.
pub fn mirror_move_pad_in_sketch(pad: &EditorPad, footprint: &mut Footprint) {
    let Some(entity_id) = pad.sketch_entity_id else {
        return;
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return;
    };
    if let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == entity_id) {
        if let EntityKind::Point { x, y } = &mut entity.kind {
            *x = pad.position_mm.0;
            *y = pad.position_mm.1;
        }
    }
    // v0.16 — also reposition the outline-corner Points so the
    // construction outline tracks the pad bbox.
    if let Some(corners) = pad.corner_entity_ids {
        let bbox = pad.bbox_mm();
        let positions: [(f64, f64); 4] = [
            (bbox.2, bbox.1), // ne
            (bbox.2, bbox.3), // se
            (bbox.0, bbox.3), // sw
            (bbox.0, bbox.1), // nw
        ];
        for (id, (px, py)) in corners.iter().zip(positions.iter()) {
            if let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == *id) {
                if let EntityKind::Point { x, y } = &mut entity.kind {
                    *x = *px;
                    *y = *py;
                }
            }
        }
    }
}

/// v0.15 — when a pad is deleted in Pads mode, also drop its
/// backing sketch entity (and any constraints that referenced it).
/// No-op when the pad has no backing sketch entity yet.
///
/// v0.24 Track A — also drop linked Circle entities (Round pads)
/// and any sketch parameters keyed by the centre-Point UUID slug
/// (`diameter_<slug>`, `corner_r_<slug>`, etc.).
pub fn mirror_delete_pad_from_sketch(pad: &EditorPad, footprint: &mut Footprint) {
    let Some(entity_id) = pad.sketch_entity_id else {
        return;
    };
    let Some(sketch) = footprint.sketch.as_mut() else {
        return;
    };
    // v0.16 — collect the corner-outline entity IDs so we can drop
    // the construction Points + the Lines connecting them. Lines
    // reference the corner Points by ID; we drop any Line whose
    // start or end is one of the dropped corner IDs.
    let mut to_drop: Vec<SketchEntityId> = vec![entity_id];
    if let Some(corners) = pad.corner_entity_ids {
        to_drop.extend_from_slice(&corners);
    }
    let drop_set: std::collections::HashSet<SketchEntityId> = to_drop.iter().copied().collect();
    sketch.entities.retain(|e| {
        if drop_set.contains(&e.id) {
            return false;
        }
        match &e.kind {
            EntityKind::Line { start, end } => {
                if drop_set.contains(start) || drop_set.contains(end) {
                    return false;
                }
            }
            EntityKind::Circle { center, .. } => {
                // v0.24 Track A — drop Round pad's Circle when its
                // centre Point is in the drop set.
                if drop_set.contains(center) {
                    return false;
                }
            }
            EntityKind::Arc { .. } | EntityKind::Point { .. } => {}
        }
        true
    });
    // Drop dangling constraint refs — coarse rule via Debug
    // stringification (mirrors the SketchEdit::DeleteEntity path in
    // sketch_dispatch.rs).
    let id_str = entity_id.to_string();
    sketch
        .constraints
        .retain(|c| !format!("{:?}", c.kind).contains(&id_str));

    // v0.24 Track A — drop shape parameters (`diameter_<slug>`,
    // `corner_r_<slug>`, etc.) keyed by the centre-Point UUID slug.
    let slug = id_slug(entity_id);
    sketch.parameters.0.retain(|name, _| !name.ends_with(&slug));
}

/// v0.16 — mint 4 corner Points + 4 Lines outlining a pad's bbox.
/// Returns the corner IDs in `[ne, se, sw, nw]` order so the caller
/// can store them on `EditorPad.corner_entity_ids` and reposition
/// them on later pad moves. Both the corner Points and the Lines
/// connecting them are flagged `construction = true` so
/// `signex_bake::bake_pads` skips them and they don't double up the
/// rendered pad geometry.
fn mint_pad_corner_outline(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &EditorPad,
) -> [SketchEntityId; 4] {
    let bbox = pad.bbox_mm();
    let positions: [(f64, f64); 4] = [
        (bbox.2, bbox.1), // ne
        (bbox.2, bbox.3), // se
        (bbox.0, bbox.3), // sw
        (bbox.0, bbox.1), // nw
    ];
    let ids: [SketchEntityId; 4] = [
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    ];
    for (id, (x, y)) in ids.iter().zip(positions.iter()) {
        let mut e = Entity::new(*id, plane_id, EntityKind::Point { x: *x, y: *y });
        e.construction = true;
        sketch.entities.push(e);
    }
    // 4 Lines around the loop — N (ne→nw), W (nw→sw), S (sw→se),
    // E (se→ne). Construction-only.
    for (a, b) in [
        (ids[0], ids[3]),
        (ids[3], ids[2]),
        (ids[2], ids[1]),
        (ids[1], ids[0]),
    ] {
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Line { start: a, end: b },
        );
        line.construction = true;
        sketch.entities.push(line);
    }
    ids
}

/// v0.24 Track A — mint a Round pad's geometry: 1 Circle entity
/// referencing the centre `Point` (the pad's `sketch_entity_id`) +
/// a `diameter_<slug>` sketch parameter recording the literal
/// diameter for later parametric edits. The Properties row (A2)
/// reads this parameter via `pad.shape_params["diameter"]`.
fn mint_round_pad_geometry(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &mut EditorPad,
    centre_id: SketchEntityId,
) {
    // Round pad's diameter equals its W (and H — it's a circle, so
    // size_mm.0 == size_mm.1 by definition). The Circle entity stores
    // the radius literal so the bake produces correct geometry; the
    // parameter records the diameter for the Properties-row link.
    let diameter = pad.size_mm.0;
    let radius = diameter / 2.0;
    let circle = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Circle {
            center: centre_id,
            radius,
        },
    );
    sketch.entities.push(circle);

    let slug = id_slug(centre_id);
    let param_name = format!("diameter_{slug}");
    sketch
        .parameters
        .insert(param_name.clone(), format!("{}mm", format_f64(diameter)));
    pad.shape_params.insert("diameter".into(), param_name);
}

/// v0.24 Track A — UUID slug for parameter-name namespacing. Strips
/// dashes so the resulting parameter name is a valid identifier in
/// the expression language.
fn id_slug(id: SketchEntityId) -> String {
    id.0.simple().to_string()
}

fn ensure_board_top_plane(footprint: &mut Footprint) -> PlaneId {
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
    if let Some(p) = sketch
        .planes
        .iter()
        .find(|p| matches!(p.kind, PlaneKind::BoardTop))
    {
        return p.id;
    }
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let id = p.id;
    sketch.planes.push(p);
    id
}

fn pad_attr_from_editor_pad(pad: &EditorPad) -> PadAttr {
    use signex_library::primitive::footprint::PadKind as LibPadKind;
    // v0.18.12.1 — carry `drill_diameter_mm` into the sketch
    // PadAttr. Without this, NPT-hole pads minted via Place Hole
    // lose their drill on the first sketch round-trip (the bake
    // emits `Pad::drill = None`). Plated/NPT semantics follow the
    // pad kind.
    let drill = pad
        .drill_diameter_mm
        .map(|d| signex_sketch::attr::DrillSpec {
            diameter_expr: format!("{}mm", format_f64(d)),
            slot_length_expr: None,
            plated: !matches!(pad.kind, LibPadKind::NptHole),
        });
    PadAttr {
        number: pad.number.clone(),
        kind: map_kind(pad.kind),
        side: map_side(pad),
        shape: map_shape(&pad.shape),
        size_x_expr: format!("{}mm", format_f64(pad.size_mm.0)),
        size_y_expr: format!("{}mm", format_f64(pad.size_mm.1)),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill,
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    }
}

fn map_kind(k: LibPadKind) -> SkPadKind {
    match k {
        LibPadKind::Smd => SkPadKind::Smd,
        LibPadKind::Tht => SkPadKind::Tht,
        LibPadKind::NptHole => SkPadKind::NptHole,
        LibPadKind::ConnectorPad => SkPadKind::ConnectorPad,
        LibPadKind::Castellated => SkPadKind::Castellated,
        LibPadKind::Fiducial => SkPadKind::Fiducial,
        // Future-proof the non_exhaustive lib enum.
        _ => SkPadKind::Smd,
    }
}

fn map_side(pad: &EditorPad) -> PadSide {
    use crate::library::editor::footprint::layers::FpLayer;
    let primary = pad.primary_layer();
    match primary {
        FpLayer::FCu | FpLayer::FFab | FpLayer::FSilks => PadSide::Top,
        FpLayer::BCu | FpLayer::BFab | FpLayer::BSilks => PadSide::Bottom,
        _ => PadSide::All,
    }
}

fn map_shape(s: &LibPadShape) -> SkPadShape {
    match s {
        LibPadShape::Round => SkPadShape::Round,
        LibPadShape::Rect => SkPadShape::Rect,
        LibPadShape::Oval => SkPadShape::Oval,
        LibPadShape::RoundRect { radius_ratio } => SkPadShape::RoundRect {
            radius_ratio_expr: format_f64(*radius_ratio),
        },
        LibPadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => SkPadShape::Chamfered {
            chamfer_ratio_expr: format_f64(*chamfer_ratio),
            corners: SkChamferedCorners {
                top_left: corners.top_left,
                top_right: corners.top_right,
                bottom_left: corners.bottom_left,
                bottom_right: corners.bottom_right,
            },
        },
        LibPadShape::Custom(poly) => {
            // Convert lib's free-form polygon into a sketch
            // CustomPadShape::StaticPoints — sketch-profile bake
            // (closed-loop walker) is not used here since literal
            // pads don't have a sketch profile to walk.
            SkPadShape::Custom(CustomPadShape::StaticPoints {
                points: poly.points.clone(),
            })
        }
    }
}

/// Format a float with up to 4 fractional digits, trimming trailing
/// zeros. Keeps the generated expression strings readable
/// (e.g. `1.5` rather than `1.5000000000000`).
fn format_f64(v: f64) -> String {
    let s = format!("{v:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_pad(number: &str, x: f64, y: f64) -> EditorPad {
        let mut p = EditorPad::new_default(number.into(), (x, y));
        p.size_mm = (1.0, 0.5);
        p
    }

    #[test]
    fn empty_pads_mint_nothing() {
        let mut fp = Footprint::empty("test");
        let mut pads: Vec<EditorPad> = Vec::new();
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 0);
        assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
    }

    #[test]
    fn three_pads_mint_three_points_with_pad_attrs() {
        let mut fp = Footprint::empty("test");
        let mut pads = vec![
            editor_pad("1", 0.0, 0.0),
            editor_pad("2", 1.27, 0.0),
            editor_pad("3", 2.54, 0.0),
        ];
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 3);
        let sketch = fp.sketch.as_ref().unwrap();
        // 1 plane.
        assert_eq!(sketch.planes.len(), 1);
        // v0.16 — per pad: 1 centre Point + 4 corner Points + 4
        // outline Lines = 9 entities. 3 pads × 9 = 27.
        assert_eq!(sketch.entities.len(), 27);
        // The 3 PadAttr-carrying centres should still match v0.15
        // expectations.
        let attr_carriers: Vec<&Entity> =
            sketch.entities.iter().filter(|e| e.pad.is_some()).collect();
        assert_eq!(attr_carriers.len(), 3);
        for entity in attr_carriers {
            assert!(matches!(entity.kind, EntityKind::Point { .. }));
            assert!(!entity.construction);
            let attr = entity.pad.as_ref().unwrap();
            assert!(!attr.number.is_empty());
            assert_eq!(attr.size_x_expr, "1mm");
            assert_eq!(attr.size_y_expr, "0.5mm");
        }
        // v0.15: every pad should now carry the minted entity ID.
        for pad in &pads {
            assert!(pad.sketch_entity_id.is_some());
            assert!(pad.corner_entity_ids.is_some());
        }
    }

    #[test]
    fn skip_when_sketch_already_has_entities() {
        let mut fp = Footprint::empty("test");
        // Pre-populate sketch with one non-construction entity.
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        sketch.planes.push(plane.clone());
        sketch.entities.push(Entity::new(
            SketchEntityId::new(),
            plane.id,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        fp.sketch = Some(sketch);

        let mut pads = vec![editor_pad("1", 0.0, 0.0)];
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 0, "auto-mint must skip when sketch is already populated");
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 1);
        assert!(
            pads[0].sketch_entity_id.is_none(),
            "skip leaves the link unset"
        );
    }

    #[test]
    fn skip_when_sketch_only_has_construction_entities() {
        // Construction-only sketches are still treated as "no real
        // user authoring", so auto-mint should fire.
        let mut fp = Footprint::empty("test");
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let plane_id = plane.id;
        sketch.planes.push(plane);
        let mut construction = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Point { x: 0.0, y: 0.0 },
        );
        construction.construction = true;
        sketch.entities.push(construction);
        fp.sketch = Some(sketch);

        let mut pads = vec![editor_pad("1", 0.0, 0.0)];
        let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
        assert_eq!(n, 1);
        // v0.16 — pre-existing construction entity (1) + minted
        // centre (1) + 4 corner Points + 4 outline Lines = 10.
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 10);
        assert!(pads[0].sketch_entity_id.is_some());
        assert!(pads[0].corner_entity_ids.is_some());
    }

    #[test]
    fn mirror_add_pad_links_to_new_sketch_entity() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 5.0, 5.0);
        assert!(pad.sketch_entity_id.is_none());
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        let id = pad.sketch_entity_id.expect("mirror should mint id");
        let sketch = fp.sketch.as_ref().unwrap();
        let entity = sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .expect("entity exists");
        match entity.kind {
            EntityKind::Point { x, y } => {
                assert_eq!((x, y), (5.0, 5.0));
            }
            _ => panic!("minted entity must be a Point"),
        }
        assert!(entity.pad.is_some(), "Point should carry PadAttr");
    }

    #[test]
    fn mirror_add_pad_with_existing_link_is_noop() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 0.0, 0.0);
        pad.sketch_entity_id = Some(SketchEntityId::new());
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        // Sketch should not have been touched.
        assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
    }

    #[test]
    fn mirror_move_pad_updates_sketch_point() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 0.0, 0.0);
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        // Now move the pad.
        pad.position_mm = (3.5, 7.25);
        mirror_move_pad_in_sketch(&pad, &mut fp);
        let id = pad.sketch_entity_id.unwrap();
        let entity = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        match entity.kind {
            EntityKind::Point { x, y } => assert_eq!((x, y), (3.5, 7.25)),
            _ => panic!("entity must still be a Point"),
        }
    }

    #[test]
    fn mirror_delete_pad_drops_sketch_entity() {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("X", 0.0, 0.0);
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        // v0.16 — 1 centre + 4 corners + 4 lines = 9.
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 9);
        mirror_delete_pad_from_sketch(&pad, &mut fp);
        // Drop the centre + corners + outline lines that referenced
        // the dropped corners → 0 left.
        assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 0);
    }

    #[test]
    fn format_f64_trims_trailing_zeros() {
        assert_eq!(format_f64(1.0), "1");
        assert_eq!(format_f64(1.5), "1.5");
        assert_eq!(format_f64(0.25), "0.25");
        assert_eq!(format_f64(1.27), "1.27");
        assert_eq!(format_f64(0.0), "0");
    }

    #[test]
    fn shape_change_preserves_corner_positions() {
        // v0.22 Phase D3 — verifying that flipping a pad's shape
        // (Rect → Oval, etc.) leaves the corner-outline Points
        // untouched. The corners track the pad's bbox, which is
        // derived from position + size only — shape isn't an input,
        // so no re-mint or re-position is needed on shape change.
        //
        // v0.24 Track A note: Round / RoundRect now mint
        // shape-specific geometry (Circle / Arcs) instead of the
        // v0.16 bbox outline, so this test exercises Rect → Oval —
        // both of which still mint the 4-Point bbox outline. Round /
        // RoundRect get their own dedicated regression coverage in
        // `crates/signex-app/tests/regression.rs`.
        use crate::library::editor::footprint::state::FootprintEditorState;

        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("1", 0.0, 0.0);
        pad.shape = LibPadShape::Rect;
        mirror_add_pad_to_sketch(&mut pad, &mut fp);

        let corner_ids = pad.corner_entity_ids.expect("corners minted");
        let snapshot_corner_pos = |fp: &Footprint| -> Vec<(f64, f64)> {
            corner_ids
                .iter()
                .map(|id| {
                    let entity = fp
                        .sketch
                        .as_ref()
                        .unwrap()
                        .entities
                        .iter()
                        .find(|e| e.id == *id)
                        .expect("corner Point present");
                    match entity.kind {
                        EntityKind::Point { x, y } => (x, y),
                        _ => panic!("corner must be Point"),
                    }
                })
                .collect()
        };

        let before = snapshot_corner_pos(&fp);

        // Flip the shape — emulating a Properties-panel shape change.
        // Pads-mode dispatch paths call `with_selected_pad` which
        // ultimately calls `sync_pads_to_primitive`; that path does
        // NOT touch corner positions because shape is bbox-orthogonal.
        pad.shape = LibPadShape::Oval;
        let mut s = FootprintEditorState::empty();
        s.pads = vec![pad.clone()];
        FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);

        let after = snapshot_corner_pos(&fp);

        assert_eq!(
            before, after,
            "corner positions must remain stable across shape changes"
        );
    }
}
