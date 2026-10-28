//! What the component picker modal is doing for a placed part: the stages
//! of Change Component (#1260) and the datasheet list (#1301). The default
//! is the plain Place Component list.

use std::path::PathBuf;

use signex_engine::ComponentChangePreview;

use crate::library::messages::{PlacedPartTarget, ResolvedPlacement};
use crate::library::placed_part::DatasheetTarget;

#[derive(Debug, Clone, Default)]
pub enum PickerMode {
    /// Pick a part to place.
    #[default]
    Place,
    /// Pick the part that replaces the placed one.
    ChangeComponent(PlacedPartTarget),
    /// Review what the replacement does to the pins, then confirm.
    ChangePreview(Box<ChangePreview>),
    /// Pick which of the part's documents to open.
    Datasheets(DatasheetChoice),
}

/// The replacement part, read and checked against the sheet.
#[derive(Debug, Clone)]
pub struct ChangePreview {
    pub target: PlacedPartTarget,
    pub reference: String,
    pub library_path: PathBuf,
    pub resolved: ResolvedPlacement,
    pub preview: ComponentChangePreview,
}

#[derive(Debug, Clone)]
pub struct DatasheetChoice {
    pub reference: String,
    pub targets: Vec<DatasheetTarget>,
}
