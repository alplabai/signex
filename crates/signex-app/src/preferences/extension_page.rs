//! An installed extension's Preferences page (#2495): each preference the
//! extension added, with the kind of value it holds and the value, on the
//! `setting_row` grid. The values sit in the one `extensions` object of
//! `prefs.json`; the extension changes them, so the page holds no draft.

use iced::Element;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;

use super::PrefMsg;
use super::pages_view::controls::readout;
use super::setting_row::{SettingRow, page, page_heading, section_header, setting_row, tone_pill};
use crate::extension::ExtensionHost;
use crate::styles::PillTone;

/// The kind of value a preference holds, as its pill names it.
pub(crate) fn value_kind(value: &serde_json::Value) -> MessageId {
    match value {
        serde_json::Value::String(_) => ids::PREFERENCES_EXTENSION_TYPE_TEXT,
        serde_json::Value::Number(_) => ids::PREFERENCES_EXTENSION_TYPE_NUMBER,
        serde_json::Value::Bool(_) => ids::PREFERENCES_EXTENSION_TYPE_SWITCH,
        serde_json::Value::Array(_) => ids::PREFERENCES_EXTENSION_TYPE_LIST,
        serde_json::Value::Object(_) => ids::PREFERENCES_EXTENSION_TYPE_GROUP,
        serde_json::Value::Null => ids::PREFERENCES_EXTENSION_TYPE_EMPTY,
    }
}

/// The page of the installed extension `index`. An index no extension with
/// preferences has draws the heading alone.
pub(super) fn content_extension<'a>(
    extensions: &'a ExtensionHost,
    index: usize,
    t: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let name = extensions.name_at(index).unwrap_or_default();
    let mut items = vec![
        page_heading(
            t!(i18n, ids::PREFERENCES_EXTENSION_PAGE, "extension" => name),
            t!(i18n, ids::PREFERENCES_EXTENSION_INTRO, "extension" => name),
            t,
        ),
        section_header(t!(i18n, ids::PREFERENCES_EXTENSION_SECTION), None, t),
    ];
    let Some(page_data) = extensions.preference_page(index) else {
        return page(items);
    };
    for (key, value) in page_data.values.iter() {
        let shown = serde_json::to_string(value).unwrap_or_default();
        items.push(setting_row(
            SettingRow {
                label: t!(i18n, ids::PREFERENCES_EXTENSION_KEY, "key" => key),
                pills: vec![tone_pill(t!(i18n, value_kind(value)), PillTone::Neutral, t)],
                help: None,
                control: readout(
                    t!(i18n, ids::PREFERENCES_EXTENSION_VALUE, "value" => shown),
                    t,
                ),
                error: None,
            },
            t,
        ));
    }
    page(items)
}
