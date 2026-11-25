//! The Messages lines the schematic edit tails raise, as core notices with
//! catalog ids (#2644). Each renders in en-US byte-identical to the line the
//! tail logged before the seam.

use signex_engine::{Engine, EngineError};
use signex_i18n_ids::{MessageId, Translatable, ids};

use crate::app::core::event::{CoreEvent, Notice, NoticeLevel, engine_notices, locked_skips};

/// [`engine_notices`] as events.
pub(super) fn engine_notice_events(engine: &mut Engine) -> impl Iterator<Item = CoreEvent> + use<> {
    engine_notices(engine).into_iter().map(CoreEvent::from)
}

/// The skip line for `skipped`, when an edit passed over a locked object.
pub(super) fn skip_notice(skipped: &[String]) -> Option<CoreEvent> {
    locked_skips(skipped).map(warning_notice)
}

/// `<failure>: <error and its causes>`, the form `diagnostics::log_error`
/// gives; `failure` is a catalog line ending in `{ $error }`.
pub(super) fn engine_error_notice(failure: MessageId, error: EngineError) -> CoreEvent {
    let error = format!("{:#}", anyhow::Error::new(error));
    let text = Translatable::new(failure).arg("error", error);
    Notice::new(NoticeLevel::Error, text).into()
}

/// A warning line.
pub(super) fn warning_notice(text: Translatable) -> CoreEvent {
    Notice::new(NoticeLevel::Warning, text).into()
}

/// The line of an edit refused because `references` are locked: the words
/// of `EngineError::ItemLocked`.
pub(super) fn locked_refusal(references: &[String], action: &str) -> Translatable {
    match references {
        [one] => Translatable::new(ids::NOTICE_EDIT_ITEM_LOCKED_ONE).arg("name", one.as_str()),
        many => Translatable::new(ids::NOTICE_EDIT_ITEM_LOCKED_MANY).arg("names", many.join(", ")),
    }
    .arg("action", action)
}
