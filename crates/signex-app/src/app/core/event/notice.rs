//! A line for the Messages panel, raised by the core and written by the GUI.

use signex_engine::EngineNotice;
use signex_i18n_ids::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoticeLevel {
    Warning,
    Error,
}

/// What the line says: an engine notice the GUI words through the catalog,
/// or text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NoticeBody {
    Engine(EngineNotice),
    Text(Translatable),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Notice {
    pub(crate) level: NoticeLevel,
    pub(crate) body: NoticeBody,
}

impl Notice {
    /// App-authored English with no catalog id yet (#2326 M2, #2644): the
    /// one place the edit tails' lines become a `Translatable`.
    pub(crate) fn legacy_english(level: NoticeLevel, text: impl Into<String>) -> Self {
        Self {
            level,
            body: NoticeBody::Text(Translatable::legacy_english(text)),
        }
    }

    /// A typed engine notice, at the level the gateway always gave it.
    pub(crate) fn engine(notice: EngineNotice) -> Self {
        Self {
            level: NoticeLevel::Warning,
            body: NoticeBody::Engine(notice),
        }
    }
}

/// The line naming the locked objects an edit passed over; `None` when it
/// passed over none.
pub(crate) fn locked_skips_line(skipped: &[String]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    Some(format!(
        "Skipped {} locked object(s): {}; unlock in Properties to change them",
        skipped.len(),
        skipped.join(", ")
    ))
}
