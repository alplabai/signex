//! A line for the Messages panel, raised by the core and written by the GUI.

use std::path::Path;

use signex_engine::{Engine, EngineNotice};
use signex_i18n_ids::{Arg, Translatable, ids};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoticeLevel {
    Warning,
    Error,
}

/// What the line says: an engine notice the GUI words through the catalog,
/// or a catalog message.
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
    /// The catalog message `text` at `level`.
    pub(crate) fn new(level: NoticeLevel, text: Translatable) -> Self {
        Self {
            level,
            body: NoticeBody::Text(text),
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

/// The notices `engine` holds from its last command, taken: its pin notices
/// first, then the others, all at warning level.
pub(crate) fn engine_notices(engine: &mut Engine) -> Vec<Notice> {
    let pins = engine.take_warnings();
    let typed = engine.take_notices();
    pins.into_iter().chain(typed).map(Notice::engine).collect()
}

/// The Messages line for an edit refused on a read-only sheet (#1402): file
/// name only, the reason and the way out.
pub(crate) fn read_only_refusal(path: Option<&Path>, format_token: &str) -> Translatable {
    let name = match path.and_then(|p| p.file_name()) {
        Some(name) => Arg::from(name.to_string_lossy().into_owned()),
        None => Arg::from(Translatable::new(ids::NOTICE_EDIT_THIS_SHEET)),
    };
    Translatable::new(ids::NOTICE_EDIT_READ_ONLY)
        .arg("name", name)
        .arg("format", format_token)
}

/// [`read_only_refusal`] in en-US, the refusal reason an edit returns (the
/// notice tests hold the two to the same text).
pub(crate) fn read_only_refusal_message(path: Option<&Path>, format_token: &str) -> String {
    let name = path
        .and_then(|p| p.file_name())
        .map_or_else(|| "This sheet".into(), |n| n.to_string_lossy());
    format!(
        "Not changed: {name} is read-only, written by a newer Signex ({format_token}). Edit it in that version."
    )
}

/// The line naming the locked objects an edit passed over; `None` when it
/// passed over none. The count goes as text, as the line always wrote it.
pub(crate) fn locked_skips(skipped: &[String]) -> Option<Translatable> {
    if skipped.is_empty() {
        return None;
    }
    Some(
        Translatable::new(ids::NOTICE_EDIT_LOCKED_SKIPPED)
            .arg("count", skipped.len().to_string())
            .arg("names", skipped.join(", ")),
    )
}
