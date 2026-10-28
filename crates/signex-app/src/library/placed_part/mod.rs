//! Library-side logic of the actions on a placed part: which documents a
//! part offers and how one is opened.

mod datasheet;
pub mod opener;
mod view;

/// Shows one document; see [`opener::open`].
pub type OpenDatasheet = fn(&DatasheetLocation) -> Result<(), String>;

pub use datasheet::{DatasheetLocation, DatasheetTarget, PartSource, datasheet_targets};
pub use view::{change_preview_view, datasheet_choice_view};
