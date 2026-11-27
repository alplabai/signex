//! Asking before a notebook from outside the project runs (mockup row
//! 200): its cells run only after the person allows it, and the answer is
//! kept per notebook.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::sandbox::is_inside;

/// Whether a notebook's cells may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    /// A notebook of the project, or one the person allowed.
    Trusted,
    /// A notebook from outside the project the person has not answered for.
    Unasked,
    /// The person said no for this notebook.
    Refused,
}

/// The answers given for notebooks from outside their project, by the
/// notebook's absolute path. The app keeps them with the person's
/// preferences, never in a project.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrustAnswers {
    answers: BTreeMap<String, bool>,
}

impl TrustAnswers {
    /// Whether the cells of the notebook at `notebook` (absolute) may run
    /// in the project at `project_root`: always for a notebook inside it,
    /// else the answer kept for it.
    pub fn trust(&self, notebook: &str, project_root: &str) -> Trust {
        if is_inside(notebook, project_root) {
            return Trust::Trusted;
        }
        match self.answers.get(notebook) {
            Some(true) => Trust::Trusted,
            Some(false) => Trust::Refused,
            None => Trust::Unasked,
        }
    }

    /// Keep the person's answer for the notebook at `notebook`.
    pub fn record(&mut self, notebook: &str, allowed: bool) {
        self.answers.insert(notebook.to_string(), allowed);
    }

    /// Forget the answer for `notebook`, so the next run asks again.
    pub fn forget(&mut self, notebook: &str) {
        self.answers.remove(notebook);
    }
}
