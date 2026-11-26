//! Graphite controls of the Annotate dialog: the checkbox pip, the order
//! and lock toggles, the field box, the order-preview cell and the sheets
//! table.

use iced::widget::{Column, Row, Space, canvas, container};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use crate::app::handlers::erc::{PlacementLabel, SheetTickRow, TickState, tick_state};
use crate::app::state::AnnotateOrder;
use crate::app::{AnnotateMsg, Message};
use crate::styles::{PillTone, Radii, Spacing, TypeScale, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Read-only checkbox pip: accent fill with a check when on, an empty
/// field box with a strong hairline when off.
pub(super) fn checkbox_pip<'a, M: 'a>(theme_tokens: &ThemeTokens, on: bool) -> Element<'a, M> {
    tick_pip(
        theme_tokens,
        if on { TickState::All } else { TickState::None },
    )
}

/// A checkbox pip in one of three states: ticked, unticked, or a dash for
/// a header over a mix.
pub(super) fn tick_pip<'a, M: 'a>(theme_tokens: &ThemeTokens, state: TickState) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let accent = tokens::accent_fill(theme_tokens);
    let side = TS.body + 2.0;
    let mark = match state {
        TickState::All => "\u{2713}",
        TickState::Some => "\u{2013}",
        TickState::None => " ",
    };
    let (fill, edge) = if state == TickState::None {
        (r.field, r.line_strong)
    } else {
        (accent, accent)
    };
    container(
        // raw: a tick glyph (or a blank), not language.
        text(raw(mark))
            .size(TS.caption)
            .color(tokens::on_accent(theme_tokens)),
    )
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: 4.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// Greyed checkbox pip for an option that cannot be changed yet: sunken
/// fill, soft hairline and a faint check, so it never reads as active.
pub(super) fn disabled_checkbox_pip<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    on: bool,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let side = TS.body + 2.0;
    let mark = if on { "\u{2713}" } else { " " };
    let (fill, edge) = (r.sunken, r.line_soft);
    // raw: a tick glyph (or a blank), not language.
    container(text(raw(mark)).size(TS.caption).color(r.ink4))
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Toggle-pill style (order choice, lock): selected takes the soft accent
/// fill, its ink and an accent hairline; unselected is a field box.
pub(super) fn toggle_style(
    theme_tokens: &ThemeTokens,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = tokens::resolve(theme_tokens);
    let accent = tokens::accent(theme_tokens);
    let ink = crate::styles::ti(theme_tokens.text);
    let hover = tokens::hover_overlay(theme_tokens);
    move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let (fill, edge, fg) = if selected {
            (r.accent_soft, accent, r.accent_soft_ink)
        } else if hovered {
            (hover, r.line_strong, ink)
        } else {
            (r.field, r.line_soft, ink)
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: fg,
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// Field box around a read-only value or a list: field fill, hairline.
pub(super) fn field_box(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    crate::styles::field(theme_tokens, false)
}

/// Bare button style: no fill, no border. The lock checkbox draws itself.
pub(super) fn bare_button_style(_: &Theme, _: button::Status) -> button::Style {
    button::Style::default()
}

/// Order diagram size, px: the mock's 76 x 52 card.
const DIAGRAM_W: f32 = 76.0;
const DIAGRAM_H: f32 = 52.0;
/// Dash pattern of the traversal line.
const DASH: &[f32] = &[3.0, 2.0];

/// The traversal of four parts, as slot indices (0 top-left, 1 top-right,
/// 2 bottom-left, 3 bottom-right), in numbering order.
fn traversal(order: AnnotateOrder) -> [usize; 4] {
    match order {
        AnnotateOrder::UpThenAcross => [2, 0, 3, 1],
        AnnotateOrder::DownThenAcross => [0, 2, 1, 3],
        AnnotateOrder::AcrossThenDown => [0, 1, 2, 3],
        AnnotateOrder::AcrossThenUp => [2, 3, 0, 1],
    }
}

/// Canvas program of the order diagram: four parts and the dashed accent
/// path that numbers them.
struct OrderDiagram {
    order: AnnotateOrder,
    ground: iced::Color,
    edge: iced::Color,
    part_fill: iced::Color,
    part_edge: iced::Color,
    accent: iced::Color,
}

impl<M> canvas::Program<M> for OrderDiagram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let whole = iced::Size::new(bounds.width, bounds.height);
        frame.fill_rectangle(iced::Point::ORIGIN, whole, self.ground);
        frame.stroke(
            &canvas::Path::rectangle(iced::Point::ORIGIN, whole),
            canvas::Stroke::default()
                .with_color(self.edge)
                .with_width(1.0),
        );
        // Part boxes at the mock's corners, scaled to the canvas.
        let (sx, sy) = (bounds.width / DIAGRAM_W, bounds.height / DIAGRAM_H);
        let (pw, ph) = (12.0 * sx, 10.0 * sy);
        let origin = |slot: usize| {
            let (x, y) = match slot {
                0 => (12.0, 9.0),
                1 => (50.0, 9.0),
                2 => (12.0, 33.0),
                _ => (50.0, 33.0),
            };
            iced::Point::new(x * sx, y * sy)
        };
        for slot in 0..4 {
            let at = origin(slot);
            let part = canvas::Path::rectangle(at, iced::Size::new(pw, ph));
            frame.fill(&part, self.part_fill);
            frame.stroke(
                &part,
                canvas::Stroke::default()
                    .with_color(self.part_edge)
                    .with_width(1.0),
            );
        }
        let centres: Vec<iced::Point> = traversal(self.order)
            .iter()
            .map(|&slot| {
                let at = origin(slot);
                iced::Point::new(at.x + pw / 2.0, at.y + ph / 2.0)
            })
            .collect();
        let path = canvas::Path::new(|b| {
            b.move_to(centres[0]);
            for point in &centres[1..] {
                b.line_to(*point);
            }
        });
        let mut stroke = canvas::Stroke::default()
            .with_color(self.accent)
            .with_width(1.4);
        stroke.line_dash = canvas::LineDash {
            segments: DASH,
            offset: 0,
        };
        frame.stroke(&path, stroke);
        // Arrowhead at the last point, along the last segment.
        let (from, to) = (centres[2], centres[3]);
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let len = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
        let (ux, uy) = (dx / len, dy / len);
        let tip = iced::Point::new(to.x - ux * pw * 0.5, to.y - uy * ph * 0.5);
        let head = 5.0;
        let arrow = canvas::Path::new(|b| {
            b.move_to(tip);
            b.line_to(iced::Point::new(
                tip.x - ux * head - uy * head * 0.5,
                tip.y - uy * head + ux * head * 0.5,
            ));
            b.line_to(iced::Point::new(
                tip.x - ux * head + uy * head * 0.5,
                tip.y - uy * head - ux * head * 0.5,
            ));
            b.close();
        });
        frame.fill(&arrow, self.accent);
        vec![frame.into_geometry()]
    }
}

/// The order-of-processing diagram: four parts joined by a dashed accent
/// path in the chosen traversal.
pub(super) fn order_diagram<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    order: AnnotateOrder,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let program = OrderDiagram {
        order,
        ground: r.sunken,
        edge: r.line_strong,
        part_fill: r.raised,
        part_edge: r.ink3,
        accent: tokens::accent(theme_tokens),
    };
    let diagram: canvas::Canvas<OrderDiagram, M> = canvas::Canvas::new(program);
    diagram
        .width(Length::Fixed(DIAGRAM_W))
        .height(Length::Fixed(DIAGRAM_H))
        .boxed()
}

/// Width of the sheets table's Start column: the mock's 54 px.
const START_COL_W: f32 = TS.body * 4.5;

/// A tick button over `pip`: sends `message` when the table is editable.
fn tick_button<'a>(pip: Element<'a, Message>, message: Option<Message>) -> Element<'a, Message> {
    button(pip)
        .padding(0.0)
        .style(bare_button_style)
        .on_press_maybe(message)
        .boxed()
}

/// One row's sheet cell: the mono file name, then the current-sheet pill or
/// the placement it stands for.
fn sheet_name_cell<'a>(
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
    sheet: &SheetTickRow,
) -> Element<'a, Message> {
    let r = tokens::resolve(theme_tokens);
    let ink = if sheet.ticked {
        crate::styles::ti(theme_tokens.text)
    } else {
        r.ink4
    };
    let mut cell = Row::<Element<'a, Message>>::new()
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .push(
            text(UserText::new(sheet.name.clone()))
                .size(TS.body)
                .color(ink)
                .font(mono())
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    if let Some(placement) = &sheet.placement {
        let label = match placement {
            PlacementLabel::Named(name) => UserText::new(format!("\u{b7} {name}")),
            PlacementLabel::Ordinal(n) => UserText::new(format!(
                "\u{b7} {}",
                t!(i18n, ids::DIALOG_ANNOTATE_SHEET_PLACEMENT, "n" => *n).into_string()
            )),
        };
        cell = cell.push(text(label).size(TS.body).color(r.ink4).font(mono()).boxed());
    }
    if sheet.is_current {
        let (fill, pill_ink) = crate::styles::pill_colors(theme_tokens, PillTone::Accent);
        cell = cell.push(
            container(
                text(t!(i18n, ids::DIALOG_ANNOTATE_SHEET_CURRENT))
                    .size(TS.caption)
                    .color(pill_ink),
            )
            .padding([0.0, SP.s])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    radius: Radii::GRAPHITE.pill.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
        );
    }
    cell.boxed()
}

/// The sheets table card: a sunken header (header tick / Schematic Sheet /
/// Start), then one ruled row per sheet or placement with its tick. A tick
/// sends the row's sheet file; every placement row of it follows.
pub(super) fn sheet_table<'a>(
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
    rows: &[SheetTickRow],
    editable: bool,
    empty: Localized,
) -> Element<'a, Message> {
    let r = tokens::resolve(theme_tokens);
    let cols =
        |lead: Element<'a, Message>, name: Element<'a, Message>, start: Element<'a, Message>| {
            Row::<Element<'a, Message>>::new()
                .push(lead)
                .push(name)
                .push(start)
                .spacing(SP.m)
                .align_y(iced::Alignment::Center)
        };
    let header_state = tick_state(rows);
    let header_tick = (editable && !rows.is_empty())
        .then(|| Message::Annotate(AnnotateMsg::AllSheetsTicked(header_state != TickState::All)));
    let head = cols(
        tick_button(tick_pip(theme_tokens, header_state), header_tick),
        text(t!(i18n, ids::DIALOG_ANNOTATE_SHEETS_COL_SHEET))
            .size(TS.caption)
            .color(r.ink4)
            .width(Length::Fill)
            .boxed(),
        text(t!(i18n, ids::DIALOG_ANNOTATE_SHEETS_COL_START))
            .size(TS.caption)
            .color(r.ink4)
            .width(Length::Fixed(START_COL_W))
            .align_x(iced::alignment::Horizontal::Right)
            .boxed(),
    );
    let sunken = r.sunken;
    let mut body: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    body = body.push(
        container(head)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(sunken)),
                ..container::Style::default()
            })
            .boxed(),
    );
    let rule_color = r.line_soft;
    let rule = move || -> Element<'a, Message> {
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(rule_color)),
                ..container::Style::default()
            })
            .boxed()
    };
    if rows.is_empty() {
        body = body.push(rule());
        body = body.push(
            container(text(empty).size(TS.body).color(r.ink3))
                .padding([SP.m, SP.m])
                .boxed(),
        );
    }
    let current_fill = tokens::selection_tint(theme_tokens);
    for sheet in rows {
        let toggle =
            editable.then(|| Message::Annotate(AnnotateMsg::SheetTickToggled(sheet.path.clone())));
        let line = cols(
            tick_button(checkbox_pip(theme_tokens, sheet.ticked), toggle),
            sheet_name_cell(theme_tokens, i18n, sheet),
            // raw: the start number is not language. A page numbering scheme
            // (#1292) starts each page at its block; else numbering runs on.
            text(raw(sheet.start.unwrap_or(1).to_string()))
                .size(TS.body)
                .color(r.ink4)
                .font(mono())
                .width(Length::Fixed(START_COL_W))
                .align_x(iced::alignment::Horizontal::Right)
                .boxed(),
        );
        let fill = sheet.is_current.then_some(current_fill);
        body = body.push(rule());
        body = body.push(
            container(line)
                .padding([0.0, SP.m])
                .width(Length::Fill)
                .height(Length::Fixed(TS.body + 2.0 * SP.m + SP.s))
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: fill.map(Background::Color),
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    container(body)
        .width(Length::Fill)
        .clip(true)
        .style(crate::styles::card(theme_tokens))
        .boxed()
}

/// Mono face of designators (the preview cells).
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}
