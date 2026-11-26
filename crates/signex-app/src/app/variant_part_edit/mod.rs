//! A part changed in one variant from the canvas (mockup rows 208 and 209):
//! right-click a part ▸ In variant ▸ Alternate… opens the Alternate part
//! picker, and Edit value for this variant… a popover on the part. Both
//! apply through the section commands of `project_set_variant_alternate`
//! and `project_set_variant_override` (`project_tab::variants`), one step of
//! the General history each. The canvas marks a part with an alternate
//! (`alt`) or a value of its own (`Δ`) in the active variant (`markers`).
//! See `docs/PROJECT_TAB.md`, "Alternate part and variant value on the
//! canvas".

mod candidates;
#[cfg(test)]
mod candidates_tests;
mod markers;
#[cfg(test)]
mod markers_tests;
mod open;
mod picker;
mod picker_parts;
mod popover;
mod state;
mod update;
#[cfg(test)]
mod update_tests;
#[cfg(test)]
mod view_tests;

pub use candidates::Candidates;
pub(crate) use picker::EditCard;
pub use state::{EditScope, VariantEditKind, VariantPartEdit};

/// The events of the picker and the popover.
#[derive(Debug, Clone)]
pub enum VariantPartEditMsg {
    /// The library read `generation` of the picker for the part `part`,
    /// looking for `query`, finished; `Err` is a read worker that died. A
    /// read the picker no longer waits for is dropped.
    CandidatesRead {
        generation: u64,
        part: String,
        query: String,
        read: Result<Box<Candidates>, String>,
    },
    QueryEdited(String),
    SameFootprintToggled(bool),
    /// A candidate row was clicked: its index in `Candidates::all`.
    CandidatePicked(usize),
    ScopePicked(EditScope),
    /// Apply (the picker) or OK (the popover).
    ApplyPressed,
    CancelPressed,
    /// The popover's Parameter field was clicked: its list opens or closes.
    ParameterListToggled,
    ParameterPicked(String),
    ValueEdited(String),
    ResetPressed,
}

impl From<VariantPartEditMsg> for crate::app::Message {
    fn from(msg: VariantPartEditMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::VariantPartEdit(msg))
    }
}
