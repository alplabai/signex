//! A line for the Messages panel, raised by the core and written by the GUI.

use std::path::Path;

use signex_engine::{Engine, EngineNotice};
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

/// The Messages line for an edit refused on a read-only sheet (#1402): file
/// name only, the reason and the way out.
pub(crate) fn read_only_refusal_message(path: Option<&Path>, format_token: &str) -> String {
    let name = path
        .and_then(|p| p.file_name())
        .map_or_else(|| "This sheet".into(), |n| n.to_string_lossy());
    format!(
        "Not changed: {name} is read-only, written by a newer Signex ({format_token}). Edit it in that version."
    )
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
