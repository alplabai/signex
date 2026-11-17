//! The identity of a loaded project in the workspace.

/// Opaque identifier for a loaded project in the workspace. Assigned by
/// `DocumentState::next_project_id` on load and never reused, so stale
/// references (e.g. a tab pointing at a closed project) resolve to `None`
/// via `DocumentState::project_by_id` instead of silently aliasing another
/// project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectId(u32);

impl ProjectId {
    /// The id for counter value `raw`. Only `DocumentState::mint_project_id`
    /// calls it, so an id is never reused.
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
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
