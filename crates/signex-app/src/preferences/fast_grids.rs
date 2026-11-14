//! Fast grid 1 / Fast grid 2 rows of the Schematic Editor section (#1672):
//! two mm fields, checked on Save, and the refusal line under them.

use super::widgets::{text_muted, text_primary, text_warning};
use super::{PrefMsg, SP, TS};
use crate::styles::{self, ti, tokens};
use iced::widget::{Space, column, container, row};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

/// Label and help of each fast-grid row, in field order.
const ROWS: [(MessageId, MessageId); 2] = [
    (
        ids::PREFERENCES_APPEARANCE_FAST_GRID_1,
        ids::PREFERENCES_APPEARANCE_FAST_GRID_1_HELP,
    ),
    (
        ids::PREFERENCES_APPEARANCE_FAST_GRID_2,
        ids::PREFERENCES_APPEARANCE_FAST_GRID_2_HELP,
    ),
];

/// Both fast-grid rows, one above the other, then why the last Save was
/// refused over them (`status`; empty renders nothing).
pub(super) fn fast_grid_rows<'a>(
    drafts: &'a [String; 2],
    status: &'a str,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let mut rows: iced::widget::Column<Element<'a, PrefMsg>> = column![].spacing(SP.l);
    for (index, (draft, messages)) in drafts.iter().zip(ROWS).enumerate() {
        rows = rows.push(fast_grid_row(index, draft, messages, t, i18n));
    }
    if !status.is_empty() {
        rows = rows.push(
            text(UserText::new(status.to_string()))
                .size(TS.caption)
                .style(text_warning)
                .boxed(),
        );
    }
    rows.boxed()
}

fn fast_grid_row<'a>(
    index: usize,
    draft: &'a str,
    (label, help): (MessageId, MessageId),
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    row![
        column![
            text(t!(i18n, label)).size(TS.label).style(text_primary),
            text(t!(i18n, help)).size(TS.caption).style(text_muted),
        ]
        .spacing(3)
        .width(200),
        Space::new().width(Length::Fill),
        mm_field(index, draft, t),
        text(t!(i18n, ids::PREFERENCES_APPEARANCE_FAST_GRID_UNIT))
            .size(TS.label)
            .style(text_primary),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// One mm field drawn as a Graphite field: a bordered frame around a
/// transparent input, as wide as a few digits at the label size.
fn mm_field<'a>(index: usize, draft: &'a str, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink4, ink) = (r.ink4, ti(t.text));
    // The default grid, shown while the field is empty.
    let placeholder = crate::canvas::grid::FAST_GRIDS_MM
        .get(index)
        .map(|&mm| crate::canvas::grid::fast_grid_field_text(mm))
        .unwrap_or_default();
    let input = text_input(UserText::new(placeholder), draft)
        .on_input(move |typed| PrefMsg::DraftFastGrid(index, typed))
        .padding(Padding::ZERO)
        .size(TS.label)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    container(input)
        .padding([SP.s, SP.m])
        .width(Length::Fixed(TS.label * 7.0))
        .style(styles::field(t, false))
        .boxed()
}
