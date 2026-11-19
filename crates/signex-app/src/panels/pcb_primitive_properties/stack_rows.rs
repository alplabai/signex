//! The rows the via and pad stacks share (#2401, #96, #97): the Simple /
//! Top-Middle-Bottom / Full Stack tiles, one size row per layer, the
//! "from type" tag, captions and the warn callout, and the one place the
//! panels reach the shared illustration, hover link and "Not stored yet"
//! group of the panel shell.

use iced::widget::{Row, column, container, row};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, ids};
use signex_types::pcb_stack::StackMode;
use signex_widgets::property_illustration::{FieldId, Illustration};
use signex_widgets::{Label, button, text};

use super::mask::greyed_unless;
use super::rows::{Look, Section, labelled_text, mm_field, pair, with_tip};
use super::{LayerSizeRow, PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, SP, TS};
use crate::panels::{PanelMsg, form_look};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, pill_colors};

/// The three stack tiles. While `live` a press picks the mode for every
/// selected item; otherwise they are inert with `tip` (set by a via type).
pub(super) fn mode_tiles<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    mode: Option<StackMode>,
    live: bool,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let options = [
        (StackMode::Simple, ids::PCB_PRIM_STACK_SIMPLE),
        (
            StackMode::TopMiddleBottom,
            ids::PCB_PRIM_STACK_TOP_MIDDLE_BOTTOM,
        ),
        (StackMode::Full, ids::PCB_PRIM_STACK_FULL),
    ];
    let cells = options
        .into_iter()
        .map(|(option, label)| super::linked::Segment {
            label: look.t(label),
            is_on: mode == Some(option),
            msg: (live && common.is_editable() && mode != Some(option)).then(|| {
                PcbPrimitiveMsg::StackModePicked {
                    target: common.target.clone(),
                    mode: option,
                }
            }),
        })
        .collect();
    let tiles = container(super::linked::segments(cells))
        .padding([SP.xs, super::rows::PAD_X])
        .width(Length::Fill)
        .boxed();
    with_tip(look, tiles, tip)
}

/// One row per layer size: a via's diameter, or a pad's x and y. Greyed
/// and inert unless `live`.
pub(super) fn size_rows<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    rows: &'a [LayerSizeRow],
    pad: bool,
    live: bool,
    s: &mut Section<'a>,
) {
    for size in rows {
        let x = mm_field(common, PrimitiveField::LayerSize(size.layer), size.x, look);
        let value = if pad {
            let y = mm_field(common, PrimitiveField::LayerSizeY(size.layer), size.y, look);
            pair(x, y)
        } else {
            x
        };
        let label = UserText::new(size.label.clone());
        if common.shows(&look.ctx.i18n, &size.label) {
            s.push_row(greyed_unless(look, live, labelled_text(look, label, value)));
        }
    }
}

/// `value` with a small trailing tag ("from type", "from drill").
pub(super) fn tagged<'a>(
    look: Look<'a>,
    value: Element<'a, PanelMsg>,
    tag: Option<MessageId>,
) -> Element<'a, PanelMsg> {
    let Some(tag) = tag else {
        return value;
    };
    row![
        container(value).width(Length::Fill),
        text(look.t(tag))
            .size(TS.dense)
            .color(resolve(look.tokens()).ink4)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// A muted caption under the rows, indented like their labels.
pub(super) fn caption<'a>(look: Look<'a>, words: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    container(
        text(words)
            .size(TS.dense)
            .color(look.muted)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .padding([SP.xs, super::rows::PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A link that cannot be followed yet: the disabled ink, with `tip`.
pub(super) fn later_link<'a>(
    look: Look<'a>,
    label: Localized,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let link = text(label)
        .size(TS.dense)
        .color(resolve(look.tokens()).ink4)
        .wrapping(iced::widget::text::Wrapping::Word);
    with_tip(look, link.boxed(), tip)
}

/// A warn callout: the warn tint behind `words`, with an optional action
/// button before them.
pub(super) fn warn_callout<'a>(
    look: Look<'a>,
    words: Localized,
    action: Option<(Localized, PcbPrimitiveMsg)>,
) -> Element<'a, PanelMsg> {
    let (fill, ink) = pill_colors(look.tokens(), PillTone::Warn);
    let mut line: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(SP.s).align_y(iced::Alignment::Center);
    if let Some((label, msg)) = action {
        line = line.push(
            button(text(label).size(TS.dense))
                .padding([SP.xs, SP.s])
                .style(form_look::ghost_button)
                .on_press(PanelMsg::PcbPrimitive(msg))
                .boxed(),
        );
    }
    line = line.push(
        text(words)
            .size(TS.dense)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::Word)
            .width(Length::Fill)
            .boxed(),
    );
    container(
        container(line)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                text_color: Some(ink),
                border: Border {
                    radius: Radii::GRAPHITE.button.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding([SP.xs, super::rows::PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A section's header pill: the accent tint behind `words`.
pub(super) fn pill<'a>(look: Look<'a>, words: Localized) -> Element<'a, PanelMsg> {
    container(text(words).size(TS.dense))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(look.tokens(), PillTone::Accent))
        .boxed()
}

/// The illustration under a group header, linked to the fields by hover
/// (the panel shell's `linked`).
pub(super) fn illustration<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    drawing: Illustration,
) -> Element<'a, PanelMsg> {
    super::linked::illustration(look, common, drawing)
}

/// `row`, highlighted while the illustration's `field` callout is hovered
/// and highlighting it while the pointer is over the row.
pub(super) fn linked<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    field: FieldId,
    row: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    super::linked::linked(look, common, field, row)
}

/// The dashed "Not stored yet" group, closed by default.
pub(super) fn unstored<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    super::unstored::group(look, common, rows)
}

/// A row of the "Not stored yet" group: its label, the value it would have
/// and why it is not stored.
pub(super) fn unstored_row<'a>(
    look: Look<'a>,
    label: MessageId,
    value: Localized,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    super::rows::labelled(look, label, super::rows::fixed_value(look, value, tip))
}

/// Rows stacked without a section: the body of a subsection.
pub(super) fn rows_column<'a>(rows: Vec<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    column(rows).spacing(SP.xs).width(Length::Fill).boxed()
}
