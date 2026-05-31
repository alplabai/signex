//! Phase 8.1 — QFN-16 end-to-end author smoke.
//!
//! Programmatically builds a SketchData representing one row of a
//! QFN-16 footprint (4 pads on the east side, 0.5 mm pitch), runs the
//! solve-on-edit dispatcher, asserts the baked Pad coordinates match
//! to within 1 µm, then mutates `pad_pitch` to 0.65 mm and re-asserts
//! the regenerated coordinates.
//!
//! Drives the entire v0.13 stack: parameter resolution → expression
//! evaluation → LM solver → DOF analysis → pad bake → sketch ↔ library
//! integration. No UI required.

use chrono::Utc;
use signex_app::library::editor::footprint::sketch_dispatch::apply_sketch_edit;
use signex_app::library::editor::footprint::sketch_mode::SketchEdit;
use signex_app::library::editor::footprint::state::FootprintEditorState;
use signex_library::primitive::footprint::Footprint;
use signex_sketch::SketchData;
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

const PITCH_05: f64 = 0.5;
const PITCH_065: f64 = 0.65;
const ROW_X: f64 = 2.0;

/// Author one row of QFN pads, parameterised on `pad_pitch`. Pads
/// are at `(ROW_X, +1.5 * pad_pitch)`, `(ROW_X, +0.5 * pad_pitch)`,
/// `(ROW_X, -0.5 * pad_pitch)`, `(ROW_X, -1.5 * pad_pitch)`. Pitch
/// resolution flows via the parameter table.
fn build_qfn_row(pad_pitch_expr: &str) -> (Footprint, Vec<SketchEntityId>) {
    let mut fp = Footprint::empty("QFN-16-row");
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    sketch.parameters.insert("pad_pitch", pad_pitch_expr);

    // Each pad's centre is `(ROW_X, idx * pad_pitch)`. Underlying
    // sketch Point sits at the row anchor `(ROW_X, 0)`; the
    // offset_y_expr carries the per-pad delta. Doing it this way
    // keeps the Point coords pad_pitch-independent so the only thing
    // that needs re-resolving when pad_pitch changes is the offset
    // expression — exercised by the second test.
    let mut ids = Vec::new();
    for (idx, label) in [(1.5, "1"), (0.5, "2"), (-0.5, "3"), (-1.5, "4")] {
        let id = SketchEntityId::new();
        let e = {
            let mut e = Entity::new(id, plane, EntityKind::Point { x: ROW_X, y: 0.0 });
            e.pad = Some(PadAttr {
                number: label.into(),
                kind: PadKind::Smd,
                side: PadSide::Top,
                shape: PadShape::Rect,
                size_x_expr: "0.6mm".into(),
                size_y_expr: "0.3mm".into(),
                rotation_expr: None,
                offset_x_expr: None,
                offset_y_expr: Some(format!("{} * pad_pitch", idx)),
                drill: None,
                mask_margin_expr: None,
                paste_margin_expr: None,
                paste_apertures: PasteAperturePattern::Single,
                ..PadAttr::default()
            });
            e
        };
        sketch.entities.push(e);
        ids.push(id);
    }

    fp.sketch = Some(sketch);
    (fp, ids)
}

#[test]
fn qfn16_row_bakes_at_05mm_pitch() {
    let (mut fp, _ids) = build_qfn_row("0.5mm");
    let mut state = FootprintEditorState::from_footprint(&fp);

    apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).unwrap();

    assert_eq!(fp.pads.len(), 4, "QFN-16 row should bake 4 pads");

    // Expected y-positions at 0.5 mm pitch:
    //   pad 1 → +0.75, pad 2 → +0.25, pad 3 → -0.25, pad 4 → -0.75.
    let mut by_number: std::collections::HashMap<&str, [f64; 2]> = std::collections::HashMap::new();
    for pad in &fp.pads {
        by_number.insert(Box::leak(pad.number.clone().into_boxed_str()), pad.position);
    }

    let expected = [
        ("1", [ROW_X, 1.5 * PITCH_05]),
        ("2", [ROW_X, 0.5 * PITCH_05]),
        ("3", [ROW_X, -0.5 * PITCH_05]),
        ("4", [ROW_X, -1.5 * PITCH_05]),
    ];
    for (label, [ex, ey]) in expected {
        let got = by_number.get(label).expect("pad missing");
        assert!(
            (got[0] - ex).abs() < 1e-6,
            "pad {label} x: got {} expected {}",
            got[0],
            ex
        );
        assert!(
            (got[1] - ey).abs() < 1e-6,
            "pad {label} y: got {} expected {}",
            got[1],
            ey
        );
    }
}

#[test]
fn qfn16_row_regenerates_when_pad_pitch_changes() {
    // Start at 0.5 mm pitch.
    let (mut fp, _ids) = build_qfn_row("0.5mm");
    let mut state = FootprintEditorState::from_footprint(&fp);
    apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).unwrap();
    assert_eq!(fp.pads.len(), 4);

    // Now bump pitch to 0.65 mm via parameter edit. The dispatcher
    // re-runs resolve + solve + bake, so the pads at indices ±0.5
    // and ±1.5 multiply through to new positions.
    apply_sketch_edit(
        &mut state,
        &mut fp,
        SketchEdit::EditParameter {
            name: "pad_pitch".into(),
            expr: "0.65mm".into(),
        },
    )
    .unwrap();

    let by_number: std::collections::HashMap<String, [f64; 2]> = fp
        .pads
        .iter()
        .map(|p| (p.number.clone(), p.position))
        .collect();

    let expected = [
        ("1", [ROW_X, 1.5 * PITCH_065]),
        ("2", [ROW_X, 0.5 * PITCH_065]),
        ("3", [ROW_X, -0.5 * PITCH_065]),
        ("4", [ROW_X, -1.5 * PITCH_065]),
    ];
    for (label, [ex, ey]) in expected {
        let got = by_number.get(label).expect("pad missing");
        assert!(
            (got[0] - ex).abs() < 1e-6,
            "after re-pitch, pad {label} x: got {} expected {}",
            got[0],
            ex
        );
        assert!(
            (got[1] - ey).abs() < 1e-6,
            "after re-pitch, pad {label} y: got {} expected {}",
            got[1],
            ey
        );
    }
}

#[test]
fn qfn16_solve_warnings_empty_on_clean_sketch() {
    let (mut fp, _ids) = build_qfn_row("0.5mm");
    let mut state = FootprintEditorState::from_footprint(&fp);
    apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).unwrap();

    let unrelated_warnings: Vec<&String> = state
        .solve_warnings
        .iter()
        .filter(|w| {
            !w.contains("ignored") && !w.contains("deferred") && !w.contains("approximated")
        })
        .collect();
    assert!(
        unrelated_warnings.is_empty(),
        "expected no error warnings, got {:?}",
        unrelated_warnings
    );
    assert!(state.last_solve.is_some());
}

// Unused-import linter sanity: tie the chrono pull-in to this stub
// so an `cargo check` doesn't strip it (Footprint::empty needs Utc::now()).
#[allow(dead_code)]
fn _link_chrono() -> chrono::DateTime<Utc> {
    Utc::now()
}
