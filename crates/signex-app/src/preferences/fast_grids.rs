//! Fast grid 1 / Fast grid 2 rows of Editing ▸ Grid (#1672, mockup row 147): an mm
//! field with its mil readout, the row error while Save would refuse the
//! text, and under the rows why the last Save was refused.

use super::PrefMsg;
use super::setting_row::{SettingRow, help_text, setting_row};
use super::widgets::text_warning;
use crate::app::UiState;
use crate::canvas::grid::{
    FAST_GRIDS_MM, fast_grid_field_text, parse_fast_grid_field, pitch_mm_exact,
};
use crate::styles::{self, ti, tokens};
use iced::alignment::Horizontal;
use iced::widget::{Space, container, row};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget as _,
};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::{LengthFormat, text, text_input};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Width of the mm field (96 px) and of the mil readout (48 px).
const FIELD_W: f32 = TS.body * 8.0;
const MIL_W: f32 = TS.body * 4.0;

/// Label and help of each fast-grid row, in field order.
const ROWS: [(MessageId, MessageId); 2] = [
    (
        ids::PREFERENCES_APPEARANCE_FAST_GRID_1,
        ids::PREFERENCES_EDITING_FAST_GRID_1_HELP,
    ),
    (
        ids::PREFERENCES_APPEARANCE_FAST_GRID_2,
        ids::PREFERENCES_EDITING_FAST_GRID_2_HELP,
    ),
];

/// Both fast-grid rows, then why the last Save was refused over them
/// (`status`; empty adds nothing).
pub(super) fn fast_grid_rows<'a>(
    drafts: &'a [String; 2],
    status: &'a str,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PrefMsg>> {
    let mut rows: Vec<Element<'a, PrefMsg>> = drafts
        .iter()
        .zip(ROWS)
        .enumerate()
        .map(|(index, (draft, messages))| fast_grid_row(index, draft, messages, t, i18n))
        .collect();
    if !status.is_empty() {
        rows.push(
            container(
                text(UserText::new(status.to_string()))
                    .size(TS.caption)
                    .style(text_warning),
            )
            .padding([SP.s, 0.0])
            .boxed(),
        );
    }
    rows
}

fn fast_grid_row<'a>(
    index: usize,
    draft: &'a str,
    (label, help): (MessageId, MessageId),
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let problem = UiState::fast_grid_problem(draft, i18n);
    let ink4 = tokens::resolve(t).ink4;
    let control = row![
        Space::new().width(Length::Fill),
        mm_field(index, draft, problem.is_some(), t),
        text(t!(i18n, ids::PREFERENCES_APPEARANCE_FAST_GRID_UNIT))
            .size(TS.caption)
            .color(ink4),
        text(mil_readout(draft, i18n))
            .size(TS.caption)
            .font(Font::MONOSPACE)
            .color(ink4)
            .width(Length::Fixed(MIL_W)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    setting_row(
        SettingRow {
            label: t!(i18n, label),
            pills: Vec::new(),
            help: Some(help_text(t!(i18n, help), t)),
            control: control.boxed(),
            error: problem.map(
                |reason| t!(i18n, ids::PREFERENCES_EDITING_FAST_GRID_PROBLEM, "reason" => reason),
            ),
        },
        t,
    )
}

/// The field's grid in mil ("50 mil"), or a dash while Save would refuse it.
pub(super) fn mil_readout(draft: &str, i18n: &I18n) -> Localized {
    match parse_fast_grid_field(draft) {
        Ok(mm) => {
            let mil = LengthFormat::new(i18n, Unit::Mil).number_rounded(pitch_mm_exact(mm), 1);
            t!(i18n, ids::PREFERENCES_EDITING_FAST_GRID_MIL, "value" => mil)
        }
        Err(_) => t!(i18n, ids::PREFERENCES_EDITING_FAST_GRID_MIL_INVALID),
    }
}

/// One mm field drawn as a Graphite field: a bordered frame around a
/// transparent mono input, red-edged while Save would refuse its text.
fn mm_field<'a>(
    index: usize,
    draft: &'a str,
    refused: bool,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink4, ink, err) = (r.ink4, ti(t.text), ti(t.error));
    // The default grid, shown while the field is empty.
    let placeholder = FAST_GRIDS_MM
        .get(index)
        .map(|&mm| fast_grid_field_text(mm))
        .unwrap_or_default();
    let input = text_input(UserText::new(placeholder), draft)
        .on_input(move |typed| PrefMsg::DraftFastGrid(index, typed))
        .padding(Padding::ZERO)
        .size(TS.label)
        .font(Font::MONOSPACE)
        .align_x(Horizontal::Right)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    let frame = styles::field(t, false);
    container(input)
        .padding([SP.s, SP.m])
        .width(Length::Fixed(FIELD_W))
        .style(move |theme: &Theme| {
            let mut style = frame(theme);
            if refused {
                style.border.color = err;
            }
            style
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn the_mil_readout_follows_the_field_and_dashes_a_refused_grid() {
        let en = I18n::en_us();
        assert_eq!(plain(mil_readout("1.27", &en)), "50 mil");
        assert_eq!(plain(mil_readout("2.54", &en)), "100 mil");
        assert_eq!(plain(mil_readout("0.0000001", &en)), "—");
        assert_eq!(plain(mil_readout("abc", &en)), "—");
    }
}
