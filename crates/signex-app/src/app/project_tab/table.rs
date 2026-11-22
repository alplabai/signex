//! The pieces the Project tab's table sections share (#1689-#1693): the
//! section frame (title, help), the plain heading and table rows, a muted
//! note, and the Graphite grid table of mockup rows 31-38 with its icon move
//! buttons. Pure view helpers in the Graphite look.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    Column, Row, Space, Stack, column, container, hover, mouse_area, row, scrollable,
};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::project::ListMove;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::look::{Glyph, ICON, glyph, icon_button, mono_font, ui_font};
use super::row_drag::GRIP_W;
use crate::app::Message;
use crate::styles::{HAIRLINE, Radii, Spacing, TypeScale, tokens as ink};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// Widest a section's content column grows, padding included (780 px).
const SECTION_MAX_W: f32 = TS.body * 65.0;
/// Widest a section's help line grows (about 72 characters).
const HELP_MAX_W: f32 = TS.body * 36.0;

/// Height of a grid table's heading row (28 px).
pub(in crate::app::project_tab) const HEAD_H: f32 = TS.body * 7.0 / 3.0;
/// Height of a grid table row (34 px).
pub(in crate::app::project_tab) const ROW_H: f32 = TS.body * 17.0 / 6.0;

/// A section: its title, a help line, then `body`, padded like the others.
pub(super) fn section<'a>(
    title: MessageId,
    help: MessageId,
    body: Vec<Element<'a, Message>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    frame(section_head(title, help, i18n, tokens), body, tokens)
}

/// [`section`] with `action` at the right of the title and help, aligned to
/// their top (`.chead .row-flex`).
pub(in crate::app::project_tab) fn section_with_action<'a>(
    title: MessageId,
    help: MessageId,
    action: Element<'a, Message>,
    body: Vec<Element<'a, Message>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let head = row![section_head(title, help, i18n, tokens), action]
        .spacing(SP.xl)
        .align_y(Alignment::Start)
        .width(Length::Fill)
        .boxed();
    frame(head, body, tokens)
}

/// The title (`.chead h3`) over the help line (`.chead p`).
fn section_head<'a>(
    title: MessageId,
    help: MessageId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    column![
        text(t!(i18n, title))
            .size(TS.heading)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(crate::styles::ti(tokens.text)),
        text(t!(i18n, help))
            .size(TS.body)
            .color(ink::resolve(tokens).ink3)
            .width(Length::Fill.max(HELP_MAX_W)),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
}

/// `page` in the vertical scroller every Project tab page but Variants sits
/// in, with the thin scrollbar. Its height is left to the content: the shell
/// puts a Fill space under it.
pub(in crate::app::project_tab) fn page_scroller<'a>(
    page: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    scrollable(page)
        .direction(crate::styles::thin_vertical())
        .style(crate::styles::scrollbar_style(tokens))
        .width(Length::Fill)
        .boxed()
}

/// The content column (`.cin`): `head`, then `body`, in a [`page_scroller`].
fn frame<'a>(
    head: Element<'a, Message>,
    body: Vec<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col: Column<Element<'a, Message>> =
        Column::new().push(head).spacing(SP.xl).width(Length::Fill);
    for part in body {
        col = col.push(part);
    }
    let padded = container(col)
        .width(Length::Fill.max(SECTION_MAX_W))
        .padding(iced::Padding {
            top: SP.xl + SP.s,
            right: SP.xl * 2.0,
            bottom: SP.xl * 2.0 + SP.s,
            left: SP.xl * 2.0,
        });
    page_scroller(padded.boxed(), tokens)
}

/// Secondary text: help, a state, a reason.
pub(super) fn note<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(ink::muted_text(tokens))
        .boxed()
}

/// A refusal reason, in the danger colour.
pub(super) fn problem<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(ink::danger(tokens))
        .boxed()
}

/// A cell of user text in the mono face (`td.m`): a file, a path.
pub(in crate::app::project_tab) fn mono_cell<'a>(
    value: impl Into<String>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    text(UserText::new(value.into()))
        .size(TS.body)
        .font(mono_font())
        .color(crate::styles::ti(tokens.text_secondary))
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .boxed()
}

/// A cell of catalog words.
pub(super) fn word_cell<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.body)
        .color(crate::styles::ti(tokens.text))
        .boxed()
}

/// Move up / Move down as two icon buttons with their tooltips, for a grid
/// table row's actions; the one at the list's end is dimmed and inert.
pub(in crate::app::project_tab) fn icon_move_buttons<'a>(
    index: usize,
    count: usize,
    moved: impl Fn(ListMove) -> Message,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let up = (index > 0).then(|| moved(ListMove::Up));
    let down = (index + 1 < count).then(|| moved(ListMove::Down));
    row![
        icon_button(Glyph::Up, t!(i18n, ids::PROJECT_MOVE_UP), up, tokens),
        icon_button(Glyph::Down, t!(i18n, ids::PROJECT_MOVE_DOWN), down, tokens),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// One row of a [`grid_table`]: a cell per column, the row's buttons (shown
/// while the pointer is over the row), whether it is a row in error, and its
/// drag grip, drawn over an empty first cell.
pub(in crate::app::project_tab) struct TableRow<'a> {
    pub cells: Vec<Element<'a, Message>>,
    pub actions: Option<Element<'a, Message>>,
    pub is_error: bool,
    pub grip: Option<RowGrip>,
}

/// What a row's drag grip sends, and whether a held row would drop here.
pub(in crate::app::project_tab) struct RowGrip {
    pub on_press: Message,
    pub on_enter: Message,
    pub is_drop_target: bool,
}

/// The Graphite table (`.tbl`): a bordered card with an uppercase heading
/// row on the sunken fill, then the rows over hairlines. Each column is
/// `(caption, width)`; `actions_width` adds an uncaptioned column at the
/// right whose buttons show on row hover.
pub(in crate::app::project_tab) fn grid_table<'a>(
    columns: &[(Option<MessageId>, Length)],
    actions_width: Option<Length>,
    rows: Vec<TableRow<'a>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let head = head_row(columns, actions_width, !rows.is_empty(), i18n, tokens);
    let mut table: Column<Element<'a, Message>> = Column::new().width(Length::Fill).push(head);
    let count = rows.len();
    let widths: Vec<Length> = columns.iter().map(|(_, width)| *width).collect();
    for (index, line) in rows.into_iter().enumerate() {
        let is_last = index + 1 == count;
        table = table.push(grid_row(line, &widths, actions_width, is_last, tokens));
    }
    let border = crate::styles::ti(tokens.border);
    container(table)
        .width(Length::Fill)
        .padding(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: border,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The heading row of a [`grid_table`] on the sunken fill: each column's
/// caption in capitals, with a hairline under it when rows follow.
fn head_row<'a>(
    columns: &[(Option<MessageId>, Length)],
    actions_width: Option<Length>,
    has_rows: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = ink::resolve(tokens);
    let mut head: Row<Element<'a, Message>> = Row::new().align_y(Alignment::Center);
    for (caption, width) in columns {
        let label: Element<'a, Message> = match caption {
            // The catalog word in capitals for the locale (`.tbl th`).
            Some(id) => text(UserText::new(
                i18n.upper_for_display(&t!(i18n, *id).into_string()),
            ))
            .size(TS.section)
            .font(ui_font(iced::font::Weight::Medium))
            .color(r.ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
            None => Space::new().boxed(),
        };
        head = head.push(cell_box(label, *width));
    }
    if let Some(width) = actions_width {
        head = head.push(Space::new().width(width).boxed());
    }
    let (sunken, line_soft) = (r.sunken, r.line_soft);
    let inner = Radii::GRAPHITE.card - HAIRLINE;
    let head_radius = if has_rows {
        iced::border::top(inner)
    } else {
        iced::border::radius(inner)
    };
    container(head)
        .width(Length::Fill)
        .height(Length::Fixed(HEAD_H))
        .align_y(Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            border: Border {
                color: line_soft,
                radius: head_radius,
                ..Border::default()
            }
            .bottom(hairline_if(has_rows)),
            ..container::Style::default()
        })
        .boxed()
}

/// A bottom hairline, or none.
fn hairline_if(drawn: bool) -> iced::border::Side {
    iced::border::Side::default().width(if drawn { HAIRLINE } else { 0.0 })
}

/// A cell of a grid table: `content` in its column's width, padded 0 10 px.
fn cell_box<'a>(content: Element<'a, Message>, width: Length) -> Element<'a, Message> {
    container(content)
        .width(width)
        .padding([0.0, SP.m])
        .align_y(Vertical::Center)
        .clip(true)
        .boxed()
}

/// One grid table row: the cells over the row fill (err_soft for a row in
/// error) with a hairline under it unless it is the last; while the pointer
/// is over it, the hover fill and the row's buttons are drawn on top, with
/// a space where each cell is so the cells are not drawn twice.
fn grid_row<'a>(
    line: TableRow<'a>,
    widths: &[Length],
    actions_width: Option<Length>,
    is_last: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = ink::resolve(tokens);
    let inner = Radii::GRAPHITE.card - HAIRLINE;
    let radius = if is_last {
        iced::border::bottom(inner)
    } else {
        iced::border::radius(0.0)
    };
    let fill = line.is_error.then_some(r.err_soft);
    let line_soft = r.line_soft;
    let mut cells: Row<Element<'a, Message>> =
        Row::new().height(Length::Fill).align_y(Alignment::Center);
    let mut overlay: Row<Element<'a, Message>> =
        Row::new().height(Length::Fill).align_y(Alignment::Center);
    for (cell, width) in line.cells.into_iter().zip(widths.iter().copied()) {
        cells = cells.push(cell_box(cell, width));
        overlay = overlay.push(Space::new().width(width).boxed());
    }
    if let Some(width) = actions_width {
        cells = cells.push(Space::new().width(width).boxed());
        let buttons: Element<'a, Message> = line.actions.unwrap_or_else(|| Space::new().boxed());
        overlay = overlay.push(
            container(buttons)
                .width(width)
                .padding([0.0, SP.s])
                .align_x(Horizontal::Right)
                .align_y(Vertical::Center)
                .boxed(),
        );
    }
    let base = container(cells)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_H))
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                color: line_soft,
                radius,
                ..Border::default()
            }
            .bottom(hairline_if(!is_last)),
            ..container::Style::default()
        });
    let shown = hover(base, hover_layer(overlay, radius, tokens)).boxed();
    match line.grip {
        Some(grip) => with_grip(shown, grip, tokens),
        None => shown,
    }
}

/// `shown` with its grip over the first column. The grip takes the press;
/// the whole row reports the pointer entering it, and a held row's drop
/// place is marked by an accent line along its top.
fn with_grip<'a>(
    shown: Element<'a, Message>,
    grip: RowGrip,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let handle = mouse_area(
        container(glyph(Glyph::Grip, ICON, ink::resolve(tokens).ink4))
            .center_x(Length::Fixed(GRIP_W))
            .center_y(Length::Fixed(ROW_H)),
    )
    .interaction(iced::mouse::Interaction::Grab)
    .on_press(grip.on_press);
    let grip_layer = row![handle, Space::new().width(Length::Fill)].width(Length::Fill);
    let mut layers: Stack<Element<'a, Message>> = Stack::new()
        .width(Length::Fill)
        .height(Length::Fixed(ROW_H))
        .push(shown)
        .push(grip_layer.boxed());
    if grip.is_drop_target {
        let accent = crate::styles::ti(tokens.accent);
        layers = layers.push(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(HAIRLINE * 2.0))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(accent)),
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    mouse_area(layers).on_enter(grip.on_enter).boxed()
}

/// The layer a grid row shows while the pointer is over it: the hover fill
/// under `overlay` (the row's buttons), with the row's corner `radius`.
fn hover_layer<'a>(
    overlay: Row<Element<'a, Message>>,
    radius: iced::border::Radius,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let hovered = ink::hover_overlay(tokens);
    container(overlay)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(hovered)),
            border: Border {
                radius,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
