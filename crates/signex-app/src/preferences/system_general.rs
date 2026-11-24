//! System › General page (mockup row 147): the settings that cover more than
//! schematic editing, Autosave recovery copies and Allow agent connections,
//! under Files and agents. They moved here from Editing; their drafts, their
//! `prefs.json` keys and their Save path are unchanged.

use super::editing::{Help, toggle_row};
use super::pages_view::HelpTopic;
use super::setting_row::{page, page_heading, section_header};
use super::{PrefMsg, PrefsView};
use iced::Element;
use signex_i18n::{ids, t};

pub(super) fn content_general<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    page(vec![
        page_heading(
            t!(i18n, ids::PREFERENCES_GENERAL_HEADING),
            t!(i18n, ids::PREFERENCES_GENERAL_INTRO),
            t,
        ),
        section_header(t!(i18n, ids::PREFERENCES_EDITING_SECTION_FILES), None, t),
        toggle_row(
            v,
            ids::PREFERENCES_AUTOSAVE,
            Help::More(
                HelpTopic::Autosave,
                [
                    ids::PREFERENCES_AUTOSAVE_HELP_SUMMARY,
                    ids::PREFERENCES_AUTOSAVE_HELP,
                ],
            ),
            (v.draft_autosave_enabled, PrefMsg::DraftAutosaveEnabled),
        ),
        toggle_row(
            v,
            ids::PREFERENCES_AGENT_CONNECTIONS_LABEL,
            Help::More(
                HelpTopic::AgentConnections,
                [
                    ids::PREFERENCES_AGENT_CONNECTIONS_HELP_SUMMARY,
                    ids::PREFERENCES_AGENT_CONNECTIONS_HELP,
                ],
            ),
            (
                v.draft_agent_connections_enabled,
                PrefMsg::DraftAgentConnectionsEnabled,
            ),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use signex_i18n::{I18n, ids};

    /// The page's own heading and intro come from the catalog.
    #[test]
    fn the_general_page_has_its_heading_and_intro() {
        let en = I18n::en_us();
        assert_eq!(
            en.message_plain(ids::PREFERENCES_GENERAL_HEADING, &[]),
            "General"
        );
        assert_eq!(
            en.message_plain(ids::PREFERENCES_GENERAL_INTRO, &[]),
            "Settings that cover every editor: recovery copies of unsaved documents and connections from agents."
        );
    }
}
