//! What drives a baked pad (mockup row 192, items D1 and D5): the source
//! geometry, on a patterned pad which instance it is and which attributes
//! that instance overrides; and the pad outlines the sketch canvas draws
//! for the pad link.

use signex_sketch::SketchData;
use signex_sketch::array::{ArrayId, ArrayKind};
use signex_sketch::attr::PadStackOverrides;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_types::sketch::SketchHost;

use super::pad_link::{PadLinkOffer, driving_profile};
use super::{EditorMode, FootprintEditorState};

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
    /// The attributes this instance holds apart from its pattern's source.
    pub overrides: InstanceOverrides,
}

/// A pad attribute as the Attribute scope table lists it (mockup row 192,
/// item D1): shared by the pattern's source, or overridden on one instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadAttribute {
    Designator,
    Locked,
    ElectricalType,
    Template,
    Features,
    Testpoint,
    Paste,
    SolderMask,
}

impl PadAttribute {
    /// Every attribute, in the table's order.
    pub const ALL: [Self; 8] = [
        Self::Designator,
        Self::Locked,
        Self::ElectricalType,
        Self::Template,
        Self::Features,
        Self::Testpoint,
        Self::Paste,
        Self::SolderMask,
    ];

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The set of [`PadAttribute`]s one pattern instance overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstanceOverrides(u8);

impl InstanceOverrides {
    /// Whether the instance holds `attr` apart from its source.
    pub fn contains(self, attr: PadAttribute) -> bool {
        self.0 & attr.bit() != 0
    }

    /// This set with `attr` added when `is_overridden`.
    pub fn with(self, attr: PadAttribute, is_overridden: bool) -> Self {
        if is_overridden {
            Self(self.0 | attr.bit())
        } else {
            self
        }
    }
}

/// A pad outline the sketch canvas draws for the pad link (mockup row
/// 192): the pad the focused entity drives solid, the pattern instance
/// the focus came from dashed. Corners in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadLinkOutline {
    pub corners: [(f64, f64); 4],
    pub is_instance: bool,
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
                overrides: instance_overrides(sketch, (pattern, i, j)),
            }
        });
        Some(PadSketchSource { edges, instance })
    }

    /// What the sketch canvas lights for the pad link (mockup row 192): the
    /// entity whose profile it highlights, and the pad outlines it draws.
    /// `None` when the link offers nothing.
    pub fn pad_link_highlight(&self) -> Option<(SketchEntityId, Vec<PadLinkOutline>)> {
        let offer = self.pad_link_offer()?;
        let entity = offer.entity();
        let source_pad = self.pad_driven_by(entity);
        let instance_pad = match offer {
            PadLinkOffer::BackToPad { pad_idx, .. } => {
                Some(pad_idx).filter(|i| Some(*i) != source_pad)
            }
            PadLinkOffer::JumpToPad { .. } => None,
        };
        let outline = |idx: usize, is_instance: bool| {
            self.pads.get(idx).map(|pad| PadLinkOutline {
                corners: pad.rotated_corners_mm(),
                is_instance,
            })
        };
        let outlines = source_pad
            .and_then(|idx| outline(idx, false))
            .into_iter()
            .chain(instance_pad.and_then(|idx| outline(idx, true)))
            .collect();
        Some((entity, outlines))
    }

    /// The pad the floating Edit in Sketch link on the Footprint canvas
    /// offers (mockup row 192): the selected pad, when sketch geometry
    /// drives it. A board sketch has no pads.
    pub fn edit_in_sketch_offer(&self, sketch: Option<&SketchData>) -> Option<usize> {
        if self.mode != EditorMode::Normal || self.sketch_host() == SketchHost::Board {
            return None;
        }
        let idx = self.selected_pad?;
        self.pad_sketch_source(sketch, idx).map(|_| idx)
    }
}

/// The attributes instance `key` of a pattern overrides apart from the
/// pattern's source pad (mockup row 192, item D1).
fn instance_overrides(sketch: &SketchData, key: (ArrayId, u32, u32)) -> InstanceOverrides {
    let (pattern, i, j) = key;
    let Some(held) = sketch.override_for(pattern, i, j) else {
        return InstanceOverrides::default();
    };
    let source = sketch.innermost_source(pattern);
    let base = sketch
        .entities
        .iter()
        .find(|e| Some(e.id) == source)
        .and_then(|e| e.pad.clone())
        .unwrap_or_default();
    let paste = |s: &PadStackOverrides| {
        (
            s.paste_top_expr.clone(),
            s.paste_top_pct,
            s.paste_bottom_expr.clone(),
            s.paste_bottom_pct,
            s.paste_top_enabled,
            s.paste_bottom_enabled,
        )
    };
    let mask = |s: &PadStackOverrides| {
        (
            s.mask_top_expr.clone(),
            s.mask_bottom_expr.clone(),
            s.mask_top_tented,
            s.mask_bottom_tented,
        )
    };
    let stack = held.stack.as_ref();
    InstanceOverrides::default()
        .with(PadAttribute::Designator, held.number.is_some())
        .with(PadAttribute::Locked, held.locked.is_some())
        .with(PadAttribute::ElectricalType, held.electrical_type.is_some())
        .with(
            PadAttribute::Template,
            held.template.is_some() || held.library.is_some(),
        )
        .with(
            PadAttribute::Features,
            held.feature_top.is_some() || held.feature_bottom.is_some(),
        )
        .with(PadAttribute::Testpoint, held.testpoint.is_some())
        .with(
            PadAttribute::Paste,
            held.paste_margin_expr.is_some()
                || stack.is_some_and(|s| paste(s) != paste(&base.stack)),
        )
        .with(
            PadAttribute::SolderMask,
            held.mask_margin_expr.is_some() || stack.is_some_and(|s| mask(s) != mask(&base.stack)),
        )
}

#[cfg(test)]
mod tests {
    use signex_library::{Footprint, Pad};
    use signex_sketch::Solver;
    use signex_sketch::array::{Array, ArrayId, NumberingScheme};
    use signex_sketch::attr::{ElectricalType, PadAttr};
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::sketch::pad_override::PadAttrOverride;

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
                overrides: InstanceOverrides::default(),
            })
        );
    }

    /// Mockup row 192: Edit in Sketch on a pattern instance focuses the
    /// source, outlines that instance, and Back to pad returns to it.
    #[test]
    fn edit_in_sketch_on_an_instance_focuses_its_source_and_outlines_it() {
        let (mut state, sketch, source) = patterned();
        state.pads[0].position_mm = (5.0, 0.0);
        state.pads[0].sketch_entity_id = None;
        state.edit_pad_in_sketch(0, Some(&sketch));
        let focus = state.live_pad_link().expect("the focus holds");
        assert_eq!((focus.entity, focus.is_instance), (source, true));
        assert_eq!(state.selected_sketch, Some(source));
        assert!(state.pad_link_instance_outline().is_some());
        state.return_to_pad(source);
        assert_eq!(state.selected_pad, Some(0));
    }

    /// Mockup row 192, item D1: an override on one instance marks only the
    /// attributes it holds; the rest stay with the pattern's source.
    #[test]
    fn an_instance_override_marks_only_its_attributes() {
        let (mut state, mut sketch, _) = patterned();
        state.pads[0].position_mm = (5.0, 0.0);
        state.pads[0].sketch_entity_id = None;
        let pattern = sketch.arrays[0].id;
        let mut stack = PadAttr::default().stack;
        stack.paste_top_enabled = !stack.paste_top_enabled;
        sketch.set_override(
            pattern,
            2,
            0,
            PadAttrOverride {
                electrical_type: Some(ElectricalType::Source),
                stack: Some(stack),
                ..PadAttrOverride::default()
            },
        );
        let instance = state
            .pad_sketch_source(Some(&sketch), 0)
            .and_then(|source| source.instance)
            .expect("an instance");
        let overridden: Vec<PadAttribute> = PadAttribute::ALL
            .into_iter()
            .filter(|attr| instance.overrides.contains(*attr))
            .collect();
        assert_eq!(
            overridden,
            [PadAttribute::ElectricalType, PadAttribute::Paste]
        );
    }

    #[test]
    fn an_instance_with_no_override_follows_its_source() {
        let (mut state, sketch, _) = patterned();
        state.pads[0].position_mm = (3.0, 0.0);
        state.pads[0].sketch_entity_id = None;
        let instance = state
            .pad_sketch_source(Some(&sketch), 0)
            .and_then(|source| source.instance)
            .expect("an instance");
        assert_eq!(instance.overrides, InstanceOverrides::default());
        assert!(
            PadAttribute::ALL
                .into_iter()
                .all(|attr| !instance.overrides.contains(attr))
        );
    }

    /// Mockup row 192 (capture: only the origin point lit, pad 1 bare): Edit
    /// in Sketch on a pad its own point drives outlines that pad, solid.
    #[test]
    fn edit_in_sketch_on_a_single_pad_outlines_that_pad() {
        let (mut state, sketch, source) = patterned();
        state.pads[0].sketch_entity_id = Some(source);
        state.edit_pad_in_sketch(0, Some(&sketch));
        let (entity, outlines) = state.pad_link_highlight().expect("highlighted");
        assert_eq!(entity, source);
        assert_eq!(
            outlines,
            [PadLinkOutline {
                corners: state.pads[0].rotated_corners_mm(),
                is_instance: false,
            }]
        );
    }

    /// Mockup row 192: from an instance, the source's pad is outlined solid
    /// and the instance dashed.
    #[test]
    fn edit_in_sketch_on_an_instance_outlines_the_source_pad_and_the_instance() {
        let (mut state, sketch, source) = patterned();
        let mut source_pad = state.pads[0].clone();
        source_pad.sketch_entity_id = Some(source);
        source_pad.position_mm = (1.0, 0.0);
        state.pads[0].position_mm = (5.0, 0.0);
        state.pads[0].sketch_entity_id = None;
        state.pads.push(source_pad);
        state.edit_pad_in_sketch(0, Some(&sketch));
        let (entity, outlines) = state.pad_link_highlight().expect("highlighted");
        assert_eq!(entity, source);
        assert_eq!(
            outlines,
            [
                PadLinkOutline {
                    corners: state.pads[1].rotated_corners_mm(),
                    is_instance: false,
                },
                PadLinkOutline {
                    corners: state.pads[0].rotated_corners_mm(),
                    is_instance: true,
                },
            ]
        );
    }

    /// A selected pad entity with no Edit in Sketch focus (Jump to pad)
    /// still lights its profile and outlines its pad.
    #[test]
    fn a_selected_pad_entity_outlines_its_pad() {
        let (mut state, _, source) = patterned();
        state.pads[0].sketch_entity_id = Some(source);
        state.mode = EditorMode::Sketch;
        state.selected_sketch = Some(source);
        let (entity, outlines) = state.pad_link_highlight().expect("highlighted");
        assert_eq!(entity, source);
        assert_eq!(outlines.len(), 1);
        assert!(!outlines[0].is_instance);
        state.selected_sketch = None;
        assert_eq!(state.pad_link_highlight(), None);
    }

    /// Mockup row 192: the floating Edit in Sketch link shows in Footprint
    /// mode for a selected pad the sketch drives, and nowhere else.
    #[test]
    fn the_canvas_offers_edit_in_sketch_for_a_selected_driven_pad() {
        let (mut state, sketch, source) = patterned();
        state.pads[0].sketch_entity_id = Some(source);
        state.mode = EditorMode::Normal;
        assert_eq!(state.edit_in_sketch_offer(Some(&sketch)), None);
        state.selected_pad = Some(0);
        assert_eq!(state.edit_in_sketch_offer(Some(&sketch)), Some(0));
        assert_eq!(state.edit_in_sketch_offer(None), None);
        state.mode = EditorMode::Sketch;
        assert_eq!(state.edit_in_sketch_offer(Some(&sketch)), None);
        state.mode = EditorMode::Normal;
        state.pads[0].position_mm = (40.0, 40.0);
        state.pads[0].sketch_entity_id = None;
        assert_eq!(state.edit_in_sketch_offer(Some(&sketch)), None);
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
