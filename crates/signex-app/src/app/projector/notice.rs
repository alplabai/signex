//! A core notice, written to the Messages panel in English.

use signex_i18n::I18n;

use crate::app::core::event::{Notice, NoticeBody, NoticeLevel};

/// The Messages line of `notice`: the en-US rendering the tails logged before
/// the seam (catalog ids come with #2644).
pub(crate) fn notice_line(i18n: &I18n, notice: &Notice) -> String {
    match &notice.body {
        NoticeBody::Engine(engine) => crate::app::engine_notices::notice_line(i18n, engine),
        NoticeBody::Text(text) => text.render(i18n),
    }
}

/// Write `notice` to the Messages panel at its level.
pub(crate) fn log_notice(notice: &Notice) {
    let line = notice_line(&I18n::en_us(), notice);
    match notice.level {
        NoticeLevel::Warning => crate::diagnostics::log_warning(line),
        NoticeLevel::Error => crate::diagnostics::log_error_line(&line),
    }
}
