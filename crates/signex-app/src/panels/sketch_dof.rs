//! The sketch's DOF state as the canvas chip, the Sketch DOF section and its
//! card all read it (mockup row 179), and the conflict the card suggests
//! removing.

/// One reading of the last solve, so the chip, the section pill and the DOF
/// card never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchDof {
    /// `conflicts` conflicts: the solve's over-constrained constraints, or
    /// one rejected edit, with the last good solve kept.
    Over {
        conflicts: usize,
    },
    /// `dof` free degrees of freedom, the free motions' count.
    Under {
        dof: usize,
    },
    Full,
}

impl SketchDof {
    /// The state of a solve with `over_constraints` over-constrained
    /// constraints and `free_dof` free degrees of freedom, after an edit
    /// that was (`rejected_edit`) or was not refused.
    pub fn of(over_constraints: usize, rejected_edit: bool, free_dof: usize) -> Self {
        if over_constraints > 0 {
            return Self::Over {
                conflicts: over_constraints,
            };
        }
        if rejected_edit {
            return Self::Over { conflicts: 1 };
        }
        if free_dof > 0 {
            return Self::Under { dof: free_dof };
        }
        Self::Full
    }
}

/// The conflicting constraint the DOF card suggests removing: the first
/// conflict row still in the sketch, a dimension first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConflictSuggestion {
    pub constraint: signex_sketch::id::ConstraintId,
    /// The constraint kind's name, as the constraint menu shows it.
    pub kind_name: signex_i18n::MessageId,
    /// A dimension, so Convert to reference applies.
    pub dimension: bool,
}

#[cfg(test)]
mod tests {
    use super::SketchDof;

    #[test]
    fn a_solve_conflict_counts_its_constraints() {
        assert_eq!(SketchDof::of(2, true, 4), SketchDof::Over { conflicts: 2 });
    }

    #[test]
    fn a_rejected_edit_is_one_conflict_even_on_a_free_sketch() {
        assert_eq!(SketchDof::of(0, true, 4), SketchDof::Over { conflicts: 1 });
    }

    #[test]
    fn free_motions_make_it_under_and_none_make_it_full() {
        assert_eq!(SketchDof::of(0, false, 4), SketchDof::Under { dof: 4 });
        assert_eq!(SketchDof::of(0, false, 0), SketchDof::Full);
    }
}
