//! Editing section — schematic move behaviour.

use super::*;
use iced::widget::{Space, column, pick_list, row, slider};
use iced::{Element, Length, Widget};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{checkbox, raw, text};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the "Allow agent connections" row (#1924) shows: its draft and the
/// language its labels read in.
#[derive(Clone, Copy)]
pub(super) struct AgentConnectionsRow<'a> {
    pub(super) draft_enabled: bool,
    pub(super) i18n: &'a signex_i18n::I18n,
}

pub(super) fn content_editing<'a>(
    draft_move_keeps_wires: bool,
    draft_net_hover_enabled: bool,
    draft_drag_direction_select: bool,
    draft_increment_step: i32,
    draft_paste_designators: crate::paste_prefs::PasteDesignators,
    draft_autosave_enabled: bool,
    agent_connections: AgentConnectionsRow<'_>,
) -> Element<'a, PrefMsg> {
    let AgentConnectionsRow {
        draft_enabled: draft_agent_connections_enabled,
        i18n,
    } = agent_connections;
    // The label and help text of one setting.
    let described = |label: MessageId, help: MessageId| {
        column![
            text(t!(i18n, label)).size(TS.label).style(text_primary),
            text(t!(i18n, help)).size(TS.caption).style(text_muted),
        ]
        .spacing(3)
        .width(320)
    };
    let setting = row![
        described(
            ids::PREFERENCES_MOVE_KEEPS_WIRES,
            ids::PREFERENCES_MOVE_KEEPS_WIRES_HELP
        ),
        Space::new().width(Length::Fill),
        checkbox(draft_move_keeps_wires)
            .size(TS.heading)
            .on_toggle(PrefMsg::DraftMoveKeepsWires),
    ]
    .align_y(iced::Alignment::Center);

    let net_hover = row![
        described(ids::PREFERENCES_NET_HOVER, ids::PREFERENCES_NET_HOVER_HELP),
        Space::new().width(Length::Fill),
        checkbox(draft_net_hover_enabled)
            .size(TS.heading)
            .on_toggle(PrefMsg::DraftNetHoverEnabled),
    ]
    .align_y(iced::Alignment::Center);

    let drag_direction = row![
        described(
            ids::PREFERENCES_DRAG_DIRECTION,
            ids::PREFERENCES_DRAG_DIRECTION_HELP
        ),
        Space::new().width(Length::Fill),
        checkbox(draft_drag_direction_select)
            .size(TS.heading)
            .on_toggle(PrefMsg::DraftDragDirectionSelect),
    ]
    .align_y(iced::Alignment::Center);

    let increment_step = row![
        described(
            ids::PREFERENCES_INCREMENT_STEP,
            ids::PREFERENCES_INCREMENT_STEP_HELP
        ),
        Space::new().width(Length::Fill),
        slider(
            signex_types::text_increment::STEP_MIN..=signex_types::text_increment::STEP_MAX,
            draft_increment_step,
            PrefMsg::DraftIncrementStep,
        )
        .width(140),
        // raw: a signed number, not language.
        text(raw(format!("{draft_increment_step:+}")))
            .size(TS.label)
            .style(text_primary)
            .width(48)
            .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let paste_designators = row![
        described(
            ids::PREFERENCES_PASTE_DESIGNATORS,
            ids::PREFERENCES_PASTE_DESIGNATORS_HELP
        ),
        Space::new().width(Length::Fill),
        // The mode names come from `PasteDesignators`'s Display (#2059).
        pick_list(
            Some(draft_paste_designators),
            crate::paste_prefs::PasteDesignators::ALL,
            ToString::to_string,
        )
        .graphite()
        .on_select(PrefMsg::DraftPasteDesignators),
    ]
    .align_y(iced::Alignment::Center);

    let autosave = row![
        described(ids::PREFERENCES_AUTOSAVE, ids::PREFERENCES_AUTOSAVE_HELP),
        Space::new().width(Length::Fill),
        checkbox(draft_autosave_enabled)
            .size(TS.heading)
            .on_toggle(PrefMsg::DraftAutosaveEnabled),
    ]
    .align_y(iced::Alignment::Center);

    let agent_connections = row![
        described(
            ids::PREFERENCES_AGENT_CONNECTIONS_LABEL,
            ids::PREFERENCES_AGENT_CONNECTIONS_HELP
        ),
        Space::new().width(Length::Fill),
        checkbox(draft_agent_connections_enabled)
            .size(TS.heading)
            .on_toggle(PrefMsg::DraftAgentConnectionsEnabled),
    ]
    .align_y(iced::Alignment::Center);

    column![
        section_title(t!(i18n, ids::PREFERENCES_SECTION_EDITING)),
        Space::new().height(10),
        setting,
        Space::new().height(14),
        net_hover,
        Space::new().height(14),
        drag_direction,
        Space::new().height(14),
        increment_step,
        Space::new().height(14),
        paste_designators,
        Space::new().height(14),
        autosave,
        Space::new().height(14),
        agent_connections,
    ]
    .padding([SP.xl, 20.0])
    .boxed()
}

#[cfg(test)]
mod tests {
    use signex_i18n::{I18n, ids};

    /// The help texts keep the English the view showed before the catalog.
    #[test]
    fn the_editing_help_keeps_its_english() {
        let en = I18n::en_us();
        assert_eq!(
            en.message_plain(ids::PREFERENCES_NET_HOVER_HELP, &[]),
            "When on, resting the pointer on a wire, pin or label for a moment shows a card with the net's name, its net class and the pins on it with the sheet each is on. Turn it off if the card gets in the way while you edit."
        );
        assert_eq!(
            en.message_plain(ids::PREFERENCES_AUTOSAVE_HELP, &[]),
            "When on, Signex keeps a recovery copy of each schematic, symbol and footprint with unsaved changes about once a minute. The copy is a separate file in the Signex config folder (recovery); your own file is never touched by it. The copy is removed when you save the document, close it without keeping the changes, or quit with Discard All, even if you turn this off first. Off by default."
        );
    }
}
