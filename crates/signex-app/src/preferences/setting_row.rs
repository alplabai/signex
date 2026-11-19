//! The page grid every Preferences page is drawn on (#143): the page
//! padding, the heading with its intro, the section header, and the setting
//! row with the label and help on the left and the control in a fixed column
//! on the right. Sizes follow the Graphite mock at the default 12 px base.

use std::sync::LazyLock;

use iced::widget::{Column, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme};
use iced::{Widget as _, font};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::PrefMsg;
use crate::styles::{self, PillTone, Radii, Spacing, TypeScale, chrome_px, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A length the mock gives in px at the default 12 px base.
const fn px(at_default_base: f32) -> f32 {
    chrome_px(at_default_base * 11.0 / 12.0)
}

/// Width of the control column of a setting row (232 px).
pub(in crate::preferences) const CONTROL_COLUMN_W: f32 = px(232.0);
/// Page padding: 18 px on top, 24 px at the sides and the bottom.
const PAGE_PAD_TOP: f32 = px(18.0);
const PAGE_PAD: f32 = px(24.0);
/// Gap between the label column and the control column.
const ROW_GAP: f32 = px(24.0);
/// Vertical padding of a setting row.
const ROW_PAD_Y: f32 = px(10.0);
/// Space above a section header and under its label.
const SECTION_TOP: f32 = px(20.0);
const SECTION_PAD_BOTTOM: f32 = px(6.0);
/// Height of a small button (`.btn.sm`).
const SMALL_BUTTON_H: f32 = px(22.0);
/// Side of a status glyph in a row error or a banner.
const GLYPH: f32 = px(14.0);
/// The CSS `ch`, in em: the advance of a digit, which the mock's line caps use.
const CH_EM: f32 = 0.55;
/// Line caps of the intro (72ch) and of a row's help (66ch).
const INTRO_MAX_W: f32 = 72.0 * CH_EM * TS.body;
const HELP_MAX_W: f32 = 66.0 * CH_EM * TS.caption;

/// Weight 600 of the default UI family.
pub(in crate::preferences) fn semibold() -> Font {
    Font {
        weight: font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

/// Weight 500 of the default UI family.
pub(in crate::preferences) fn medium() -> Font {
    Font {
        weight: font::Weight::Medium,
        ..Font::DEFAULT
    }
}

/// A full-width 1 px `line_soft` rule.
pub(in crate::preferences) fn hairline<'a>(t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let line = tokens::resolve(t).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// A whole page: `items` stacked inside the page padding.
pub(in crate::preferences) fn page<'a>(items: Vec<Element<'a, PrefMsg>>) -> Element<'a, PrefMsg> {
    container(Column::with_children(items).width(Length::Fill))
        .padding(Padding {
            top: PAGE_PAD_TOP,
            right: PAGE_PAD,
            bottom: PAGE_PAD,
            left: PAGE_PAD,
        })
        .width(Length::Fill)
        .boxed()
}

/// The page heading (heading size, weight 600) and the muted intro under it.
pub(in crate::preferences) fn page_heading<'a>(
    title: Localized,
    intro: Localized,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let ink3 = tokens::resolve(t).ink3;
    column![
        text(title)
            .size(TS.heading)
            .font(semibold())
            .color(ti(t.text)),
        container(text(intro).size(TS.body).color(ink3)).width(Length::Fit.max(INTRO_MAX_W)),
    ]
    .spacing(SP.xs)
    .padding(Padding {
        bottom: SP.s,
        ..Padding::ZERO
    })
    .width(Length::Fill)
    .boxed()
}

/// A section header: the label in capitals (a catalog message written in
/// capitals, the catalog's rule for caps labels), an optional control at the
/// right, a hairline under it.
pub(in crate::preferences) fn section_header<'a>(
    title: Localized,
    trailing: Option<Element<'a, PrefMsg>>,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let mut line = row![
        text(title)
            .size(TS.section)
            .font(medium())
            .style(styles::section_label(t))
            .width(Length::Fill)
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    if let Some(trailing) = trailing {
        line = line.push(trailing);
    }
    column![
        container(line).padding(Padding {
            top: SECTION_TOP,
            bottom: SECTION_PAD_BOTTOM,
            ..Padding::ZERO
        }),
        hairline(t),
    ]
    .padding(Padding {
        bottom: SP.xs,
        ..Padding::ZERO
    })
    .width(Length::Fill)
    .boxed()
}

/// One setting: what [`setting_row`] draws.
pub(in crate::preferences) struct SettingRow<'a> {
    /// The setting's name.
    pub label: Localized,
    /// Pills after the name (Restart, Experimental, ...).
    pub pills: Vec<Element<'a, PrefMsg>>,
    /// What the setting does, under the name.
    pub help: Option<Element<'a, PrefMsg>>,
    /// The control, right-aligned in the control column.
    pub control: Element<'a, PrefMsg>,
    /// Why the drafted value is refused; tints the row and shows under the help.
    pub error: Option<Localized>,
}

/// A setting row: the name (label size, weight 500) with its pills, the help
/// under it, the control in the [`CONTROL_COLUMN_W`] column, a hairline under
/// the row. An error tints the row `err_soft` and adds a warn glyph and the
/// message in the error ink under the help.
pub(in crate::preferences) fn setting_row<'a>(
    row_data: SettingRow<'a>,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let SettingRow {
        label,
        pills,
        help,
        control,
        error,
    } = row_data;
    let mut name = row![text(label).size(TS.label).font(medium()).color(ti(t.text))]
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center);
    for pill in pills {
        name = name.push(pill);
    }
    let mut left = column![name].spacing(SP.xs).width(Length::Fill);
    if let Some(help) = help {
        left = left.push(container(help).width(Length::Fit.max(HELP_MAX_W)).boxed());
    }
    let failed = error.is_some();
    if let Some(error) = error {
        let ink = ti(t.error);
        left = left.push(
            row![
                status_glyph(StatusGlyph::Warn, ink),
                text(error).size(TS.caption).color(ink).width(Length::Fill),
            ]
            .spacing(SP.s + SP.xs)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    let control = container(control)
        .width(Length::Fixed(CONTROL_COLUMN_W))
        .align_x(iced::alignment::Horizontal::Right)
        .padding(Padding {
            top: 1.0,
            ..Padding::ZERO
        });
    let tint = failed.then(|| tokens::resolve(t).err_soft);
    column![
        container(
            row![left, control]
                .spacing(ROW_GAP)
                .align_y(Alignment::Start)
                .width(Length::Fill),
        )
        .padding([ROW_PAD_Y, 0.0])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: tint.map(Background::Color),
            ..container::Style::default()
        }),
        hairline(t),
    ]
    .width(Length::Fill)
    .boxed()
}

/// A row's help: caption size in `ink3`.
pub(in crate::preferences) fn help_text<'a>(
    text_: Localized,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    text(text_)
        .size(TS.caption)
        .color(tokens::resolve(t).ink3)
        .width(Length::Fill)
        .boxed()
}

/// A small status pill in `tone`, for the name of a setting row.
pub(in crate::preferences) fn tone_pill<'a>(
    label: Localized,
    tone: PillTone,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    container(text(label).size(TS.caption))
        .padding([0.0, SP.s + SP.xs])
        .height(Length::Fixed(px(16.0)))
        .align_y(iced::alignment::Vertical::Center)
        .style(styles::pill(t, tone))
        .boxed()
}

/// Style of a small button (`.btn.sm`): field fill, `border` hairline,
/// small radius; the hover token on hover.
pub(in crate::preferences) fn small_button_style(
    t: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (fill, hover, line, ink) = (
        tokens::resolve(t).field,
        ti(t.hover),
        ti(t.border),
        ti(t.text),
    );
    move |_: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                hover
            } else {
                fill
            },
        )),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The face of a small button with no action, for a preview of how buttons
/// read: drawn as [`small_button_style`] at rest, not clickable.
pub(in crate::preferences) fn small_button_face<'a>(
    label: Localized,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let (fill, line) = (tokens::resolve(t).field, ti(t.border));
    container(text(label).size(TS.caption).color(ti(t.text)))
        .padding([0.0, SP.m])
        .height(Length::Fixed(SMALL_BUTTON_H))
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The glyph of a status line or banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::preferences) enum StatusGlyph {
    Warn,
    Info,
}

const GLYPH_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;
const GLYPH_INFO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>"##;

/// The two glyphs parsed once: warn, info.
static GLYPHS: LazyLock<[svg::Handle; 2]> = LazyLock::new(|| {
    [
        svg::Handle::from_memory(GLYPH_WARN),
        svg::Handle::from_memory(GLYPH_INFO),
    ]
});

/// `glyph` drawn in `ink`.
pub(in crate::preferences) fn status_glyph<'a>(
    glyph: StatusGlyph,
    ink: Color,
) -> Element<'a, PrefMsg> {
    let handle = match glyph {
        StatusGlyph::Warn => GLYPHS[0].clone(),
        StatusGlyph::Info => GLYPHS[1].clone(),
    };
    svg(handle)
        .width(Length::Fixed(GLYPH))
        .height(Length::Fixed(GLYPH))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// A tone banner (`.banner`): the glyph in the tone's ink on its soft fill,
/// `body` beside it, button radius.
pub(in crate::preferences) fn banner<'a>(
    glyph: StatusGlyph,
    tone: PillTone,
    body: Element<'a, PrefMsg>,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let (fill, ink) = styles::pill_colors(t, tone);
    container(
        row![
            container(status_glyph(glyph, ink)).padding(Padding {
                top: 1.0,
                ..Padding::ZERO
            }),
            container(body).width(Length::Fill),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Start),
    )
    .padding([SP.m, px(10.0)])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_matches_the_mock_at_the_default_base() {
        for (got, mock) in [
            (CONTROL_COLUMN_W, 232.0),
            (PAGE_PAD_TOP, 18.0),
            (PAGE_PAD, 24.0),
            (ROW_GAP, 24.0),
            (ROW_PAD_Y, 10.0),
            (SECTION_TOP, 20.0),
        ] {
            assert!((got - mock).abs() < 1e-3, "{got} != {mock}");
        }
    }
}
