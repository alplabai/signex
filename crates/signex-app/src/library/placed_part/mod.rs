//! Library-side logic of the actions on a placed part: which documents a
//! part offers and how one is opened.

mod datasheet;
pub mod opener;

pub use datasheet::{DatasheetLocation, DatasheetTarget, PartSource, datasheet_targets};
