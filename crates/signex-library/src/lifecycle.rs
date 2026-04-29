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
    fn round_trip_all_variants() {
        for s in [
            LifecycleState::Draft,
            LifecycleState::InReview,
            LifecycleState::Released,
            LifecycleState::Deprecated,
            LifecycleState::Obsolete,
        ] {
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
