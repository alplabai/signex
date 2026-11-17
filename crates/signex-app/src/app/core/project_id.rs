//! The identity of a loaded project in the workspace.

/// Opaque identifier for a loaded project in the workspace. Assigned by
/// `DocumentState::next_project_id` on load and never reused, so stale
/// references (e.g. a tab pointing at a closed project) resolve to `None`
/// via `DocumentState::project_by_id` instead of silently aliasing another
/// project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectId(u32);

impl ProjectId {
    /// The next id from the workspace counter `next`, which it bumps, so an
    /// id is never handed out twice. `DocumentState::mint_project_id` owns
    /// the counter.
    pub(in crate::app) fn mint(next: &mut u32) -> Self {
        let id = Self(*next);
        *next = next.wrapping_add(1);
        id
    }

    /// The id as an engine pick marker carries it.
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }

    /// A fixed id for a test that has no `DocumentState` to mint one from.
    #[cfg(test)]
    pub(crate) const fn test_id(n: u32) -> Self {
        Self(n)
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proj:{}", self.0)
    }
}
