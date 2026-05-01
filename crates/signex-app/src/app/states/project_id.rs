/// Opaque identifier for a loaded project in the workspace. Assigned by
/// `DocumentState::next_project_id` on load and never reused, so stale
/// references (e.g. a tab pointing at a closed project) resolve to `None`
/// via `DocumentState::project_by_id` instead of silently aliasing another
/// project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectId(pub(crate) u32);

impl ProjectId {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proj:{}", self.0)
    }
}
