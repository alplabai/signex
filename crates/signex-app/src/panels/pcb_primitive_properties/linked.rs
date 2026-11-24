//! The illustration of a primitive panel and the rows linked to it (#2523):
//! hovering a row lights its callouts and a hovered callout lights its row
//! (as drawn in the mockup: the label in the accent text ink, a 1 px
//! accent ring round the value). Also the small controls the illustrated
//! rows share: a collapsible section header, a segmented row, the Ends row
//! and a field with an X / Y / Start / End prefix.

use iced::widget::{Space, container, mouse_area, row};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, ids};
use signex_widgets::property_illustration::{
    Cap, FieldId, Illustration, IllustrationMsg, property_illustration,
};
use signex_widgets::{FitRow, Label, button, text};

use super::rows::{Look, PAD_X, input, labelled, with_tip};
use super::{PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, SP, TS, format_number};
use crate::panels::graphite::PROPERTY_FIELD_RADIUS;
use crate::panels::{PanelMsg, form_look};
use crate::styles::{HAIRLINE, tokens::resolve};

/// The illustration under the header: full width, inset like the rows, its
/// text in the caption size. Hover goes to the panel's hover state.
pub(super) fn illustration<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    drawing: Illustration,
) -> Element<'a, PanelMsg> {
    let drawn = property_illustration(drawing, &common.hover, look.tokens(), TS.caption)
        .map(|m| PanelMsg::PcbPrimitive(PcbPrimitiveMsg::IllustrationHovered(m)))
        .boxed();
    container(drawn)
        .padding(Padding {
            top: SP.xs,
            right: PAD_X,
            bottom: SP.s,
            left: PAD_X,
        })
        .width(Length::Fill)
        .boxed()
}

/// `row` linked to the illustration's `field`: entering it lights the
/// callouts of `field`, leaving it puts them out.
pub(super) fn linked<'a>(
    _look: Look<'a>,
    _common: &'a PrimitiveCommon,
    field: FieldId,
    row: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let hovered = |msg| PanelMsg::PcbPrimitive(PcbPrimitiveMsg::IllustrationHovered(msg));
    mouse_area(row)
        .on_enter(hovered(IllustrationMsg::FieldHovered(field.clone())))
        .on_exit(hovered(IllustrationMsg::FieldLeft(field)))
        .boxed()
}

/// Whether the row of `field` is lit (its callout or the row is hovered).
pub(super) fn is_hot(common: &PrimitiveCommon, field: &FieldId) -> bool {
    common.hover.is_field_highlighted(field)
}

/// `look` with the label ink of a lit row.
pub(super) fn hot_look<'a>(look: Look<'a>, hot: bool) -> Look<'a> {
    if hot {
        Look {
            muted: resolve(look.tokens()).accent_text,
            ..look
        }
    } else {
        look
    }
}

/// `value` in a ring that turns the accent while its row is lit. The ring
/// is always there, transparent when unlit, so lighting moves nothing.
pub(super) fn ring<'a>(
    look: Look<'a>,
    hot: bool,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let accent = crate::styles::ti(look.tokens().accent);
    container(value)
        .padding(HAIRLINE)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: if hot { accent } else { Color::TRANSPARENT },
                radius: (PROPERTY_FIELD_RADIUS + HAIRLINE).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A labelled row linked to `field`: lit label and ringed value while hot.
pub(super) fn linked_row<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    field: FieldId,
    label: MessageId,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let hot = is_hot(common, &field);
    let row = labelled(hot_look(look, hot), label, ring(look, hot, value));
    linked(look, common, field, row)
}

/// The header of a collapsible section: chevron, title, and on the right
/// the summary. A press folds or unfolds it under `key`.
pub(super) fn section_header<'a>(
    look: Look<'a>,
    title: Localized,
    is_open: bool,
    summary: Option<Label<'a>>,
    key: String,
) -> Element<'a, PanelMsg> {
    let faint = resolve(look.tokens()).ink4;
    let mut line = row![
        signex_widgets::chevron::chevron(is_open, TS.section, faint),
        crate::panels::widgets::section_title(title, look.tokens(), &look.ctx.i18n),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    if let Some(summary) = summary {
        line = line.push(
            text(summary)
                .size(TS.caption)
                .color(faint)
                .font(iced::Font::MONOSPACE)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    button(container(line).padding([SP.s, PAD_X]).width(Length::Fill))
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key))
        .style(|_: &Theme, _| iced::widget::button::Style::default())
        .boxed()
}

/// One cell of a segmented row: its label, whether it is the chosen one,
/// and what a press sends (`None`: inert, drawn greyed unless chosen).
pub(super) struct Segment {
    pub label: Localized,
    pub is_on: bool,
    pub msg: Option<PcbPrimitiveMsg>,
}

/// A segmented row on the sunken frame: [`FitRow`] shares the width and
/// folds the cells rather than overlap their labels.
pub(super) fn segments<'a>(cells: Vec<Segment>) -> Element<'a, PanelMsg> {
    segment_row(cells.into_iter().map(|cell| (cell, false)).collect())
}

/// How a segment cell draws: chosen, live (pressable), disabled (faded as
/// a whole, as a disabled control is) or inert (greyed text unless chosen).
pub(super) fn segment_style(
    theme: &Theme,
    is_on: bool,
    is_live: bool,
    is_disabled: bool,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    if is_disabled && !is_on {
        return crate::styles::dimmed_button(form_look::segment_cell(theme, false, false));
    }
    let hovered = is_live && matches!(status, iced::widget::button::Status::Hovered);
    let mut style = form_look::segment_cell(theme, is_on, hovered && !is_on);
    if !is_live && !is_on {
        style.text_color = form_look::FormLook::of(theme).ink4;
    }
    style
}

/// [`segments`] with each cell marked disabled or not.
fn segment_row<'a>(cells: Vec<(Segment, bool)>) -> Element<'a, PanelMsg> {
    let mut line: Vec<Element<'a, PanelMsg>> = Vec::with_capacity(cells.len());
    for (Segment { label, is_on, msg }, is_disabled) in cells {
        let is_live = msg.is_some() && !is_disabled;
        let cell = button(
            text(label)
                .size(TS.caption)
                .align_x(iced::alignment::Horizontal::Center)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End),
        )
        .padding([SP.xs, SP.xs])
        .on_press_maybe(msg.filter(|_| is_live).map(PanelMsg::PcbPrimitive))
        .style(move |theme: &Theme, status| {
            segment_style(theme, is_on, is_live, is_disabled, status)
        });
        line.push(cell.boxed());
    }
    container(FitRow::new(line).spacing(SP.xs).fill(true))
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}

/// One end cap of the Ends row (#2523): the board stores no cap for a
/// track, an arc or a drawing, so Round is chosen and Square and Flat are
/// greyed until square and flat ends are decided for copper and stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndChoice {
    pub cap: Cap,
    pub is_on: bool,
    pub is_enabled: bool,
}

/// The Ends row's choices, Round first.
pub fn end_choices() -> [EndChoice; 3] {
    [Cap::Round, Cap::Square, Cap::Flat].map(|cap| EndChoice {
        cap,
        is_on: cap == Cap::Round,
        is_enabled: cap == Cap::Round,
    })
}

/// Why Square and Flat are greyed: on copper they wait for the copper
/// decision, on a drawing for a stored cap.
pub fn ends_tip(is_copper: bool) -> MessageId {
    if is_copper {
        ids::PCB_PRIM_ENDS_COPPER_TIP
    } else {
        ids::PCB_PRIM_ENDS_UNSTORED_TIP
    }
}

/// The Ends row's segments, Square and Flat faded as disabled (mockup rows
/// 95 and 98), with the reason they are greyed.
pub(super) fn ends<'a>(look: Look<'a>, is_copper: bool) -> Element<'a, PanelMsg> {
    let cells = end_choices()
        .into_iter()
        .map(|choice| {
            let cell = Segment {
                label: look.t(match choice.cap {
                    Cap::Round => ids::PCB_PRIM_ENDS_ROUND,
                    Cap::Square => ids::PCB_PRIM_ENDS_SQUARE,
                    Cap::Flat => ids::PCB_PRIM_ENDS_FLAT,
                }),
                is_on: choice.is_on,
                msg: None,
            };
            (cell, !choice.is_enabled)
        })
        .collect();
    with_tip(look, segment_row(cells), ends_tip(is_copper))
}

/// A field with a small prefix inside its frame ("X", "Start"), as the
/// two-value rows show their halves.
pub(super) fn prefixed<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    field: PrimitiveField,
    value: Option<f64>,
    prefix: MessageId,
) -> Element<'a, PanelMsg> {
    prefixed_text(look, common, field, value.map(format_number), prefix)
}

/// [`prefixed`] showing `value` as already formatted.
pub(super) fn prefixed_text<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    field: PrimitiveField,
    value: Option<String>,
    prefix: MessageId,
) -> Element<'a, PanelMsg> {
    let faint = resolve(look.tokens()).ink4;
    let bare = input(common, field, value, look)
        .padding([SP.s, 0.0])
        .style(|theme: &Theme, status| {
            let mut style = form_look::text_input_style(theme, status);
            style.background = Background::Color(Color::TRANSPARENT);
            style.border = Border::default();
            style
        });
    container(
        row![text(look.t(prefix)).size(TS.caption).color(faint), bare]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .padding([0.0, SP.s])
    .width(Length::Fill)
    .style(crate::styles::field(look.tokens(), false))
    .boxed()
}

/// Narrowest a prefixed field gets beside its partner: its padding, the
/// prefix and a six-character value, so X and Y share a row at the default
/// 300 px dock (mockup row 95).
const PREFIXED_FIELD_MIN: f32 = 2.0 * SP.s + SP.xs + TS.caption + TS.body * 3.4;

/// Two prefixed fields and the unit after them, on one row while each keeps
/// room for its value, else stacked with the unit beside them.
pub(super) fn two_fields<'a>(
    a: Element<'a, PanelMsg>,
    b: Element<'a, PanelMsg>,
    unit: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let fields = crate::panels::element_properties::rows::pair_line(a, b, PREFIXED_FIELD_MIN);
    row![container(fields).width(Length::Fill), unit]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// A point as one X / Y row in mm, three decimals (mockup rows 95 and 98).
pub(super) fn mm_point<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    (fx, fy): (PrimitiveField, PrimitiveField),
    (x, y): (Option<f64>, Option<f64>),
) -> Element<'a, PanelMsg> {
    use super::rows::fixed_mm;
    two_fields(
        prefixed_text(look, common, fx, x.map(fixed_mm), ids::PCB_PRIM_X),
        prefixed_text(look, common, fy, y.map(fixed_mm), ids::PCB_PRIM_Y),
        unit(look, ids::PCB_PRIM_UNIT_MM),
    )
}

/// A value and the unit after it.
pub(super) fn with_unit<'a>(
    value: Element<'a, PanelMsg>,
    unit: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![container(value).width(Length::Fill), unit]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// A length field with "mm" after it.
pub(super) fn mm<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    field: PrimitiveField,
    value: Option<f64>,
) -> Element<'a, PanelMsg> {
    mm_text(look, common, field, value.map(format_number))
}

/// [`mm`] showing `value` as already formatted.
pub(super) fn mm_text<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    field: PrimitiveField,
    value: Option<String>,
) -> Element<'a, PanelMsg> {
    super::rows::field(
        common,
        field,
        value,
        Some(look.t(ids::PCB_PRIM_UNIT_MM)),
        look,
    )
}

/// A read-only length, already formatted, with "mm" after it; N/A when
/// there is none.
pub(super) fn length<'a>(look: Look<'a>, value: Option<String>) -> Element<'a, PanelMsg> {
    let shown = match value {
        Some(v) => Label::from(signex_i18n::UserText::new(v)),
        None => Label::from(look.t(ids::PCB_PRIM_NOT_APPLICABLE)),
    };
    with_unit(read_only(look, shown), unit(look, ids::PCB_PRIM_UNIT_MM))
}

/// A round dot of a layer colour (the layer dropdown, a net's layer).
pub(super) fn dot<'a>(colour: Color) -> Element<'a, PanelMsg> {
    let side = SP.m + SP.xs / 2.0;
    container(Space::new())
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: (side / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A unit after a field ("mm", "°").
pub(super) fn unit<'a>(look: Look<'a>, id: MessageId) -> Element<'a, PanelMsg> {
    text(look.t(id))
        .size(TS.caption)
        .color(resolve(look.tokens()).ink4)
        .boxed()
}

/// A read-only value on the sunken fill, as Length and the pair lengths
/// read.
pub(super) fn read_only<'a>(look: Look<'a>, value: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    container(
        text(value)
            .size(TS.body)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([SP.xs, SP.s])
    .width(Length::Fill)
    .clip(true)
    .style(crate::styles::read_only_field(look.tokens()))
    .boxed()
}
