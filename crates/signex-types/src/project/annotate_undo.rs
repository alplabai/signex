//! How many undo steps one annotate run is (#1692). A project stores its
//! own choice in its `annotation` settings; a project without one follows
//! the machine's Preferences value, which also seeds new projects.

use serde::{Deserialize, Serialize};

/// Whether an annotate run is one undo step for the whole project or one per
/// sheet. Stored as `"project"` / `"per_sheet"` in `prefs.json` and in the
/// `.snxprj` `annotation.annotate_undo` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotateUndo {
    /// One step: Ctrl+Z on any sheet the run touched reverts every touched
    /// sheet together, and Ctrl+Y redoes them together.
    #[default]
    Project,
    /// One step per sheet; each sheet undoes on its own.
    PerSheet,
}

impl AnnotateUndo {
    pub const ALL: &'static [AnnotateUndo] = &[AnnotateUndo::Project, AnnotateUndo::PerSheet];

    /// The stored token.
    pub fn token(self) -> &'static str {
        match self {
            AnnotateUndo::Project => "project",
            AnnotateUndo::PerSheet => "per_sheet",
        }
    }

    /// Parse a `prefs.json` token; anything unknown is the default.
    pub fn from_token(token: &str) -> Self {
        if token.eq_ignore_ascii_case("per_sheet") {
            AnnotateUndo::PerSheet
        } else {
            AnnotateUndo::Project
        }
    }

    /// Parse a token strictly: `None` for one that is not a choice.
    pub fn parse(token: &str) -> Option<Self> {
        let token = token.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|choice| choice.token().eq_ignore_ascii_case(token))
    }
}

impl std::fmt::Display for AnnotateUndo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnotateUndo::Project => write!(f, "One step for the whole project"),
            AnnotateUndo::PerSheet => write!(f, "Per sheet"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AnnotateUndo;
    use crate::designator::DesignatorStandard;
    use crate::project::{parse_project, write_project};

    fn project_file(dir: &std::path::Path, extra: &str) -> std::path::PathBuf {
        let path = dir.join("p.snxprj");
        let body =
            format!(r#"{{"name":"p","dir":"","schematic_root":null,"pcb_file":null{extra}}}"#);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn tokens_parse_strictly_and_leniently() {
        assert_eq!(
            AnnotateUndo::parse("per_sheet"),
            Some(AnnotateUndo::PerSheet)
        );
        assert_eq!(AnnotateUndo::parse("PROJECT"), Some(AnnotateUndo::Project));
        assert_eq!(AnnotateUndo::parse("each"), None);
        assert_eq!(AnnotateUndo::from_token("each"), AnnotateUndo::Project);
    }

    #[test]
    fn the_project_choice_round_trips_beside_the_designator_standard() {
        let dir = tempfile::tempdir().unwrap();
        let path = project_file(
            dir.path(),
            r#","annotation":{"designator_standard":"din40719_2","annotate_undo":"per_sheet"}"#,
        );
        let data = parse_project(&path).unwrap();
        assert_eq!(data.annotate_undo(), Some(AnnotateUndo::PerSheet));
        assert_eq!(
            data.designator_standard(),
            Some(DesignatorStandard::Din40719_2)
        );
        write_project(&path, &data).unwrap();
        let again = parse_project(&path).unwrap();
        assert_eq!(again.annotate_undo(), Some(AnnotateUndo::PerSheet));
    }

    #[test]
    fn clearing_the_last_choice_drops_the_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = project_file(dir.path(), "");
        let mut data = parse_project(&path).unwrap();
        data.set_annotate_undo(Some(AnnotateUndo::Project));
        assert!(data.annotation.is_some());
        data.set_annotate_undo(None);
        assert!(data.annotation.is_none());
        write_project(&path, &data).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("annotation"), "{text}");
    }
}
