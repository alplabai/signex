//! ERC / annotate / modal handler impls for `Signex`, grouped by concern.

mod annotate;
mod annotate_preview;
#[cfg(test)]
mod annotate_tests;
mod erc_job;
mod erc_run;
#[cfg(test)]
mod erc_run_tests;

pub(crate) use annotate_preview::{AnnotatePreviewCache, AnnotatePreviewLoad};
pub(crate) use erc_job::ErcRunOutcome;
mod modals;
#[cfg(test)]
pub(crate) use modals::modal_is_fixed;
pub(crate) use modals::modal_window_settings;
mod net_focus;
mod rule_profiles;
#[cfg(test)]
mod rule_profiles_tests;
mod rules_edit;
#[cfg(test)]
mod rules_edit_tests;
#[cfg(test)]
mod rules_undo_routing_tests;
