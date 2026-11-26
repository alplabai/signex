//! Shared modal-chrome primitives — backdrop wrapper, draggable / detached
//! headers, the close-X button, and the common button builders every
//! dialog family reaches for.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change.

use super::*;
use iced::widget::{Space, container};
use iced::{Background, Color, Element, Length, Theme};

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

/// `card` centred in a window-filling container, claiming the pointer over
/// its own bounds.
fn centred<'a>(card: Element<'a, Message>) -> Element<'a, Message> {
    container(claim_pointer(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// `card` claiming the pointer over its own bounds, so a press on its padding
/// stops at it instead of reaching the click-outside layer below.
fn claim_pointer<'a>(card: Element<'a, Message>) -> Element<'a, Message> {
    iced::widget::mouse_area(card)
        .interaction(iced::mouse::Interaction::Idle)
        .boxed()
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
    /// The click-outside layer for `on_dismiss` under `card` as drawn (it
    /// positions itself), as ONE overlay layer. `collect_overlays` wraps each
    /// layer in `scroll_guard`, which claims the pointer over the whole
    /// window-sized card layer; a backdrop in a layer of its own below it
    /// never saw a click (#2349).
    pub(in crate::app::view) fn dismiss_over<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
    ) -> Vec<Element<'a, Message>> {
        vec![
            iced::widget::Stack::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .push(Self::dismiss_layer(on_dismiss))
                .push(card)
                .boxed(),
        ]
    }

    /// [`Self::dismiss_over`] with `card` placed at the absolute `at`.
    pub(in crate::app::view) fn dismiss_and_place<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
        at: (f32, f32),
    ) -> Vec<Element<'a, Message>> {
        Self::dismiss_over(
            on_dismiss,
            signex_widgets::Translate::new(claim_pointer(card), at).boxed(),
        )
    }

    /// An Active Bar flyout `card` at the absolute `at`, pulled in only as far
    /// as needed to stay inside the window (mockup row 61), as one overlay
    /// layer over [`Self::flyout_dismiss_layer`]. A right press on the card
    /// sends `on_card_right_press` and leaves the flyout open. The layer dims
    /// the window in the shared menu scrim, as every menu does.
    pub(in crate::app::view) fn flyout_and_place<'a>(
        on_dismiss: Message,
        on_right_press: Message,
        on_card_right_press: Message,
        card: Element<'a, Message>,
        at: (f32, f32),
    ) -> Vec<Element<'a, Message>> {
        let scrim = crate::app::view::menu_scrim();
        let card = iced::widget::mouse_area(claim_pointer(card))
            .on_right_press(on_card_right_press)
            .boxed();
        vec![
            iced::widget::Stack::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .push(Self::flyout_dismiss_layer(
                    on_dismiss,
                    on_right_press,
                    scrim,
                ))
                .push(crate::app::view::AnchoredMenu::new(card, at).boxed())
                .boxed(),
        ]
    }

    /// [`Self::dismiss_over`] with `card` centred in the window.
    pub(in crate::app::view) fn dismiss_and_centre<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
    ) -> Vec<Element<'a, Message>> {
        Self::dismiss_over(on_dismiss, centred(card))
    }

    /// [`Self::dismiss_and_centre`] over the theme's dialog scrim, the dim
    /// `wrap_dialog` draws under every other in-window dialog (#2757).
    pub(in crate::app::view) fn dismiss_and_centre_dialog<'a>(
        on_dismiss: Message,
        card: Element<'a, Message>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Vec<Element<'a, Message>> {
        let scrim = super::header::dialog_scrim(tokens);
        vec![
            iced::widget::Stack::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .push(Self::dismiss_layer_tinted(on_dismiss, scrim))
                .push(centred(card))
                .boxed(),
        ]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Backdrop and card share one layer, so the scroll guard that
    /// `collect_overlays` puts around each layer cannot hide the backdrop
    /// from a click (#2349).
    #[test]
    fn dismiss_helpers_stack_the_backdrop_under_one_card_in_one_layer() {
        let card = || Space::new().boxed();
        let msg = Message::noop;
        assert_eq!(Signex::dismiss_over(msg(), card()).len(), 1);
        assert_eq!(
            Signex::dismiss_and_place(msg(), card(), (3.0, 4.0)).len(),
            1
        );
        assert_eq!(Signex::dismiss_and_centre(msg(), card()).len(), 1);
    }

    /// A click outside the placed card dismisses; a click on the card's
    /// padding does not, even inside the scroll guard every overlay gets.
    #[test]
    fn a_click_outside_a_placed_card_dismisses_and_one_on_it_does_not() {
        use crate::layout_probe::messages_on_click_at;
        let window = iced::Size::new(400.0, 300.0);
        let click = |at: iced::Point| {
            let card = container(Space::new().width(100).height(60))
                .padding(10)
                .boxed();
            let layer = Signex::dismiss_and_place(Message::noop(), card, (50.0, 40.0))
                .pop()
                .expect("one layer");
            let guarded = super::super::super::overlays::scroll_guard(layer);
            messages_on_click_at(guarded, window, None, at)
        };
        let outside = click(iced::Point::new(300.0, 250.0));
        assert_eq!(outside.len(), 1, "{outside:?}");
        let on_padding = click(iced::Point::new(55.0, 45.0));
        assert!(on_padding.is_empty(), "{on_padding:?}");
    }
}
