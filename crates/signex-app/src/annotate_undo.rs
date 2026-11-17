//! The "Annotate undo" preference: how many undo steps one annotate run is.
//! The type lives in `signex-types` since a project stores its own choice
//! (#1692); the Preferences value is the default of a project that has none.

pub use signex_types::project::AnnotateUndo;

/// The name a pick list shows for `undo`, rendered at view time.
pub fn annotate_undo_label(undo: AnnotateUndo) -> signex_i18n::MessageId {
    use signex_i18n::ids;
    match undo {
        AnnotateUndo::Project => ids::PREFERENCES_ANNOTATE_UNDO_PROJECT,
        AnnotateUndo::PerSheet => ids::PREFERENCES_ANNOTATE_UNDO_PER_SHEET,
    }
}
