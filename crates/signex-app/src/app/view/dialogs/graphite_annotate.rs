//! Graphite controls of the Annotate dialog: the checkbox pip, the order
//! and lock toggles, the field box and the order-preview cell. Callers use
//! them only when `is_graphite(tokens)`.

use iced::widget::{Column, Row, Space, button, canvas, container, text};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::app::state::AnnotateOrder;
use crate::styles::{Radii, Spacing, TypeScale, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Read-only checkbox pip: accent fill with a check when on, an empty
/// field box with a strong hairline when off.
pub(super) fn checkbox_pip<'a, M: 'a>(theme_tokens: &ThemeTokens, on: bool) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let accent = tokens::accent_fill(theme_tokens);
    let side = TS.body + 2.0;
    let mark = if on { "\u{2713}" } else { " " };
    let (fill, edge) = if on {
        (accent, accent)
    } else {
        (r.field, r.line_strong)
    };
    container(
        text(mark)
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

/// One cell of the order-preview legend: sunken fill, hairline, mono ink.
pub(super) fn preview_cell(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = tokens::resolve(theme_tokens);
    let (fill, edge) = (r.sunken, r.line_soft);
    move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: 4.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
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

/// One sheet of the Annotate table.
pub(super) struct SheetRow {
    pub name: String,
    pub on: bool,
}

/// The sheets table card: sunken header (Schematic Sheet / Start / Add
/// Suffix), then one ruled row per sheet with a checkbox and a mono name.
pub(super) fn sheet_table<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    rows: Vec<SheetRow>,
    empty: &'static str,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let ink = crate::styles::ti(theme_tokens.text);
    let pad = [0.0, SP.m];
    let cols = |lead: Element<'a, M>,
                name: Element<'a, M>,
                start: Element<'a, M>,
                suffix: Element<'a, M>| {
        Row::<Element<'a, M>>::new()
            .push(lead)
            .push(name)
            .push(start)
            .push(suffix)
            .spacing(SP.m)
            .align_y(iced::Alignment::Center)
    };
    let cell = |s: &'static str, color: iced::Color, width: Length| -> Element<'a, M> {
        text(s).size(TS.caption).color(color).width(width).boxed()
    };
    let head = cols(
        Space::new().width(Length::Fixed(TS.body + 2.0)).boxed(),
        cell("Schematic Sheet", r.ink4, Length::Fill),
        cell("Start", r.ink4, Length::Fixed(70.0)),
        cell("Add Suffix", r.ink4, Length::Fixed(70.0)),
    );
    let sunken = r.sunken;
    let mut body: Column<Element<'a, M>> = Column::new().width(Length::Fill);
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
    let rule = move || -> Element<'a, M> {
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
    for sheet in rows {
        let name_ink = if sheet.on { ink } else { r.ink3 };
        let line = cols(
            checkbox_pip(theme_tokens, sheet.on),
            text(sheet.name)
                .size(TS.body)
                .color(name_ink)
                .font(mono())
                .width(Length::Fill)
                .boxed(),
            text("1")
                .size(TS.body)
                .color(r.ink3)
                .font(mono())
                .width(Length::Fixed(70.0))
                .boxed(),
            text("\u{2014}")
                .size(TS.body)
                .color(r.ink4)
                .font(mono())
                .width(Length::Fixed(70.0))
                .boxed(),
        );
        body = body.push(rule());
        body = body.push(
            container(line)
                .padding(pad)
                .height(Length::Fixed(TS.body + 2.0 * SP.m + SP.s))
                .align_y(iced::alignment::Vertical::Center)
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
