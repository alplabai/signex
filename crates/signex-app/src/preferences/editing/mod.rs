//! Editing page (mockup row 147): Move and select, Grid, Text and paste; Files
//! and agents moved to System › General. Long help texts show a summary with
//! More…; the fast grids moved in from Appearance with a mil readout and the row error, and the snap
//! preview edits both pitches on its drawing.

mod snap_labels;
mod snap_preview;

use super::pages_view::controls::{
    GLYPH_MINUS, GLYPH_PLUS, glyph_button, help_with_more, toggle, view_msg,
};
use super::pages_view::{HelpTopic, PagesViewMsg, PreviewKind};
use super::setting_row::{SettingRow, help_text, page, page_heading, section_header, setting_row};
use super::{PrefMsg, PrefsView};
use crate::choice::{Choice, chosen};
use crate::paste_prefs::PasteDesignators;
use crate::styles::{self, ti};
use iced::alignment::Horizontal;
use iced::widget::{Space, container, pick_list, row};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text;

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;

/// The stepper's frame height (26 px) and the width of its value (44 px).
const STEPPER_H: f32 = TS.body * 13.0 / 6.0;
const STEPPER_VALUE_W: f32 = TS.body * 11.0 / 3.0;

/// The help under a setting: plain, or a summary that More… opens.
pub(super) enum Help {
    Plain(MessageId),
    More(HelpTopic, [MessageId; 2]),
}

pub(super) fn content_editing<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    if v.pages_view.expanded_preview() == Some(PreviewKind::Snap) {
        return page(vec![snap_labels::snap_preview(v, true)]);
    }
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let section = |id: MessageId| section_header(t!(i18n, id), None, t);
    let mut items = vec![
        page_heading(
            t!(i18n, ids::PREFERENCES_EDITING_HEADING),
            t!(i18n, ids::PREFERENCES_EDITING_INTRO),
            t,
        ),
        section(ids::PREFERENCES_EDITING_SECTION_MOVE),
        toggle_row(
            v,
            ids::PREFERENCES_MOVE_KEEPS_WIRES,
            Help::More(
                HelpTopic::KeepWiresConnected,
                [
                    ids::PREFERENCES_MOVE_KEEPS_WIRES_HELP_SUMMARY,
                    ids::PREFERENCES_MOVE_KEEPS_WIRES_HELP,
                ],
            ),
            (v.draft_move_keeps_wires, PrefMsg::DraftMoveKeepsWires),
        ),
        toggle_row(
            v,
            ids::PREFERENCES_DRAG_DIRECTION,
            Help::More(
                HelpTopic::SelectByDragDirection,
                [
                    ids::PREFERENCES_DRAG_DIRECTION_HELP_SUMMARY,
                    ids::PREFERENCES_DRAG_DIRECTION_HELP,
                ],
            ),
            (
                v.draft_drag_direction_select,
                PrefMsg::DraftDragDirectionSelect,
            ),
        ),
        toggle_row(
            v,
            ids::PREFERENCES_NET_HOVER,
            Help::Plain(ids::PREFERENCES_NET_HOVER_HELP),
            (v.draft_net_hover_enabled, PrefMsg::DraftNetHoverEnabled),
        ),
        section(ids::PREFERENCES_EDITING_SECTION_GRID),
    ];
    items.extend(super::fast_grids::fast_grid_rows(
        v.draft_fast_grids,
        v.fast_grid_status,
        t,
        i18n,
    ));
    items.push(snap_labels::snap_preview(v, false));
    items.push(section(ids::PREFERENCES_EDITING_SECTION_TEXT));
    items.push(increment_step_row(v));
    items.push(paste_designators_row(v));
    page(items)
}

fn help<'a>(v: PrefsView<'a>, about: Help) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    match about {
        Help::Plain(id) => help_text(t!(i18n, id), t),
        Help::More(topic, texts) => {
            let open = v.pages_view.is_help_expanded(topic);
            help_with_more(topic, texts, open, t, i18n)
        }
    }
}

fn described<'a>(
    v: PrefsView<'a>,
    label: MessageId,
    about: Help,
    control: Element<'a, PrefMsg>,
) -> Element<'a, PrefMsg> {
    setting_row(
        SettingRow {
            label: t!(v.i18n, label),
            pills: Vec::new(),
            help: Some(help(v, about)),
            control,
            error: None,
        },
        v.panel_tokens,
    )
}

/// A setting with a toggle: its draft value and the message a flip sends.
pub(super) fn toggle_row<'a>(
    v: PrefsView<'a>,
    label: MessageId,
    about: Help,
    (on, on_change): (bool, fn(bool) -> PrefMsg),
) -> Element<'a, PrefMsg> {
    described(v, label, about, toggle(on, v.panel_tokens, on_change))
}

/// Increment step as a − / + stepper; the handler skips 0 and keeps the
/// step within its range.
fn increment_step_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (t, i18n) = (v.panel_tokens, v.i18n);
    let nudge = |delta: i32| view_msg(PagesViewMsg::IncrementStepNudged(delta));
    let edge = ti(t.border);
    let value = text(UserText::new(format!("{:+}", v.draft_increment_step)))
        .size(TS.label)
        .font(Font::MONOSPACE)
        .color(ti(t.text))
        .width(Length::Fixed(STEPPER_VALUE_W))
        .align_x(Horizontal::Center);
    let stepper = container(
        row![
            glyph_button(
                GLYPH_MINUS,
                t!(i18n, ids::PREFERENCES_EDITING_INCREMENT_DOWN),
                nudge(-1),
                0.0,
                t,
            ),
            value,
            glyph_button(
                GLYPH_PLUS,
                t!(i18n, ids::PREFERENCES_EDITING_INCREMENT_UP),
                nudge(1),
                0.0,
                t,
            ),
        ]
        .align_y(Alignment::Center),
    )
    .center_y(Length::Fixed(STEPPER_H))
    .clip(true)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(iced::Color::TRANSPARENT)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: styles::Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let control = row![Space::new().width(Length::Fill), stepper].align_y(Alignment::Center);
    described(
        v,
        ids::PREFERENCES_INCREMENT_STEP,
        Help::Plain(ids::PREFERENCES_INCREMENT_STEP_HELP),
        control.boxed(),
    )
}

fn paste_designators_row<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let modes = Choice::named(PasteDesignators::ALL, PasteDesignators::label, v.i18n);
    let picker = pick_list(
        chosen(&modes, &v.draft_paste_designators),
        modes,
        |mode: &Choice<PasteDesignators>| mode.label.clone(),
    )
    .graphite()
    .on_select(|mode: Choice<PasteDesignators>| PrefMsg::DraftPasteDesignators(mode.value))
    .width(Length::Fill);
    described(
        v,
        ids::PREFERENCES_PASTE_DESIGNATORS,
        Help::More(
            HelpTopic::PasteDesignators,
            [
                ids::PREFERENCES_PASTE_DESIGNATORS_HELP_SUMMARY,
                ids::PREFERENCES_PASTE_DESIGNATORS_HELP,
            ],
        ),
        picker.boxed(),
    )
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

    /// Mockup row 147: the whole Designators on paste help also names the
    /// default, Keep unless taken, and says what it does.
    #[test]
    fn the_paste_designators_help_names_keep_unless_taken() {
        let en = I18n::en_us();
        let help = en.message_plain(ids::PREFERENCES_PASTE_DESIGNATORS_HELP, &[]);
        let choice = en.message_plain(ids::PREFERENCES_PASTE_DESIGNATORS_KEEP_IF_FREE, &[]);
        assert_eq!(choice, "Keep unless taken");
        assert!(
            help.contains(
                "Keep unless taken, the default, keeps each designator the project does not \
                 use yet and gives a taken one the next unused number of its prefix; R? stays R?."
            ),
            "{help}"
        );
    }

    /// Each summary is shorter than the whole text its More… opens.
    #[test]
    fn every_summary_comes_with_its_whole_help_text() {
        let en = I18n::en_us();
        for (summary, full) in [
            (
                ids::PREFERENCES_MOVE_KEEPS_WIRES_HELP_SUMMARY,
                ids::PREFERENCES_MOVE_KEEPS_WIRES_HELP,
            ),
            (
                ids::PREFERENCES_DRAG_DIRECTION_HELP_SUMMARY,
                ids::PREFERENCES_DRAG_DIRECTION_HELP,
            ),
            (
                ids::PREFERENCES_PASTE_DESIGNATORS_HELP_SUMMARY,
                ids::PREFERENCES_PASTE_DESIGNATORS_HELP,
            ),
            (
                ids::PREFERENCES_AUTOSAVE_HELP_SUMMARY,
                ids::PREFERENCES_AUTOSAVE_HELP,
            ),
            (
                ids::PREFERENCES_AGENT_CONNECTIONS_HELP_SUMMARY,
                ids::PREFERENCES_AGENT_CONNECTIONS_HELP,
            ),
        ] {
            let (short, long) = (en.message_plain(summary, &[]), en.message_plain(full, &[]));
            assert!(
                short.len() < long.len(),
                "{short:?} is not shorter than {long:?}"
            );
        }
    }
}
