//! The Graphite list box of a dialog body: bordered rows with hairline rules
//! between them, a group heading per section, the kind code a row leads with
//! (as a key cap or as a file tile) and the soft "newer" pill.

use std::path::Path;

use iced::widget::{Column, Space, container, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use crate::app::file_extension::has_extension;
use crate::styles::{self, HAIRLINE, PillTone, Radii, Spacing, TypeScale, chrome_px, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Minimum height of a list row (30 px at the 12 px base).
pub(crate) const LIST_ROW_H: f32 = chrome_px(30.0 * 11.0 / 12.0);
/// Minimum height of a row that leads with a file tile (40 px).
pub(crate) const LIST_TILE_ROW_H: f32 = chrome_px(40.0 * 11.0 / 12.0);
/// Height of a group heading (24 px).
pub(crate) const LIST_GROUP_H: f32 = chrome_px(24.0 * 11.0 / 12.0);
/// Height of a key cap (18 px).
const CAP_H: f32 = chrome_px(18.0 * 11.0 / 12.0);
/// The file tile: 30 x 36 px, its code set at 8.5 px on the bottom edge.
const TILE_W: f32 = chrome_px(30.0 * 11.0 / 12.0);
const TILE_H: f32 = chrome_px(36.0 * 11.0 / 12.0);
const TILE_CODE: f32 = chrome_px(8.5 * 11.0 / 12.0);
/// Height of a pill (16 px).
const PILL_H: f32 = chrome_px(16.0 * 11.0 / 12.0);

/// The kind code of the document at `path`, by extension, as its tab shows
/// it: SCH, PCB, SYM, FPT or PRJ. `None` for any other file.
pub(super) fn kind_code(path: &Path) -> Option<MessageId> {
    [
        ("snxsch", ids::PANELS_TAB_KIND_SCHEMATIC),
        ("snxpcb", ids::PANELS_TAB_KIND_PCB),
        ("snxsym", ids::PANELS_TAB_KIND_SYMBOL),
        ("snxfpt", ids::PANELS_TAB_KIND_FOOTPRINT),
        ("snxprj", ids::PANELS_TAB_KIND_PROJECT),
    ]
    .into_iter()
    .find(|(ext, _)| has_extension(path, ext))
    .map(|(_, code)| code)
}

/// The mono face of kind codes, counts and paths.
fn mono() -> iced::Font {
    super::graphite::mono()
}

/// A kind code as a key cap: sunken fill, hairline border, mono ink3.
pub(super) fn kind_cap<'a, M: 'a>(
    i18n: &I18n,
    code: MessageId,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    container(
        text(t!(i18n, code))
            .size(TS.caption)
            .font(mono())
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.xs * 2.0])
    .height(Length::Fixed(CAP_H))
    .align_y(iced::alignment::Vertical::Center)
    .style(styles::key_cap(theme_tokens))
    .boxed()
}

/// A kind code as a file tile: a small sunken page with the code at its foot.
pub(super) fn kind_tile<'a, M: 'a>(
    i18n: &I18n,
    code: MessageId,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let (fill, ink, line) = (r.sunken, r.ink3, r.line_strong);
    container(
        text(t!(i18n, code))
            .size(TILE_CODE)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..mono()
            })
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(Length::Fixed(TILE_W))
    .height(Length::Fixed(TILE_H))
    .padding(iced::Padding {
        bottom: SP.s,
        ..iced::Padding::ZERO
    })
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Bottom)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The soft accent pill that marks the newer of two versions.
pub(super) fn accent_pill<'a, M: 'a>(
    label: impl Into<Label<'a>>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    container(
        text(label)
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.xs * 3.0])
    .height(Length::Fixed(PILL_H))
    .align_y(iced::alignment::Vertical::Center)
    .style(styles::pill(theme_tokens, PillTone::Accent))
    .boxed()
}

/// One list row: `cells` side by side, centred, at least `min_height` tall.
pub(super) fn list_row<'a, M: 'a>(cells: Vec<Element<'a, M>>, min_height: f32) -> Element<'a, M> {
    container(
        iced::widget::row(cells)
            .spacing(SP.m)
            .align_y(Alignment::Center)
            .width(Length::Fill),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(Length::Fixed(min_height))
    .align_y(iced::alignment::Vertical::Center)
    .boxed()
}

/// A group heading: the uppercase `title` on the sunken fill, its row count
/// at the right in mono.
pub(super) fn group_header<'a, M: 'a>(
    i18n: &I18n,
    title: &str,
    count: usize,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let fill = r.sunken;
    let heading = text(UserText::new(i18n.upper_for_display(title)))
        .size(TS.section)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .style(styles::section_label(theme_tokens))
        .wrapping(iced::widget::text::Wrapping::None);
    let count = text(UserText::new(i18n.count(count as i64)))
        .size(TS.section)
        .font(mono())
        .color(r.ink3);
    container(
        iced::widget::row![heading, Space::new().width(Length::Fill), count]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(Length::Fixed(LIST_GROUP_H))
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        ..container::Style::default()
    })
    .boxed()
}

/// A hairline rule between two list rows.
fn rule<'a, M: 'a>(theme_tokens: &ThemeTokens) -> Element<'a, M> {
    let line = tokens::resolve(theme_tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(HAIRLINE))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// The bordered list box holding `items` with a rule between each two; it
/// scrolls when the rows outgrow the height it is given.
pub(super) fn list_box<'a, M: 'a>(
    items: Vec<Element<'a, M>>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let border = styles::ti(theme_tokens.border);
    let mut column: Column<Element<'a, M>> = Column::new().width(Length::Fill);
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            column = column.push(rule(theme_tokens));
        }
        column = column.push(item);
    }
    container(
        scrollable(column)
            .height(Length::Shrink)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(HAIRLINE)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: border,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .clip(true)
    .boxed()
}

/// Height of a list box of `groups` headings and `rows` rows of
/// `row_height`, rules and border included.
pub(crate) fn list_box_height(groups: usize, rows: usize, row_height: f32) -> f32 {
    let items = groups + rows;
    groups as f32 * LIST_GROUP_H
        + rows as f32 * row_height
        + items.saturating_sub(1) as f32 * HAIRLINE
        + 2.0 * HAIRLINE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each document kind gets the code its tab shows; other files none.
    #[test]
    fn kind_codes_follow_the_extension() {
        let code = |p: &str| kind_code(Path::new(p));
        assert_eq!(code("a.snxsch"), Some(ids::PANELS_TAB_KIND_SCHEMATIC));
        assert_eq!(code("B.SNXPCB"), Some(ids::PANELS_TAB_KIND_PCB));
        assert_eq!(code("lib/s.snxsym"), Some(ids::PANELS_TAB_KIND_SYMBOL));
        assert_eq!(code("lib/f.snxfpt"), Some(ids::PANELS_TAB_KIND_FOOTPRINT));
        assert_eq!(code("p.snxprj"), Some(ids::PANELS_TAB_KIND_PROJECT));
        assert_eq!(code("notes.txt"), None);
        let en = I18n::en_us();
        let shown: Vec<String> = ["a.snxsch", "b.snxpcb", "s.snxsym", "f.snxfpt", "p.snxprj"]
            .iter()
            .filter_map(|p| code(p))
            .map(|id| en.message_plain(id, &[]))
            .collect();
        assert_eq!(shown, ["SCH", "PCB", "SYM", "FPT", "PRJ"]);
    }

    /// The box height counts headings, rows, the rules between them and the
    /// border.
    #[test]
    fn the_list_box_height_counts_rules_and_border() {
        assert_eq!(list_box_height(0, 0, LIST_ROW_H), 2.0 * HAIRLINE);
        assert_eq!(
            list_box_height(1, 2, LIST_ROW_H),
            LIST_GROUP_H + 2.0 * LIST_ROW_H + 4.0 * HAIRLINE
        );
    }
}
