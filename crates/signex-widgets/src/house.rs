//! House `text`, `text_input`, `checkbox`, `button` and `tooltip` (#2056).
//!
//! They return the iced widget types, so every builder method keeps working.
//! What changes is the text: a label is a [`Label`], and the content of a
//! button or tooltip is a widget ([`Content`]), so a bare `&str` or `String`
//! no longer compiles. `clippy.toml` bans the raw iced functions outside this
//! module.
//!
//! The same-named iced modules (`button::Style`, `text::Shaping`, ...) are
//! re-exported in `lib.rs`, so `use signex_widgets::button;` brings both.

use iced::advanced::text::IntoFragment;
use iced::widget::{
    Button, Canvas, Checkbox, Column, Container, Grid, Image, MouseArea, Row, Scrollable, Space,
    Stack, Svg, Text, TextInput, Tooltip, canvas, container, svg, text as text_mod,
    tooltip as tooltip_mod,
};
use iced::{Element, widget};

use crate::label::Label;

/// A text widget: `text(raw("Save"))`, `text(t!(i18n, ids::MENU_FILE))`.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap the raw iced widgets (#2056)"
)]
pub fn text<'a, Theme>(label: impl Into<Label<'a>>) -> Text<'a, Theme>
where
    Theme: text_mod::Catalog + 'a,
{
    widget::text(label.into())
}

/// A text input. The placeholder is a label; the value is what the user typed.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap the raw iced widgets (#2056)"
)]
pub fn text_input<'a, Message, Theme>(
    placeholder: impl Into<Label<'a>>,
    value: impl IntoFragment<'a>,
) -> TextInput<'a, Message, Theme>
where
    Message: Clone,
    Theme: widget::text_input::Catalog + 'a,
{
    widget::text_input(placeholder.into(), value)
}

/// A checkbox. Set its label with [`CheckboxLabel::labelled`], never with
/// iced's `label`.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap the raw iced widgets (#2056)"
)]
pub fn checkbox<'a, Message, Theme, Renderer>(
    is_checked: bool,
) -> Checkbox<'a, Message, Theme, Renderer>
where
    Theme: widget::checkbox::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer,
{
    widget::checkbox(is_checked)
}

/// The label of a [`checkbox`].
pub trait CheckboxLabel<'a>: Sized {
    fn labelled(self, label: impl Into<Label<'a>>) -> Self;
}

impl<'a, Message, Theme, Renderer> CheckboxLabel<'a> for Checkbox<'a, Message, Theme, Renderer>
where
    Theme: widget::checkbox::Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    #[allow(
        clippy::disallowed_methods,
        reason = "the house widgets wrap the raw iced widgets (#2056)"
    )]
    fn labelled(self, label: impl Into<Label<'a>>) -> Self {
        self.label(label.into())
    }
}

/// A button around a widget: `button(text(raw("Save")))`.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap the raw iced widgets (#2056)"
)]
pub fn button<'a, Message, W, Theme>(content: W) -> Button<'a, Message, W, Theme>
where
    W: Content,
    Theme: widget::button::Catalog + 'a,
{
    widget::button(content)
}

/// A tooltip over a widget. The tip is a widget built from house text.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap the raw iced widgets (#2056)"
)]
pub fn tooltip<'a, W, V, Theme>(
    content: W,
    tip: V,
    position: tooltip_mod::Position,
) -> Tooltip<'a, W, V, Theme>
where
    W: Content,
    V: Content,
    Theme: container::Catalog + 'a,
{
    widget::tooltip(content, tip, position)
}

/// A widget the house `button` and `tooltip` accept as content. Everything
/// iced can lay out is listed except a bare `&str`, which is a widget in this
/// iced and would let `button("Save")` skip [`Label`].
pub trait Content {}

impl<M, T, R> Content for Element<'_, M, T, R> {}
impl<W> Content for Row<W> {}
impl<W> Content for Column<W> {}
impl<W> Content for Stack<W> {}
impl<W> Content for Grid<W> {}
impl<W, T: container::Catalog> Content for Container<'_, W, T> {}
impl<T: text_mod::Catalog> Content for Text<'_, T> {}
impl<T: svg::Catalog> Content for Svg<'_, T> {}
impl<H> Content for Image<H> {}
impl Content for Space {}
impl<M, W> Content for MouseArea<'_, M, W> {}
impl<M, W, T: widget::scrollable::Catalog> Content for Scrollable<'_, M, W, T> {}
impl<M, W, T: widget::button::Catalog> Content for Button<'_, M, W, T> {}
impl<W, V, T: container::Catalog> Content for Tooltip<'_, W, V, T> {}
impl<P, M, T, R> Content for Canvas<P, M, T, R>
where
    R: iced::advanced::graphics::geometry::Renderer,
    P: canvas::Program<M, T, R>,
{
}
