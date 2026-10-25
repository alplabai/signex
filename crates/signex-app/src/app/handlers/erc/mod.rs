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
