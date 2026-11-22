//! What drives a baked pad, for the "Driven by sketch" section of its
//! Properties (mockup row 192, item D5): the source geometry and, on a
//! patterned pad, which instance it is.

use signex_sketch::SketchData;
use signex_sketch::array::ArrayKind;
use signex_sketch::entity::EntityKind;

use super::FootprintEditorState;
use super::pad_link::driving_profile;

/// The kind of pattern a pad is an instance of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadPatternKind {
    Linear,
    Grid,
    Circular,
    Mirror,
}

/// A pad's place in its pattern, counted from 1. `row` is the second index
/// of a grid or of a pattern that repeats another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadInstance {
    pub kind: PadPatternKind,
    pub index: u32,
    pub row: Option<u32>,
}

/// The sketch geometry behind a pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadSketchSource {
    /// Edges of the profile that drives it; 0 for a bare point.
    pub edges: usize,
    /// Its pattern instance; `None` for a pad no pattern places.
    pub instance: Option<PadInstance>,
}

impl FootprintEditorState {
    /// What drives pad `pad_idx` in `sketch`; `None` for a pad with no
    /// sketch geometry behind it.
    pub fn pad_sketch_source(
        &self,
        sketch: Option<&SketchData>,
        pad_idx: usize,
    ) -> Option<PadSketchSource> {
        let sketch = sketch?;
        let pad = self.pads.get(pad_idx)?;
        let key = self
            .last_solve
            .as_ref()
            .and_then(|solve| signex_bake::instance_at(sketch, solve, pad.position_mm));
        let entity = pad
            .sketch_entity_id
            .or_else(|| sketch.innermost_source(key?.0))?;
        let edges = driving_profile(sketch, entity, pad.corner_entity_ids)
            .into_iter()
            .filter(|id| {
                sketch
                    .entities
                    .iter()
                    .any(|e| e.id == *id && matches!(e.kind, EntityKind::Line { .. }))
            })
            .count();
        let instance = key.map(|(pattern, i, j)| {
            let listed = sketch.arrays.iter().find(|a| a.id == pattern);
            let kind = match listed.map(|a| &a.kind) {
                Some(ArrayKind::Linear { .. }) => PadPatternKind::Linear,
                Some(ArrayKind::Grid { .. }) => PadPatternKind::Grid,
                Some(ArrayKind::Polar { .. }) => PadPatternKind::Circular,
                None => PadPatternKind::Mirror,
            };
            let two_indices =
                kind == PadPatternKind::Grid || sketch.pattern_source_of(pattern).is_some();
            PadInstance {
                kind,
                index: i + 1,
                row: two_indices.then_some(j + 1),
            }
        });
        Some(PadSketchSource { edges, instance })
    }
}

#[cfg(test)]
mod tests {
    use signex_library::{Footprint, Pad};
    use signex_sketch::Solver;
    use signex_sketch::array::{Array, ArrayId, NumberingScheme};
    use signex_sketch::attr::PadAttr;
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::*;

    /// A pad point at (1, 0) with a linear pattern of three, 2 mm apart.
    fn patterned() -> (FootprintEditorState, SketchData, SketchEntityId) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let id = SketchEntityId::new();
        let mut point = Entity::new(id, plane, EntityKind::Point { x: 1.0, y: 0.0 });
        point.pad = Some(PadAttr {
            number: "1".into(),
            ..PadAttr::default()
        });
        sketch.entities.push(point);
        sketch.arrays.push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Linear {
                source: id,
                count_expr: "3".into(),
                dx_expr: "2mm".into(),
                dy_expr: "0mm".into(),
            },
            numbering: NumberingScheme::default(),
        });
        let mut fp = Footprint::empty("t");
        fp.pads.push(Pad {
            number: "1".into(),
            ..Pad::default()
        });
        fp.sketch = Some(sketch.clone());
        let mut state = FootprintEditorState::empty();
        state.refresh_pads_from_primitive(&fp);
        let params = resolve(&sketch.parameters).unwrap();
        state.last_solve = Some(Solver::default().solve(&sketch, &params).unwrap());
        (state, sketch, id)
    }

    #[test]
    fn a_patterned_pad_names_its_instance() {
        let (mut state, sketch, _) = patterned();
        state.pads[0].position_mm = (5.0, 0.0);
        state.pads[0].sketch_entity_id = None;
        let source = state.pad_sketch_source(Some(&sketch), 0).expect("driven");
        assert_eq!(
            source.instance,
            Some(PadInstance {
                kind: PadPatternKind::Linear,
                index: 3,
                row: None,
            })
        );
    }

    #[test]
    fn a_pad_with_no_sketch_geometry_has_no_source() {
        let (mut state, sketch, _) = patterned();
        state.pads[0].position_mm = (40.0, 40.0);
        state.pads[0].sketch_entity_id = None;
        assert_eq!(state.pad_sketch_source(Some(&sketch), 0), None);
        assert_eq!(state.pad_sketch_source(None, 0), None);
    }
}
