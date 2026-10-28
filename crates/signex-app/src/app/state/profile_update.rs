//! An "Update from profile" the user has not confirmed yet (#1085).

use signex_erc::project_rules::profile::RuleChange;
use signex_types::project::RuleProfile;

use super::ProjectId;

/// A profile read from the library plus the diff applying it would make to
/// the project's rules. Held in the UI state while the dialog shows the diff;
/// the project is untouched until the user confirms.
#[derive(Debug, Clone)]
pub struct PendingProfileUpdate {
    pub project: ProjectId,
    pub profile: RuleProfile,
    /// `"builtin"` or `"user"`, recorded as the new provenance.
    pub source: &'static str,
    /// Empty when only the provenance record is out of date.
    pub changes: Vec<RuleChange>,
}
