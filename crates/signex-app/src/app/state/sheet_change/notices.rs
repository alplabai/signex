//! The Messages lines the schematic edit tails raise, as core notices. Each
//! renders byte-identical to the line the tail logged before the seam.

use signex_engine::Engine;

use crate::app::core::event::{CoreEvent, Notice, NoticeLevel, locked_skips_line};

/// The notices `engine` holds from its last command, taken: its older
/// English warnings first, then its typed notices, all at warning level.
pub(crate) fn engine_notices(engine: &mut Engine) -> Vec<Notice> {
    let warnings = engine
        .take_warnings()
        .into_iter()
        .map(|warning| Notice::legacy_english(NoticeLevel::Warning, warning));
    let typed = engine.take_notices().into_iter().map(Notice::engine);
    warnings.chain(typed).collect()
}

/// [`engine_notices`] as events.
pub(super) fn engine_events(engine: &mut Engine) -> impl Iterator<Item = CoreEvent> + use<> {
    engine_notices(engine).into_iter().map(CoreEvent::from)
}

/// The skip line for `skipped`, when an edit passed over a locked object.
pub(super) fn locked_skips(skipped: &[String]) -> Option<CoreEvent> {
    locked_skips_line(skipped).map(|line| Notice::legacy_english(NoticeLevel::Warning, line).into())
}

/// `context: <error and its causes>`, the form `diagnostics::log_error` gives.
pub(super) fn error_line(context: &str, error: signex_engine::EngineError) -> CoreEvent {
    let error = anyhow::Error::new(error);
    Notice::legacy_english(NoticeLevel::Error, format!("{context}: {error:#}")).into()
}

/// A plain warning line.
pub(super) fn warning(text: impl Into<String>) -> CoreEvent {
    Notice::legacy_english(NoticeLevel::Warning, text).into()
}
