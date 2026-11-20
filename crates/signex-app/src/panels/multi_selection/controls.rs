//! Form controls the selection Properties forms share (#1699): the Graphite
//! segmented control with an optional "nothing lit" state and per-cell
//! tooltips, a section header with a count pill, the paired field row
//! (X | Y, W | H, Start | End) and the field-row hover link of an
//! illustration.

use super::super::*;
use crate::panels::form_look::{self, FormLook};
use crate::panels::{SelectionFormMsg, graphite::PROPERTY_FIELD_RADIUS};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::Row;
use signex_widgets::Label;
use signex_widgets::property_illustration::{FieldId, IllustrationMsg};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One cell of a [`segmented_cells`] control.
pub(in crate::panels) struct Cell<'a, V> {
    pub value: V,
    pub label: Label<'a>,
    pub tip: Option<signex_i18n::Localized>,
}

impl<'a, V> Cell<'a, V> {
    pub fn new(value: V, label: impl Into<Label<'a>>) -> Self {
        Self {
            value,
            label: label.into(),
            tip: None,
        }
    }

    pub fn tip(mut self, tip: signex_i18n::Localized) -> Self {
        self.tip = Some(tip);
        self
    }
}

/// A Graphite segmented control, `columns` cells to a line, on one sunken
/// frame. `selected: None` lights no cell: the values are mixed.
pub(in crate::panels) fn segmented_cells<'a, V>(
    cells: Vec<Cell<'a, V>>,
    selected: Option<V>,
    columns: usize,
    on_pick: impl Fn(V) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    let columns = columns.max(1);
    let mut lines: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs);
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.xs).width(Length::Fill);
    let mut in_line = 0;
    for Cell { value, label, tip } in cells {
        let active = selected == Some(value);
        let cell = button(
            text(label)
                .size(TS.body)
                .wrapping(iced::widget::text::Wrapping::None)
                .align_x(iced::alignment::Horizontal::Center)
                .width(Length::Fill),
        )
        .padding([SP.s, SP.xs])
        .width(Length::Fill)
        .on_press(on_pick(value))
        .style(move |theme: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            form_look::segment_cell(theme, active, hovered && !active)
        })
        .boxed();
        let cell = match tip {
            Some(tip) => signex_widgets::tooltip(
                cell,
                container(text(tip).size(TS.caption)).padding([SP.xs, SP.s]),
                signex_widgets::tooltip::Position::Bottom,
            )
            .style(form_look::value_tip)
            .boxed(),
            None => cell,
        };
        line = line.push(container(cell).width(Length::Fill).boxed());
        in_line += 1;
        if in_line == columns {
            lines = lines.push(line.boxed());
            line = Row::new().spacing(SP.xs).width(Length::Fill);
            in_line = 0;
        }
    }
    if in_line > 0 {
        lines = lines.push(line.boxed());
    }
    container(lines)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}

/// A label | control row in the Properties column layout.
pub(in crate::panels) fn control_row<'a>(
    label: impl Into<Label<'a>>,
    control: Element<'a, PanelMsg>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label, muted),
            container(control).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A small read-only pill holding a count, beside a section title.
pub(in crate::panels) fn count_pill<'a>(count: String) -> Element<'a, PanelMsg> {
    container(
        text(signex_i18n::UserText::new(count))
            .size(TS.caption)
            .font(iced::Font::MONOSPACE)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s])
    .style(|theme: &Theme| {
        let l = FormLook::of(theme);
        container::Style {
            background: Some(Background::Color(l.sunken)),
            text_color: Some(l.ink3),
            border: Border {
                radius: PROPERTY_FIELD_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    })
    .boxed()
}

/// A collapsible section like [`PanelContext::collapsible`], with `pill`
/// (a count) at the right end of its header.
pub(in crate::panels) fn counted_section<'a>(
    ctx: &'a PanelContext,
    key: &str,
    title: signex_i18n::MessageId,
    pill: Option<String>,
    content: impl FnOnce() -> Column<Element<'a, PanelMsg>>,
) -> Element<'a, PanelMsg> {
    let is_collapsed = ctx.collapsed_sections.contains(key);
    let ink = crate::styles::tokens::resolve(&ctx.tokens).ink4;
    let title = ctx.i18n.message_plain(title, &[]);
    let mut header = row![
        signex_widgets::chevron::chevron(!is_collapsed, TS.section, ink),
        // raw: the catalog title, upper-cased for the locale.
        text(raw(ctx.i18n.upper_for_display(&title)))
            .size(TS.section)
            .color(ink)
            .width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(pill) = pill {
        header = header.push(count_pill(pill));
    }
    let header = button(container(header).padding(iced::Padding {
        top: SP.l,
        right: SP.l,
        bottom: SP.s,
        left: SP.l,
    }))
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(key.to_string()))
    .style(|_: &Theme, _| iced::widget::button::Style::default());
    let mut col = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(header.boxed());
    if !is_collapsed {
        col = col.push(content().boxed());
    }
    col.boxed()
}

/// One half of a [`pair_row`]: the axis mark, the field and its unit.
pub(in crate::panels) struct PairField<'a> {
    pub axis: signex_i18n::Localized,
    pub value: String,
    pub unit: signex_i18n::Localized,
    pub on_input: Box<dyn Fn(String) -> PanelMsg + 'a>,
    pub on_submit: Option<PanelMsg>,
}

fn pair_half<'a>(half: PairField<'a>, muted: Color) -> Element<'a, PanelMsg> {
    let PairField {
        axis,
        value,
        unit,
        on_input,
        on_submit,
    } = half;
    let mut input = signex_widgets::text_input(signex_i18n::UserText::new(String::new()), value)
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .on_input(on_input)
        .style(form_look::text_input_style);
    if let Some(submit) = on_submit {
        input = input.on_submit(submit);
    }
    row![
        text(axis).size(TS.caption).color(muted),
        input,
        text(unit).size(TS.caption).color(muted),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .boxed()
}

/// Two values on one row (X | Y, W | H, Start | End).
pub(in crate::panels) fn pair_row<'a>(
    label: impl Into<Label<'a>>,
    first: PairField<'a>,
    second: PairField<'a>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    control_row(
        label,
        row![pair_half(first, muted), pair_half(second, muted)]
            .spacing(SP.s)
            .width(Length::Fill)
            .boxed(),
        muted,
    )
}

/// `row` reporting the pointer to the illustration hover link of `field`,
/// and drawn with the accent wash while the drawing's callout of `field`
/// is hovered.
pub(in crate::panels) fn hover_linked<'a>(
    ctx: &PanelContext,
    field: FieldId,
    row: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let lit = ctx.selection_form.hover.is_field_highlighted(&field);
    let wash = crate::styles::tokens::resolve(&ctx.tokens).accent_soft;
    let row = container(row)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: lit.then_some(Background::Color(wash)),
            ..container::Style::default()
        });
    let hovered = |message: IllustrationMsg| {
        PanelMsg::SelectionForm(SelectionFormMsg::IllustrationHovered(message))
    };
    iced::widget::mouse_area(row)
        .on_enter(hovered(IllustrationMsg::FieldHovered(field.clone())))
        .on_exit(hovered(IllustrationMsg::FieldLeft(field)))
        .boxed()
}

/// A property illustration, its hover mapped into the panel's messages.
pub(in crate::panels) fn illustration_view<'a>(
    ctx: &PanelContext,
    illustration: signex_widgets::property_illustration::Illustration,
) -> Element<'a, PanelMsg> {
    let drawing = signex_widgets::property_illustration::property_illustration(
        illustration,
        &ctx.selection_form.hover,
        &ctx.tokens,
        TS.caption,
    )
    .map(|m| PanelMsg::SelectionForm(SelectionFormMsg::IllustrationHovered(m)));
    container(drawing)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A caption under a row, inset like the field column so it stays inside
/// the dock.
pub(in crate::panels) fn inset_caption<'a>(
    caption: impl Into<Label<'a>>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    container(
        text(caption)
            .size(TS.caption)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .padding(iced::Padding {
        top: SP.xs,
        right: SP.l,
        bottom: SP.s,
        left: SP.l,
    })
    .width(Length::Fill)
    .boxed()
}
