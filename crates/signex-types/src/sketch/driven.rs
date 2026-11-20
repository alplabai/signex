//! Driven (reference) dimensions (mockup row 187, mockup row 188): a dimension the user
//! converted to a reference measures the geometry and never drives it.
//!
//! The set is stored on the sketch as [`SketchData::driven_dimensions`], a
//! list of constraint ids, not as a flag on [`Constraint`]. Only dimension
//! kinds ([`ConstraintKind::dim_target`] is `Some`) can be driven; an id in
//! the list that names anything else is ignored, so a hand-edited file
//! cannot switch a relation off.
//!
//! [`ConstraintKind::dim_target`]: crate::sketch::constraint::ConstraintKind::dim_target

use crate::sketch::constraint::Constraint;
use crate::sketch::data::SketchData;
use crate::sketch::id::ConstraintId;

impl SketchData {
    /// Whether constraint `id` is a dimension that is driven (a reference).
    pub fn is_driven(&self, id: ConstraintId) -> bool {
        self.driven_dimensions.contains(&id)
            && self
                .constraints
                .iter()
                .any(|c| c.id == id && c.kind.dim_target().is_some())
    }

    /// Make dimension `id` driven (a reference) or driving again. Returns
    /// whether the sketch changed. Only a dimension can be made driven;
    /// asking for anything else, or for the state it already has, changes
    /// nothing.
    pub fn set_driven(&mut self, id: ConstraintId, driven: bool) -> bool {
        if !driven {
            let before = self.driven_dimensions.len();
            self.driven_dimensions.retain(|d| *d != id);
            return self.driven_dimensions.len() != before;
        }
        let is_dimension = self
            .constraints
            .iter()
            .any(|c| c.id == id && c.kind.dim_target().is_some());
        if !is_dimension || self.driven_dimensions.contains(&id) {
            return false;
        }
        self.driven_dimensions.push(id);
        true
    }

    /// Whether `c` takes part in the solve: every constraint but a driven
    /// dimension.
    pub fn is_driving(&self, c: &Constraint) -> bool {
        c.kind.dim_target().is_none() || !self.driven_dimensions.contains(&c.id)
    }

    /// The constraints the solver drives toward, in sketch order: every
    /// constraint but the driven dimensions. Every solver walk over the
    /// constraints reads this one list.
    pub fn driving_constraints(&self) -> impl Iterator<Item = &Constraint> {
        self.constraints.iter().filter(move |c| self.is_driving(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::constraint::{ConstraintKind, DimTarget};
    use crate::sketch::id::SketchEntityId;

    /// A distance and a horizontal, with their ids.
    fn sketch() -> (SketchData, ConstraintId, ConstraintId) {
        let (dim, relation) = (ConstraintId::new(), ConstraintId::new());
        let sketch = SketchData {
            constraints: vec![
                Constraint {
                    id: dim,
                    kind: ConstraintKind::DistancePtPt {
                        p1: SketchEntityId::new(),
                        p2: SketchEntityId::new(),
                        target: DimTarget::Literal(2.0),
                    },
                },
                Constraint {
                    id: relation,
                    kind: ConstraintKind::Horizontal {
                        line: SketchEntityId::new(),
                    },
                },
            ],
            ..SketchData::default()
        };
        (sketch, dim, relation)
    }

    #[test]
    fn only_a_dimension_can_be_driven() {
        let (mut sketch, dim, relation) = sketch();
        assert!(
            !sketch.set_driven(relation, true),
            "a relation is never driven"
        );
        assert!(!sketch.is_driven(relation));
        assert!(
            !sketch.set_driven(ConstraintId::new(), true),
            "an unknown id changes nothing"
        );
        assert!(sketch.driven_dimensions.is_empty());

        assert!(sketch.set_driven(dim, true));
        assert!(sketch.is_driven(dim));
        assert!(!sketch.set_driven(dim, true), "already driven");
        assert_eq!(sketch.driven_dimensions, [dim]);

        assert!(sketch.set_driven(dim, false), "made driving again");
        assert!(!sketch.is_driven(dim));
        assert!(!sketch.set_driven(dim, false), "already driving");
    }

    #[test]
    fn a_driven_dimension_leaves_the_driving_list_and_a_listed_relation_does_not() {
        let (mut sketch, dim, relation) = sketch();
        sketch.set_driven(dim, true);
        // A hand-edited file may list a relation; it still drives.
        sketch.driven_dimensions.push(relation);
        let driving: Vec<ConstraintId> = sketch.driving_constraints().map(|c| c.id).collect();
        assert_eq!(driving, [relation]);
        assert!(!sketch.is_driven(relation));
    }

    #[test]
    fn a_driven_dimension_survives_a_serde_round_trip() {
        let (mut sketch, dim, _) = sketch();
        sketch.set_driven(dim, true);
        let json = serde_json::to_string(&sketch).expect("serialise");
        let back: SketchData = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, sketch);
        assert!(back.is_driven(dim));

        let (plain, _, _) = self::sketch();
        let json = serde_json::to_string(&plain).expect("serialise");
        let back: SketchData = serde_json::from_str(&json).expect("parse");
        assert!(back.driven_dimensions.is_empty(), "no list, none driven");
    }
}
