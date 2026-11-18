use serde::{Deserialize, Serialize};

/// v0.9-library-plan.md §4 lifecycle states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LifecycleState {
    /// Working state; placeable only with explicit opt-in.
    Draft,
    /// Submitted; only set when `review_required` is on.
    InReview,
    /// Default placeable state.
    Released,
    /// Placeable but warns; for repair / continuity.
    Deprecated,
    /// Not placeable in new designs; ECN required.
    Obsolete,
}

impl LifecycleState {
    /// Every state, in lifecycle order.
    pub const ALL: [Self; 5] = [
        Self::Draft,
        Self::InReview,
        Self::Released,
        Self::Deprecated,
        Self::Obsolete,
    ];

    /// The state's name as a project requirement lists it (`released`,
    /// `in_review`; #2339).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InReview => "in_review",
            Self::Released => "released",
            Self::Deprecated => "deprecated",
            Self::Obsolete => "obsolete",
        }
    }

    /// Whether new placements are allowed (UI may still gate).
    pub fn placeable(self) -> bool {
        matches!(self, Self::Released | Self::Deprecated)
    }

    /// Whether a placement should warn the user.
    pub fn warns_on_place(self) -> bool {
        matches!(self, Self::Deprecated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_state_once() {
        // Exhaustive on purpose: a new state does not compile here until it
        // has a place in `ALL`.
        let place = |state: LifecycleState| match state {
            LifecycleState::Draft => 0,
            LifecycleState::InReview => 1,
            LifecycleState::Released => 2,
            LifecycleState::Deprecated => 3,
            LifecycleState::Obsolete => 4,
        };
        for (index, state) in LifecycleState::ALL.into_iter().enumerate() {
            assert_eq!(place(state), index, "{state:?}");
        }
    }

    #[test]
    fn round_trip_all_variants() {
        for s in LifecycleState::ALL {
            let json = serde_json::to_string(&s).unwrap();
            let back: LifecycleState = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn placeability_matches_plan() {
        assert!(!LifecycleState::Draft.placeable());
        assert!(!LifecycleState::InReview.placeable());
        assert!(LifecycleState::Released.placeable());
        assert!(LifecycleState::Deprecated.placeable());
        assert!(!LifecycleState::Obsolete.placeable());
    }

    #[test]
    fn only_deprecated_warns() {
        assert!(LifecycleState::Deprecated.warns_on_place());
        assert!(!LifecycleState::Released.warns_on_place());
    }
}
