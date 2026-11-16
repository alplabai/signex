//! The "Annotate undo" preference: how many undo steps one annotate run is.

/// Whether an annotate run is one undo step for the whole project or one per
/// sheet. Persisted in `prefs.json` under `annotate_undo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    /// The `prefs.json` token.
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

    /// The name a pick list shows, rendered at view time.
    pub fn label(self) -> signex_i18n::MessageId {
        use signex_i18n::ids;
        match self {
            AnnotateUndo::Project => ids::PREFERENCES_ANNOTATE_UNDO_PROJECT,
            AnnotateUndo::PerSheet => ids::PREFERENCES_ANNOTATE_UNDO_PER_SHEET,
        }
    }
}
