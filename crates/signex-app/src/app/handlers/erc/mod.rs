//! ERC / annotate / modal handler impls for `Signex`, grouped by concern.

mod annotate;
#[cfg(test)]
mod annotate_colour_tests;
mod annotate_command;
#[cfg(test)]
mod annotate_command_tests;
#[cfg(test)]
mod annotate_golden_tests;
mod annotate_job;
#[cfg(test)]
mod annotate_job_tests;
#[cfg(test)]
mod annotate_mixed_versions_tests;
#[cfg(test)]
mod annotate_parked_tests;
mod annotate_pass;
mod annotate_placements;
mod annotate_preview;
#[cfg(test)]
mod annotate_reset_tests;
#[cfg(test)]
mod annotate_session_tests;
#[cfg(test)]
mod annotate_tests;
mod erc_check;
mod erc_job;
mod erc_run;
#[cfg(test)]
mod erc_run_tests;
mod live;
mod reprefix;
mod reprefix_confirm;
mod reprefix_job;
mod reprefix_plan;
mod reset_duplicates;
mod stale_findings;
#[cfg(test)]
mod stale_findings_tests;

pub(crate) use annotate_command::{annotate_quietly_by_operands, force_annotate_by_operands};
pub(crate) use annotate_job::{AnnotateRuns, SheetsRead};
pub(crate) use annotate_preview::rows::{
    AnnotateListView, AnnotateSession, PlacementLabel, SheetTickRow, TickState, change_count,
    changing_sheets, is_changing, listed_rows, tick_state,
};
pub(crate) use annotate_preview::{
    AnnotatePreviewCache, AnnotatePreviewEntry, AnnotatePreviewLoad,
};
pub(crate) use erc_check::ERC_CHECK;
pub(crate) use erc_job::{ErcRunOutcome, PageFrames, WaivedFinding};
pub(crate) use live::{LiveErc, LiveErcMsg};
pub(crate) use reprefix_confirm::ReprefixConfirm;
pub(crate) use reprefix_job::ReprefixSheets;
mod machine_rules;
mod modals;
#[cfg(test)]
pub(crate) use modals::modal_min_size;
pub(crate) use modals::{modal_is_fixed, modal_size, modal_window_settings};
mod net_focus;
mod net_waivers;
#[cfg(test)]
mod net_waivers_tests;
#[cfg(test)]
mod rule_profile_dialog_tests;
mod rule_profiles;
mod violation_waivers;
#[cfg(test)]
mod violation_waivers_tests;
pub(crate) use rule_profiles::blocking;
#[cfg(test)]
mod erc_dialog_tests;
#[cfg(test)]
mod rule_profiles_drc_tests;
#[cfg(test)]
mod rule_profiles_tests;
mod rules_edit;
#[cfg(test)]
mod rules_edit_tests;
#[cfg(test)]
mod rules_undo_routing_tests;
mod validation_panels;
