//! The stitch warnings already shown in Messages, per project.
//!
//! The net-class style refresh and the net index both stitch the project and
//! both can find an unreadable sheet; whichever sees it first reports it, and
//! the other stays quiet, so each is said exactly once until it clears.

use std::collections::HashSet;

use crate::app::state::ProjectId;

#[derive(Debug, Default)]
pub struct ReportedWarnings {
    seen: HashSet<(ProjectId, String)>,
}

impl ReportedWarnings {
    /// `true` the first time `warning` is claimed for `project`: the caller
    /// reports it. `false` when it was already reported.
    pub fn claim(&mut self, project: ProjectId, warning: &str) -> bool {
        self.seen.insert((project, warning.to_string()))
    }

    /// `project`'s warnings are now exactly `current`: one that went away may
    /// be reported again if it comes back.
    pub fn settle(&mut self, project: ProjectId, current: &[String]) {
        self.seen
            .retain(|(p, w)| *p != project || current.contains(w));
    }

    pub fn retain_projects(&mut self, keep: &[ProjectId]) {
        self.seen.retain(|(p, _)| keep.contains(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_warning_is_claimed_once_by_whichever_job_sees_it_first() {
        let p = ProjectId::test_id(1);
        let mut r = ReportedWarnings::default();
        assert!(r.claim(p, "page.snxsch unreadable"));
        assert!(!r.claim(p, "page.snxsch unreadable"));
        assert!(r.claim(ProjectId::test_id(2), "page.snxsch unreadable"));
    }

    #[test]
    fn a_warning_that_cleared_is_reported_again_when_it_returns() {
        let p = ProjectId::test_id(1);
        let mut r = ReportedWarnings::default();
        let w = vec!["w".to_string()];
        r.claim(p, "w");
        r.settle(p, &w);
        assert!(!r.claim(p, "w"));
        r.settle(p, &[]);
        assert!(r.claim(p, "w"));
    }

    #[test]
    fn a_closed_project_is_forgotten() {
        let (a, b) = (ProjectId::test_id(1), ProjectId::test_id(2));
        let mut r = ReportedWarnings::default();
        r.claim(a, "w");
        r.claim(b, "w");
        r.retain_projects(&[a]);
        assert!(r.claim(b, "w"));
        assert!(!r.claim(a, "w"));
    }
}
