//! The results list: grouped by kind (Net Label / Text Note / Designator /
//! Value) with a count each, every row showing its text with the matches
//! highlighted and, in Replace mode, an include box and the result inline
//! (`VOUT → VOUT_3V3`); the sheet sits in a right column.

use std::ops::Range;

use iced::widget::{Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_engine::{FindKind, FindQuery};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, checkbox, raw, text};

use super::{SP, TS};
use crate::find_replace::{FindReplaceMsg, FindReplaceState};
use crate::styles::{ChromeMetrics, Radii, chrome_px, ti, tokens};

/// Width of the right-hand sheet column.
const SHEET_W: f32 = chrome_px(90.0 * 11.0 / 12.0);

/// The chrome mono face for net names, designators and values.
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// One result as the list shows it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResultRow {
    /// Its index in the state's results, for selection and the include box.
    pub index: usize,
    pub text: String,
    /// What else names it, such as the placement a designator was read at.
    pub extra: Option<String>,
    /// The sheet (or board) file it is on, without the extension.
    pub sheet: String,
    /// Drawn in the mono face: every kind but a text note.
    pub mono: bool,
}

/// The results of one kind, in list order.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResultGroup {
    /// The kind's name, as signex-engine words it (#2060).
    pub kind: &'static str,
    pub rows: Vec<ResultRow>,
}

fn file_stem(path: Option<&std::path::Path>) -> String {
    path.and_then(|p| p.file_stem())
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Push `row` into the group of `kind`, opening one on its first result.
fn push(groups: &mut Vec<ResultGroup>, kind: &'static str, row: ResultRow) {
    match groups.iter_mut().find(|g| g.kind == kind) {
        Some(group) => group.rows.push(row),
        None => groups.push(ResultGroup {
            kind,
            rows: vec![row],
        }),
    }
}

/// The groups the list shows, in the order the kinds first appear: a
/// board's hits on a board (#2403), else the sheet results.
pub(super) fn groups(state: &FindReplaceState) -> Vec<ResultGroup> {
    let mut groups = Vec::new();
    if let Some(board) = &state.board {
        let sheet = file_stem(Some(&board.path));
        for (index, hit) in board.hits.iter().enumerate() {
            let row = ResultRow {
                index,
                text: hit.text.clone(),
                extra: None,
                sheet: sheet.clone(),
                mono: true,
            };
            push(&mut groups, hit.kind.label(), row);
        }
        return groups;
    }
    for (index, found) in state.matches.iter().enumerate() {
        let row = ResultRow {
            index,
            text: found.hit.text.clone(),
            extra: found.placement_label.clone(),
            sheet: file_stem(found.sheet.as_deref()),
            mono: found.hit.kind != FindKind::TextNote,
        };
        push(&mut groups, found.hit.kind.label(), row);
    }
    groups
}

/// `text` cut into `(segment, is_match)` runs at `ranges`.
pub(super) fn segments<'t>(text: &'t str, ranges: &[Range<usize>]) -> Vec<(&'t str, bool)> {
    let mut out = Vec::new();
    let mut last = 0;
    for at in ranges {
        if at.start > last {
            out.push((&text[last..at.start], false));
        }
        out.push((&text[at.clone()], true));
        last = at.end;
    }
    if last < text.len() {
        out.push((&text[last..], false));
    }
    out
}

/// The list box: the groups, or the empty state.
pub(super) fn results<'a>(
    state: &'a FindReplaceState,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let groups = groups(state);
    let muted = tokens::muted_text(theme_tokens);
    let content: Element<'a, FindReplaceMsg> = if groups.is_empty() {
        container(
            text(t!(i18n, ids::FIND_NO_MATCHES))
                .size(TS.body)
                .color(muted),
        )
        .padding([SP.l, SP.l])
        .width(Length::Fill)
        .center_x(Length::Fill)
        .boxed()
    } else {
        // Compiled once per frame; a board's hits are not highlighted.
        let query = state.find_query().ok().filter(|_| state.board.is_none());
        let mut list = column![].width(Length::Fill);
        for (n, group) in groups.into_iter().enumerate() {
            list = list.push(group_header(&group, n == 0, theme_tokens, i18n));
            for row in group.rows {
                list = list.push(result_row(state, row, query.as_ref(), theme_tokens, i18n));
            }
        }
        scrollable(list)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(theme_tokens))
            .height(Length::Fill)
            .boxed()
    };
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(crate::styles::card(theme_tokens))
        .boxed()
}

/// A group's head: the kind and its count, and over the first group the
/// head of the sheet column.
fn group_header<'a>(
    group: &ResultGroup,
    first: bool,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let r = tokens::resolve(theme_tokens);
    let sheet_head: Element<'a, FindReplaceMsg> = if first {
        text(t!(i18n, ids::FIND_SHEET_COLUMN))
            .size(TS.caption)
            .color(r.ink3)
            .width(Length::Fixed(SHEET_W))
            .align_x(iced::alignment::Horizontal::Right)
            .boxed()
    } else {
        Space::new().width(Length::Fixed(SHEET_W)).boxed()
    };
    let rule = r.line_soft;
    let fill = r.sunken;
    container(
        row![
            // raw: the kind is named by signex-engine (#2060).
            text(raw(group.kind)).size(TS.caption).color(r.ink3),
            text(t!(i18n, ids::FIND_GROUP_COUNT, "count" => group.rows.len()))
                .size(TS.caption)
                .color(r.ink3)
                .font(mono()),
            Space::new().width(Length::Fill),
            sheet_head,
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            color: rule,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One result: include box (Replace mode, sheet results), the text with its
/// matches highlighted and the result after the arrow, the sheet on the
/// right. A click selects it and zooms to it.
fn result_row<'a>(
    state: &FindReplaceState,
    row_data: ResultRow,
    query: Option<&FindQuery>,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let r = tokens::resolve(theme_tokens);
    let index = row_data.index;
    let selected = state.selected_index == Some(index);
    let included = state.is_included(index);
    let boxes = state.replace_mode && state.board.is_none();
    let font = if row_data.mono {
        mono()
    } else {
        iced::Font::DEFAULT
    };
    let ink = if included || !boxes {
        ti(theme_tokens.text)
    } else {
        r.ink4
    };

    let mut line: Row<Element<'a, FindReplaceMsg>> = row![].align_y(Alignment::Center);
    let ranges = query.map(|q| q.ranges(&row_data.text)).unwrap_or_default();
    for (segment, hit) in segments(&row_data.text, &ranges) {
        let piece = text(UserText::new(segment.to_string()))
            .size(TS.body)
            .font(font)
            .wrapping(iced::widget::text::Wrapping::None);
        line = line.push(if hit && included {
            container(piece.color(r.accent_soft_ink))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(r.accent_soft)),
                    border: Border {
                        radius: Radii::GRAPHITE.small.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed()
        } else {
            piece.color(ink).boxed()
        });
    }
    if boxes
        && included
        && let Some(query) = query
    {
        let after = query.replace_all(&row_data.text, &state.replacement);
        if after != row_data.text {
            line = line.push(
                text(t!(i18n, ids::FIND_REPLACE_ARROW))
                    .size(TS.body)
                    .color(r.ink4)
                    .boxed(),
            );
            line = line.push(
                text(UserText::new(after))
                    .size(TS.body)
                    .font(font)
                    .color(ink)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .boxed(),
            );
        }
    }
    if let Some(extra) = row_data.extra {
        line = line.push(
            text(UserText::new(format!(" \u{00B7} {extra}")))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }

    let mut cells: Row<Element<'a, FindReplaceMsg>> =
        row![].spacing(SP.m).align_y(Alignment::Center);
    if boxes {
        cells = cells.push(
            checkbox(included)
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.body)
                .on_toggle(move |_| FindReplaceMsg::ResultIncludeToggled(index))
                .boxed(),
        );
    }
    cells = cells.push(container(line).width(Length::Fill).clip(true).boxed());
    cells = cells.push(
        text(UserText::new(row_data.sheet))
            .size(TS.caption)
            .color(r.ink3)
            .width(Length::Fixed(SHEET_W))
            .align_x(iced::alignment::Horizontal::Right)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    );

    let accent = ti(theme_tokens.accent);
    let hover = tokens::hover_overlay(theme_tokens);
    let selection = tokens::overlay(theme_tokens, tokens::Overlay::Pressed);
    button(cells)
        .width(Length::Fill)
        .height(Length::Fixed(ChromeMetrics::DEFAULT.row))
        .padding([0.0, SP.m])
        .on_press(FindReplaceMsg::SelectResult(index))
        .style(move |_: &Theme, status: button::Status| {
            let background = if selected {
                Some(Background::Color(selection))
            } else if status == button::Status::Hovered {
                Some(Background::Color(hover))
            } else {
                None
            };
            let mut border = Border::default();
            if selected {
                // The current result: a 2 px accent bar on its leading edge.
                border.color = accent;
                border.left = iced::border::Side::default().width(2.0);
            }
            button::Style {
                background,
                border,
                ..button::Style::default()
            }
        })
        .boxed()
}

/// The warning line under the list naming the sheets a search skipped.
pub(super) fn skipped_line<'a>(
    note: &'a str,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let warn = ti(theme_tokens.warning);
    row![
        text(t!(i18n, ids::FIND_SKIPPED_GLYPH))
            .size(TS.caption)
            .color(warn),
        // raw: the skipped-sheets note is worded by the project scan (#2061).
        text(raw(note.to_string())).size(TS.caption).color(warn),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}
