//! Small pieces both views of the Polygon Manager draw: the state chip, a
//! ghost button with its hint, a value cell and a dash.

use iced::widget::container;
use iced::widget::text::Wrapping;
use iced::widget::tooltip::Position;
use iced::{Element, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::{PourRow, PourState, SP, TS};
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::{PillTone, pill, ti, tokens::resolve};

/// The tone a state's chip takes.
fn tone(state: PourState) -> PillTone {
    match state {
        PourState::Poured => PillTone::Ok,
        PourState::Shelved => PillTone::Neutral,
        PourState::NotFilled => PillTone::Warn,
    }
}

/// A pour's state as a small chip.
pub(super) fn state_chip<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    state: PourState,
) -> Element<'a, PanelMsg> {
    container(
        text(t!(i18n, state.message()))
            .size(TS.caption)
            .wrapping(Wrapping::None),
    )
    .padding([0.0, SP.s])
    .style(pill(tokens, tone(state)))
    .boxed()
}

/// A ghost button with a hint under it; inert without a message.
pub(super) fn action<'a>(
    ctx: &'a PanelContext,
    label: MessageId,
    on_press: Option<PanelMsg>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let face = button(text(t!(ctx.i18n, label)).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press_maybe(on_press);
    hint_tooltip(
        face.boxed(),
        t!(ctx.i18n, tip),
        &ctx.tokens,
        Position::Bottom,
    )
}

/// A value read from the board, not typed.
pub(super) fn value<'a>(ctx: &PanelContext, shown: String) -> Element<'a, PanelMsg> {
    text(UserText::new(shown))
        .size(TS.dense)
        .color(ti(ctx.tokens.text))
        .wrapping(Wrapping::None)
        .boxed()
}

/// A dash where there is nothing to show.
pub(super) fn dash<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    text(t!(ctx.i18n, ids::PCB_STACK_NONE))
        .size(TS.dense)
        .color(resolve(&ctx.tokens).ink4)
        .boxed()
}

/// A pour's net, or No Net.
pub(super) fn net_label(i18n: &I18n, row: &PourRow) -> String {
    if row.net.is_empty() {
        t!(i18n, ids::PCB_PRIM_NO_NET).into_string()
    } else {
        row.net.clone()
    }
}

/// An area in mm², two places: an engineering value, `.` in every locale.
pub(super) fn area_text(mm2: f64) -> String {
    format!("{mm2:.2}")
}
