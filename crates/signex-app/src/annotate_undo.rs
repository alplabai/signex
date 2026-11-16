//! The "Annotate undo" preference: how many undo steps one annotate run is.
//! The type lives in `signex-types` since a project stores its own choice
//! (#1692); the Preferences value is the default of a project that has none.

pub use signex_types::project::AnnotateUndo;
