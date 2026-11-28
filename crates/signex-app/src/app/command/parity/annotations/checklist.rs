//! The Design Checklist's queries (#1291). The checklist's last run is check
//! results, core-bound session state (scripts/uifree-state.txt), so it is no
//! implicit input.

use super::super::forms::Form::Query;
use super::Annotation;

pub(super) const CHECKLIST: &[Annotation] = &[(
    &["project_checklist", "project_checklist_link_choices"],
    Query,
    &[],
)];
