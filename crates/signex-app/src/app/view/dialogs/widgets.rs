//! Shared modal-chrome primitives — backdrop wrapper, draggable / detached
//! headers, the close-X button, and the common button / section builders
//! every dialog family reaches for.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change.

use super::*;
use iced::widget::{Space, container, row};
use iced::{Background, Color, Element, Length, Theme};
use signex_widgets::{raw, text};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Gap kept between a modal and the window edge, in logical px.
pub(in crate::app::view) const MODAL_WINDOW_MARGIN: f32 = 24.0;

/// A modal's size: its `desired` size, capped so it fits the window with
/// [`MODAL_WINDOW_MARGIN`] to spare (#1227). iced lays out in logical px, so
/// a high OS scale or a user interface scale leaves fewer of them; a modal
/// hard-sized past that clips its footer buttons.
pub(in crate::app::view) fn fit_modal_size(
    window_size: (f32, f32),
    desired: (f32, f32),
) -> (f32, f32) {
    let cap = |window: f32, want: f32| want.min((window - 2.0 * MODAL_WINDOW_MARGIN).max(1.0));
    (cap(window_size.0, desired.0), cap(window_size.1, desired.1))
}

/// Card width for a modal that wants `px`: the width [`wrap_dialog`](Signex::wrap_dialog) centres on.
pub(in crate::app::view) fn modal_card_width(window_size: (f32, f32), px: f32) -> f32 {
    fit_modal_size(window_size, (px, 0.0)).0
}

/// Place `inner` over a backdrop with the scrim colour chosen by the caller
/// (the themed one from [`super::header::dialog_scrim`]).
pub(in crate::app::view) fn wrap_modal_scrim<'a>(
    inner: Element<'a, Message>,
    offset: (f32, f32),
    window_size: (f32, f32),
    modal_size: (f32, f32),
    scrim: Color,
) -> Element<'a, Message> {
    // Absolute top-left = centre + drag offset. The Translate widget (see
    // signex_widgets::Translate) passes full parent limits to the child and then
    // translates the child's layout node, so the modal keeps its fixed
    // width/height even when positioned partially off-screen. No clamp is
    // applied here — Altium lets modals drag completely off the client
    // area and the OS window edge is the only hard boundary. If the user
    // drags the modal fully outside the window, they can dismiss it with
    // Escape (see bootstrap key handler).
    let (dx, dy) = offset;
    let (ww, wh) = window_size;
    let (mw, mh) = modal_size;
    let centre_x = (ww - mw) * 0.5;
    let centre_y = (wh - mh) * 0.5;
    let left = centre_x + dx;
    let top = centre_y + dy;

    let backdrop: Element<'a, Message> = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(scrim)),
            ..container::Style::default()
        })
        .boxed();

    let positioned: Element<'a, Message> =
        signex_widgets::Translate::new(inner, (left, top)).into();

    iced::widget::stack![backdrop, positioned].boxed()
}

/// Wrap a header element in a mouse_area so pressing on it begins a modal
/// drag. Uses the last known mouse position as the drag anchor.
pub(in crate::app::view) fn draggable_header<'a>(
    header_content: Element<'a, Message>,
    modal: super::super::super::state::ModalId,
    last_mouse: (f32, f32),
) -> Element<'a, Message> {
    iced::widget::mouse_area(header_content)
        .on_press(Message::Overlay(OverlayMsg::ModalDragStart {
            modal,
            x: last_mouse.0,
            y: last_mouse.1,
        }))
        .boxed()
}

impl Signex {
    /// The click-outside layer for `on_dismiss`, then `card` as drawn (it
    /// positions itself).
    pub(in crate::app::view) fn dismiss_over<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
    ) -> Vec<Element<'a, Message>> {
        vec![Self::dismiss_layer(on_dismiss), card]
    }

    /// [`Self::dismiss_over`] with `card` placed at the absolute `at`.
    pub(in crate::app::view) fn dismiss_and_place<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
        at: (f32, f32),
    ) -> Vec<Element<'a, Message>> {
        Self::dismiss_over(on_dismiss, signex_widgets::Translate::new(card, at).boxed())
    }

    /// [`Self::dismiss_over`] with `card` centred in the window.
    pub(in crate::app::view) fn dismiss_and_centre<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
    ) -> Vec<Element<'a, Message>> {
        let centred = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .boxed();
        Self::dismiss_over(on_dismiss, centred)
    }

    /// `body` as the in-window modal `modal`: sized to `desired`, capped to
    /// fit the window (#1227), centred, then moved by the modal's drag offset.
    pub(in crate::app::view) fn wrap_dialog<'a>(
        &self,
        modal: crate::app::state::ModalId,
        body: Element<'a, Message>,
        desired: (f32, f32),
    ) -> Element<'a, Message> {
        let offset = self
            .ui_state
            .modal_offsets
            .get(&modal)
            .copied()
            .unwrap_or_default();
        let window = self.ui_state.window_size;
        let scrim = super::header::dialog_scrim(&self.document_state.panel_ctx.tokens);
        wrap_modal_scrim(body, offset, window, fit_modal_size(window, desired), scrim)
    }

    /// [`Self::wrap_dialog`] for a modal that is never dragged.
    pub(in crate::app::view) fn wrap_centred_dialog<'a>(
        &self,
        body: Element<'a, Message>,
        desired: (f32, f32),
    ) -> Element<'a, Message> {
        let window = self.ui_state.window_size;
        let scrim = super::header::dialog_scrim(&self.document_state.panel_ctx.tokens);
        wrap_modal_scrim(
            body,
            (0.0, 0.0),
            window,
            fit_modal_size(window, desired),
            scrim,
        )
    }

    /// `size.0` for the card in the main window, capped like [`fit_modal_size`] so
    /// the card is as wide as the width the dialog is centred on; the full
    /// width in the dialog's own window (the window is the frame).
    pub(in crate::app::view) fn dialog_width(
        &self,
        modal: super::super::super::state::ModalId,
        size: (f32, f32),
    ) -> Length {
        if self.modal_detached(modal) {
            Length::Fill
        } else {
            Length::Fixed(modal_card_width(self.ui_state.window_size, size.0))
        }
    }

    /// The header of `modal`'s body: the draggable in-window strip while the
    /// card paints in the main window, the empty slot once it lives in its own
    /// OS window (whose native title bar carries the title and the close).
    pub(in crate::app::view) fn modal_header<'a>(
        &self,
        header_content: Element<'a, Message>,
        modal: super::super::super::state::ModalId,
    ) -> Element<'a, Message> {
        if self.modal_detached(modal) {
            os_title_bar_slot()
        } else {
            draggable_header(header_content, modal, self.interaction_state.last_mouse_pos)
        }
    }
}

/// Header slot for a modal body shown in its own OS window. The window
/// carries a native title bar (title, close button, move and raise), so
/// the body draws no header of its own: an empty zero-height slot keeps
/// the surrounding column layout identical to the in-window form.
pub(crate) fn os_title_bar_slot<'a>() -> Element<'a, Message> {
    Space::new().height(0).boxed()
}

/// The title strip every app modal opens with: `title` on the left, the
/// close-X sending `on_close` on the right, on the modal header strip. Pass
/// it to [`Signex::modal_header`] (or `draggable_header`): the 44 px header
/// with the neutral close.
pub(crate) fn modal_title_header<'a, M: Clone + 'a>(
    title: impl iced::widget::text::IntoFragment<'a>,
    on_close: M,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, M> {
    let title: String = title.into_fragment().into_owned();
    super::header::graphite_header(
        title,
        None,
        None,
        on_close,
        super::header::DIALOG_HEADER_H,
        tokens,
    )
}

/// Subtle section divider used inside the BOM modal's properties
/// sidebar. Rendered as a 1-line label on a slightly tinted strip
/// so the panel reads as a stack of named sections.
pub(super) fn section_header(title: &str, muted: Color) -> Element<'_, Message> {
    container(
        row![
            text(raw("\u{25BE}")).size(TS.caption).color(muted),
            Space::new().width(6),
            text(raw(title.to_string())).size(TS.body).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6.0, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismiss_helpers_stack_the_backdrop_under_one_card() {
        let card = || Space::new().boxed();
        let msg = Message::noop;
        assert_eq!(Signex::dismiss_over(msg(), card()).len(), 2);
        assert_eq!(
            Signex::dismiss_and_place(msg(), card(), (3.0, 4.0)).len(),
            2
        );
        assert_eq!(Signex::dismiss_and_centre(msg(), card()).len(), 2);
    }
}
