//! The Messages lines the schematic edit tails raise, as core notices. Each
//! renders byte-identical to the line the tail logged before the seam.

use signex_engine::Engine;

use crate::app::core::event::{CoreEvent, Notice, NoticeLevel, engine_notices, locked_skips_line};

/// [`engine_notices`] as events.
pub(super) fn engine_notice_events(engine: &mut Engine) -> impl Iterator<Item = CoreEvent> + use<> {
    engine_notices(engine).into_iter().map(CoreEvent::from)
}

/// The skip line for `skipped`, when an edit passed over a locked object.
pub(super) fn skip_notice(skipped: &[String]) -> Option<CoreEvent> {
    locked_skips_line(skipped).map(|line| Notice::legacy_english(NoticeLevel::Warning, line).into())
}

/// `context: <error and its causes>`, the form `diagnostics::log_error` gives.
pub(super) fn engine_error_notice(context: &str, error: signex_engine::EngineError) -> CoreEvent {
    let error = anyhow::Error::new(error);
    Notice::legacy_english(NoticeLevel::Error, format!("{context}: {error:#}")).into()
}

/// A plain warning line.
pub(super) fn warning_notice(text: impl Into<String>) -> CoreEvent {
    Notice::legacy_english(NoticeLevel::Warning, text).into()
}
