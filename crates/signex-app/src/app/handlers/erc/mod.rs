//! ERC / annotate / modal handler impls for `Signex`, grouped by concern.

mod annotate;
mod annotate_pass;
mod annotate_preview;
#[cfg(test)]
mod annotate_tests;
mod erc_job;
mod erc_run;
#[cfg(test)]
mod erc_run_tests;
mod reprefix;
mod reprefix_confirm;
mod reprefix_plan;

pub(crate) use annotate_preview::{AnnotatePreviewCache, AnnotatePreviewLoad};
pub(crate) use erc_job::ErcRunOutcome;
pub(crate) use reprefix_confirm::ReprefixConfirm;
mod modals;
mod net_focus;
mod net_waivers;
#[cfg(test)]
mod net_waivers_tests;
mod rule_profiles;
#[cfg(test)]
mod rule_profiles_tests;
mod rules_edit;
#[cfg(test)]
mod rules_edit_tests;
#[cfg(test)]
mod rules_undo_routing_tests;
