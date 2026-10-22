mod catalog;
mod gerber_grid_editor_state;
mod message;
mod view;

pub use gerber_grid_editor_state::{GerberGridEditorOutcome, GerberGridEditorState};
pub use message::GerberGridEditorMessage;
pub use view::view;

#[cfg(test)]
#[path = "../../tests/grid_editor/catalog.rs"]
mod grid_editor_catalog_tests;

#[cfg(test)]
#[path = "../../tests/grid_editor/draft.rs"]
mod grid_editor_draft_tests;

#[cfg(test)]
#[path = "../../tests/grid_editor/units.rs"]
mod grid_editor_unit_tests;

#[cfg(test)]
#[path = "../../tests/grid_editor/view.rs"]
mod grid_editor_view_tests;
