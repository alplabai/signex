//! The pre-placement form (Tab while a placement tool is armed, #2532): the
//! values the next object is placed with. Location takes typed X / Y (a
//! wire's Start and End) that follow the cursor until typed; OK or Enter
//! places at the typed point.

mod data;
mod illustration;
mod position;
mod rows;
#[cfg(test)]
mod tests;

pub use data::{PrePlacementData, PrePlacementKind, PrePlacementStyle};
pub use position::{PlacementPosition, PositionField, PrePlacementPositionMsg, wire_length};

use iced::widget::{Column, container, row, scrollable};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::coord::Unit;
use signex_types::schematic::Point;
use signex_widgets::LengthFormat;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::property_illustration::{IllustrationHover, property_illustration};
use signex_widgets::text;

use super::{
    PanelContext, PanelMsg, PanelPalette, font_style_row, form_look, shape_fill_row, wire_color_row,
};
use crate::styles::{Spacing, TypeScale};
use rows::Look;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The font sizes the Font Size list offers, in points.
const FONT_SIZES_PT: [u32; 13] = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];

/// The text field a kind names, if it places text.
fn text_label(kind: PrePlacementKind) -> Option<signex_i18n::MessageId> {
    match kind {
        PrePlacementKind::NetLabel | PrePlacementKind::PowerPort => Some(ids::PROPERTIES_NET_NAME),
        PrePlacementKind::GlobalPort => Some(ids::PROPERTIES_PORT_NAME),
        PrePlacementKind::HierPort => Some(ids::PROPERTIES_SHEET_NAME),
        PrePlacementKind::TextNote => Some(ids::PROPERTIES_TEXT),
        PrePlacementKind::Component => Some(ids::PROPERTIES_VALUE),
        _ => None,
    }
}

/// A shape tool: Width and, for closed shapes, Fill.
fn is_shape(kind: PrePlacementKind) -> bool {
    matches!(
        kind,
        PrePlacementKind::Line
            | PrePlacementKind::Rectangle
            | PrePlacementKind::Circle
            | PrePlacementKind::Arc
            | PrePlacementKind::Polygon
    )
}

/// A tool with nothing to set but where it goes (junction, no-connect…).
pub(crate) fn has_no_options(kind: PrePlacementKind) -> bool {
    text_label(kind).is_none() && !is_shape(kind) && kind != PrePlacementKind::Wire
}

/// What a coordinate field shows: the text being typed, else `mm` (Y from
/// the page origin).
fn shown(
    pp: &PrePlacementData,
    ctx: &PanelContext,
    fmt: &LengthFormat<'_>,
    field: PositionField,
    mm: f64,
) -> String {
    pp.position.draft(field).map_or_else(
        || fmt.field_text(if field.is_y() { ctx.shown_y(mm) } else { mm }),
        str::to_owned,
    )
}

/// The Location fields as the form shows them, in [`PositionField`] order
/// (X, Y, End X, End Y), and the derived Length. Untyped fields show the
/// cursor (a wire's Start its anchor); Y counts from the page origin.
pub(crate) fn location_texts(pp: &PrePlacementData, ctx: &PanelContext) -> ([String; 4], String) {
    let fmt = LengthFormat::new(&ctx.i18n, Unit::Mm);
    let here = cursor(pp);
    let start = if pp.kind == PrePlacementKind::Wire {
        pp.position.live_start(here)
    } else {
        pp.position.start(here)
    };
    let end = pp.position.end(here);
    (
        [
            shown(pp, ctx, &fmt, PositionField::X, start.x),
            shown(pp, ctx, &fmt, PositionField::Y, start.y),
            shown(pp, ctx, &fmt, PositionField::EndX, end.x),
            shown(pp, ctx, &fmt, PositionField::EndY, end.y),
        ],
        fmt.field_text(wire_length(start, end)),
    )
}

/// The Tab form of the armed placement tool.
pub(super) fn view<'a>(pp: &PrePlacementData, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let look = Look::of(&ctx.tokens);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(rows::header(look, &ctx.tokens, i18n, &pp.tool_name));
    if let Some(drawing) = illustration_of(pp, i18n) {
        col = col.push(drawing_box(ctx, drawing, pp.kind == PrePlacementKind::Wire));
    }
    col = col.push(location(pp, ctx, look).boxed());
    col = col.push(properties(pp, ctx, look));
    container(
        scrollable(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(form_look::scrollbar_style),
    )
    .width(Length::Fill)
    .boxed()
}

/// The cursor, the point untyped coordinates follow.
fn cursor(pp: &PrePlacementData) -> Point {
    Point::new(pp.cursor_x_mm, pp.cursor_y_mm)
}

/// The illustration of a net label or wire tool; other tools have none.
fn illustration_of(
    pp: &PrePlacementData,
    i18n: &I18n,
) -> Option<signex_widgets::property_illustration::Illustration> {
    match pp.kind {
        PrePlacementKind::NetLabel => Some(illustration::net_label(
            i18n,
            &pp.label_text,
            (pp.justify_h, pp.justify_v),
        )),
        PrePlacementKind::Wire => {
            let (start, end) = (
                pp.position.live_start(cursor(pp)),
                pp.position.end(cursor(pp)),
            );
            Some(illustration::wire(
                i18n,
                ((start.x, start.y), (end.x, end.y)),
                pp.wire_width,
                pp.wire_color.map(crate::styles::ti),
            ))
        }
        _ => None,
    }
}

/// The illustration on the panel ground; the wire tool says where Length
/// comes from under it.
fn drawing_box<'a>(
    ctx: &'a PanelContext,
    drawing: signex_widgets::property_illustration::Illustration,
    is_wire: bool,
) -> Element<'a, PanelMsg> {
    let canvas =
        property_illustration(drawing, &IllustrationHover::default(), &ctx.tokens, TS.body)
            .map(|_| PanelMsg::Noop)
            .boxed();
    let mut body: Column<Element<'a, PanelMsg>> = Column::new().push(canvas);
    if is_wire {
        body = body.push(
            text(t!(ctx.i18n, ids::PROPERTIES_PRE_PLACEMENT_LENGTH_DERIVED))
                .size(TS.caption)
                .color(Look::of(&ctx.tokens).r.ink4)
                .boxed(),
        );
    }
    container(body)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// Location: Position X / Y (a wire's Start, End and Length) and Rotation.
fn location<'a>(
    pp: &PrePlacementData,
    ctx: &'a PanelContext,
    look: Look,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a I18n = &ctx.i18n;
    let is_wire = pp.kind == PrePlacementKind::Wire;
    let ([x, y, end_x, end_y], length) = location_texts(pp, ctx);
    let (start_text, end_text) = ((x, y), (end_x, end_y));
    let rotation = pp.rotation;
    let with_rotation = !has_no_options(pp.kind);
    ctx.collapsible(
        "preplace_location",
        ids::PANELS_PLACED_LOCATION,
        &ctx.collapsed_sections,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            let xy = (PositionField::X, PositionField::Y);
            if is_wire {
                let ends = (PositionField::EndX, PositionField::EndY);
                c = c
                    .push(rows::field(
                        look,
                        t!(i18n, ids::PROPERTIES_PRE_PLACEMENT_START),
                        rows::coordinate_pair(look, i18n, xy, start_text),
                    ))
                    .push(rows::field(
                        look,
                        t!(i18n, ids::PANELS_PLACED_END),
                        rows::coordinate_pair(look, i18n, ends, end_text),
                    ))
                    .push(rows::field(
                        look,
                        t!(i18n, ids::PANELS_PLACED_LENGTH),
                        rows::read_only_mm(
                            look,
                            &ctx.tokens,
                            i18n,
                            signex_i18n::UserText::new(length),
                        ),
                    ));
            } else {
                c = c.push(rows::field(
                    look,
                    t!(i18n, ids::PROPERTIES_POSITION),
                    rows::coordinate_pair(look, i18n, xy, start_text),
                ));
            }
            if with_rotation {
                c = c.push(rows::field(
                    look,
                    t!(i18n, ids::PANELS_PLACED_ROTATION),
                    rows::rotation(i18n, rotation),
                ));
            }
            c
        },
    )
}

/// The kind's own section: text and its look, a shape's Width and Fill, a
/// wire's Width and Color, or the card of a tool with no other options.
fn properties<'a>(
    pp: &PrePlacementData,
    ctx: &'a PanelContext,
    look: Look,
) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    if let Some(label) = text_label(pp.kind) {
        let label_text = pp.label_text.clone();
        let designator = pp.designator.clone();
        let is_component = pp.kind == PrePlacementKind::Component;
        let (font, size_pt, anchor) = (
            pp.font.clone(),
            pp.font_size_pt,
            (pp.justify_h, pp.justify_v),
        );
        return ctx
            .collapsible(
                "preplace_props",
                ids::PANELS_PLACED_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(rows::field(
                        look,
                        t!(i18n, label),
                        text_entry(label_text, PanelMsg::SetPrePlacementText),
                    ));
                    if is_component {
                        c = c.push(rows::field(
                            look,
                            t!(i18n, ids::PROPERTIES_DESIGNATOR),
                            text_entry(designator, PanelMsg::SetPrePlacementDesignator),
                        ));
                    }
                    c.push(rows::field(
                        look,
                        t!(i18n, ids::PROPERTIES_FONT),
                        iced::widget::pick_list(
                            Some(font),
                            crate::fonts::system_font_families().clone(),
                            |f: &String| f.clone(),
                        )
                        .graphite()
                        .on_select(PanelMsg::SetPrePlacementFont)
                        .padding([SP.xs, SP.s])
                        .width(Length::Fill)
                        .boxed(),
                    ))
                    .push(rows::field(
                        look,
                        t!(i18n, ids::PROPERTIES_FONT_SIZE),
                        font_size_row(size_pt, palette),
                    ))
                    .push(rows::field(
                        look,
                        t!(i18n, ids::PANELS_PROPERTIES_JUSTIFICATION),
                        rows::justification(look, i18n, anchor),
                    ))
                },
            )
            .boxed();
    }
    if is_shape(pp.kind) {
        let (width, fill, kind) = (pp.shape_width_mm, pp.shape_fill, pp.kind);
        let numeric_draft = ctx.numeric_draft.as_ref();
        let show_fill = !matches!(kind, PrePlacementKind::Line | PrePlacementKind::Arc);
        return ctx
            .collapsible(
                "preplace_shape",
                ids::PANELS_PLACED_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(super::form_edit_row_f64(
                        t!(i18n, ids::PROPERTIES_WIDTH_MM),
                        width,
                        super::NumericFieldKey::ShapeWidth,
                        numeric_draft,
                        palette.muted,
                    ));
                    if show_fill {
                        c = c.push(shape_fill_row(fill, palette.muted, palette.border, i18n));
                    }
                    c
                },
            )
            .boxed();
    }
    if pp.kind == PrePlacementKind::Wire {
        let (width, color, picker) = (pp.wire_width, pp.wire_color, ctx.wire_color_picker);
        return ctx
            .collapsible(
                "preplace_wire",
                ids::PANELS_PLACED_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    Column::new()
                        .spacing(0)
                        .width(Length::Fill)
                        .push(rows::field(
                            look,
                            t!(i18n, ids::PROPERTIES_WIRE_WIDTH),
                            rows::wire_width(i18n, width),
                        ))
                        .push(wire_color_row(
                            color,
                            picker,
                            std::rc::Rc::new(PanelMsg::SetPrePlacementWireColor),
                            palette.muted,
                            palette.border,
                            i18n,
                        ))
                },
            )
            .boxed();
    }
    rows::no_options(&ctx.tokens, i18n)
}

/// A text field of the form; Enter is OK.
fn text_entry<'a>(value: String, on_input: fn(String) -> PanelMsg) -> Element<'a, PanelMsg> {
    signex_widgets::text_input(signex_i18n::UserText::new(String::new()), value)
        .on_input(on_input)
        .on_submit(PanelMsg::ConfirmPrePlacement)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .style(form_look::text_input_style)
        .width(Length::Fill)
        .boxed()
}

/// Font Size and the B / I / U / T buttons on one row.
fn font_size_row<'a>(size_pt: u32, palette: PanelPalette) -> Element<'a, PanelMsg> {
    let sizes: Vec<String> = FONT_SIZES_PT.iter().map(u32::to_string).collect();
    row![
        iced::widget::pick_list(Some(size_pt.to_string()), sizes, |s: &String| s.clone())
            .graphite()
            .on_select(|s: String| PanelMsg::SetPrePlacementFontSize(s.parse().unwrap_or(10)))
            .padding([SP.xs, SP.s])
            .width(Length::FillPortion(2)),
        container(font_style_row(
            palette.muted,
            palette.primary,
            palette.input_bg,
            palette.input_bdr,
        ))
        .width(Length::FillPortion(3)),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}
