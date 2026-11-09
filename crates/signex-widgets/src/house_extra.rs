//! House `span`, `rich_text`, `toggler`, `radio`, `combo_box` and the label
//! setters of `pick_list` and `text_editor` (#2056). Same rule as `house.rs`:
//! the text is a [`Label`], and `clippy.toml` bans the iced functions outside
//! these two modules.

use iced::widget;
use iced::widget::{
    ComboBox, PickList, Radio, TextEditor, Toggler, combo_box::State, text as text_mod,
};
use std::borrow::Borrow;

use crate::label::Label;

/// One span of [`rich_text`]: `span(raw("Save"))`.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap iced (#2056)"
)]
pub fn span<'a, Link>(label: impl Into<Label<'a>>) -> text_mod::Span<'a, Link> {
    widget::span(label.into())
}

/// Rich text over house [`span`]s.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap iced (#2056)"
)]
pub fn rich_text<'a, Link, Message, Theme>(
    spans: impl AsRef<[text_mod::Span<'a, Link>]> + 'a,
) -> text_mod::Rich<'a, Link, Message, Theme>
where
    Link: Clone + 'static,
    Theme: text_mod::Catalog + 'a,
{
    widget::rich_text(spans)
}

/// A toggler. Set its label with [`TogglerLabel::labelled`], never with iced's
/// `label`.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap iced (#2056)"
)]
pub fn toggler<'a, Message, Theme>(is_checked: bool) -> Toggler<'a, Message, Theme>
where
    Theme: widget::toggler::Catalog + 'a,
{
    widget::toggler(is_checked)
}

/// The label of a [`toggler`].
pub trait TogglerLabel<'a>: Sized {
    fn labelled(self, label: impl Into<Label<'a>>) -> Self;
}

impl<'a, Message, Theme> TogglerLabel<'a> for Toggler<'a, Message, Theme>
where
    Theme: widget::toggler::Catalog,
{
    #[allow(
        clippy::disallowed_methods,
        reason = "the house widgets wrap iced (#2056)"
    )]
    fn labelled(self, label: impl Into<Label<'a>>) -> Self {
        self.label(label.into())
    }
}

/// A radio button with a label.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap iced (#2056)"
)]
pub fn radio<'a, Message, Theme, V>(
    label: impl Into<Label<'a>>,
    value: V,
    selected: Option<V>,
    on_click: impl FnOnce(V) -> Message,
) -> Radio<'a, Message, Theme>
where
    Message: Clone,
    Theme: widget::radio::Catalog + 'a,
    V: Copy + Eq,
{
    let label: String = label.into().into_string();
    widget::radio(label, value, selected, on_click)
}

/// A combo box. The placeholder is a label.
#[allow(
    clippy::disallowed_methods,
    reason = "the house widgets wrap iced (#2056)"
)]
pub fn combo_box<'a, T, Message, Theme>(
    state: &'a State<T>,
    placeholder: impl Into<Label<'a>>,
    selection: Option<&T>,
    on_selected: impl Fn(T) -> Message + 'static,
) -> ComboBox<'a, T, Message, Theme>
where
    T: std::fmt::Display + Clone,
    Theme: widget::combo_box::Catalog + widget::text_input::Catalog + 'a,
{
    widget::combo_box(state, placeholder.into(), selection, on_selected)
}

/// The placeholder of a `pick_list`.
pub trait PickListPlaceholder<'a>: Sized {
    fn placeholder_label(self, label: impl Into<Label<'a>>) -> Self;
}

impl<'a, T, L, V, Message, Theme> PickListPlaceholder<'a> for PickList<'a, T, L, V, Message, Theme>
where
    T: PartialEq + Clone,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: widget::pick_list::Catalog,
{
    #[allow(
        clippy::disallowed_methods,
        reason = "the house widgets wrap iced (#2056)"
    )]
    fn placeholder_label(self, label: impl Into<Label<'a>>) -> Self {
        self.placeholder(label.into().into_string())
    }
}

/// The placeholder of a `text_editor`.
pub trait TextEditorPlaceholder<'a>: Sized {
    fn placeholder_label(self, label: impl Into<Label<'a>>) -> Self;
}

impl<'a, Parser, Message, Theme, Renderer> TextEditorPlaceholder<'a>
    for TextEditor<'a, Parser, Message, Theme, Renderer>
where
    Parser: iced::advanced::text::Parser,
    Theme: widget::text_editor::Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    #[allow(
        clippy::disallowed_methods,
        reason = "the house widgets wrap iced (#2056)"
    )]
    fn placeholder_label(self, label: impl Into<Label<'a>>) -> Self {
        self.placeholder(label.into())
    }
}
