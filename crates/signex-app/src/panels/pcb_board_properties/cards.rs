//! Pieces of the Grid and Guide Manager cards (#2386): one card per grid or
//! guide, a header line (swatch, name, value, ⋯) over a muted line, and the
//! ⋯ menu's actions under them. Sized from the type and spacing scales so
//! nothing wraps in a 220 px dock.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Row, Space, container};
use iced::{Alignment, Element, Length, Theme, Widget as _};
use iced_aw::Wrap;
use signex_i18n::{Localized, ids};
use signex_widgets::{Label, button, text};

use super::super::form_look;
use super::super::properties_parameters::{chip_label, graphite_filter_chip};
use super::chrome::Form;
use super::{PanelMsg, SP};
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;

/// A card's name: one line, cut with an ellipsis when long.
pub(super) fn name<'a>(form: &Form<'a>, label: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    text(label)
        .size(TS.body)
        .color(form.ink())
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .width(Length::Fill)
        .boxed()
}

/// A muted one-line value on a card's second line.
pub(super) fn muted<'a>(form: &Form<'a>, label: impl Into<Label<'a>>) -> Element<'a, PanelMsg> {
    text(label)
        .size(TS.caption)
        .color(form.muted())
        .wrapping(Wrapping::None)
        .boxed()
}

/// The swatch slot of a card with no colour of its own.
pub(super) fn no_swatch<'a>() -> Element<'a, PanelMsg> {
    Space::new()
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .boxed()
}

/// The ⋯ button that opens or closes a card's menu.
pub(super) fn more<'a>(form: &Form<'a>, msg: Option<PanelMsg>) -> Element<'a, PanelMsg> {
    let shown = button(text(form.t(ids::PCB_BOARD_MORE)).size(TS.body))
        .padding([0.0, SP.s])
        .on_press_maybe(msg)
        .style(form_look::ghost_button);
    form.tip(shown.boxed(), form.t(ids::PCB_BOARD_MORE_TIP))
}

/// A small toggle pill: lit when `on`. `msg: None` draws it inert in the
/// paint `on` says (the board grid's Parts and Other, always on).
pub(super) fn pill<'a>(
    form: &Form<'a>,
    label: Localized,
    on: bool,
    msg: Option<PanelMsg>,
    why: Option<Localized>,
) -> Element<'a, PanelMsg> {
    let pill = button(chip_label(label.into(), on))
        .padding([0.0, SP.s])
        .on_press_maybe(msg)
        .style(move |theme: &Theme, status: button::Status| {
            graphite_filter_chip(theme, on, matches!(status, button::Status::Hovered))
        });
    match why {
        Some(why) => form.tip(pill.boxed(), why),
        None => pill.boxed(),
    }
}

/// A line of a card: its parts side by side, centred.
pub(super) fn line<'a>(parts: Vec<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    Row::with_children(parts)
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .boxed()
}

/// The ⋯ menu: its actions as small buttons, wrapping to a new line when the
/// dock is narrow. `None` greys an action.
pub(super) fn menu<'a>(
    form: &Form<'a>,
    actions: Vec<(Localized, Option<PanelMsg>)>,
) -> Element<'a, PanelMsg> {
    let mut wrap = Wrap::new().spacing(SP.xs).line_spacing(SP.xs);
    for (label, msg) in actions {
        wrap = wrap.push(form.action(label, msg, None));
    }
    container(wrap).padding([SP.xs, 0.0]).boxed()
}

/// A field with its label over it, for a card's value pairs.
pub(super) fn labelled<'a>(
    form: &Form<'a>,
    label: Localized,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    iced::widget::column![muted(form, label), value]
        .spacing(SP.xs)
        .width(Length::FillPortion(1))
        .boxed()
}
