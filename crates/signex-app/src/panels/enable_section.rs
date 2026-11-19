//! Enable-header subsection of a Properties form (epic #2523, BB3).
//!
//! The subsection header is its own enable checkbox ("Paste", "Thermal
//! Relief"). While it is off, the fields under it are greyed and take no
//! input: the disable-when-off rule. The fields keep their values, so
//! turning the switch back on restores them as they were.
//!
//! The panel owns the on/off state and the message the toggle sends; this
//! module only draws it.
#![allow(
    dead_code,
    reason = "a building block of epic #2523; the Properties panels adopt it in later units"
)]

use iced::widget::{Column, container, tooltip};
use iced::{Color, Element, Length, Padding, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel, Label, checkbox, text};

use super::form_look::{self, FormLook};
use crate::library::editor::inert::Inert;
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Alpha of the panel-coloured veil over the fields of a switched-off
/// subsection: the fields show through at 42 %, the mockup's greyed look.
pub(crate) const DEPENDENT_VEIL_ALPHA: f32 = 0.58;

/// A subsection whose header is its enable checkbox.
///
/// ```rust,ignore
/// EnableSection::new(t!(i18n, ids::PCB_PRIM_PASTE), pad.paste_enabled)
///     .on_toggle(|on| PanelMsg::PcbPrimitive(PcbPrimitiveMsg::PasteEnabledToggled(on)))
///     .view(tokens, paste_rows)
/// ```
pub(crate) struct EnableSection<'a, M> {
    label: Label<'a>,
    is_enabled: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M + 'a>>,
    tip: Option<Label<'a>>,
}

impl<'a, M: 'a> EnableSection<'a, M> {
    /// A subsection headed by `label`, switched on when `is_enabled`.
    /// Without [`Self::on_toggle`] the header is read-only.
    pub(crate) fn new(label: impl Into<Label<'a>>, is_enabled: bool) -> Self {
        Self {
            label: label.into(),
            is_enabled,
            on_toggle: None,
            tip: None,
        }
    }

    /// The message the header sends when it is ticked or cleared.
    pub(crate) fn on_toggle(mut self, on_toggle: impl Fn(bool) -> M + 'a) -> Self {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    /// A hover note on the header: what turning it off does.
    pub(crate) fn tip(mut self, tip: impl Into<Label<'a>>) -> Self {
        self.tip = Some(tip.into());
        self
    }

    /// The header above `body`, the subsection's dependent fields.
    pub(crate) fn view(
        self,
        tokens: &ThemeTokens,
        body: impl Into<Element<'a, M>>,
    ) -> Element<'a, M> {
        let header = header(self.label, self.is_enabled, self.on_toggle, tokens);
        let header = match self.tip {
            Some(tip) => with_tip(header, tip, tokens),
            None => header,
        };
        Column::new()
            .width(Length::Fill)
            .push(header)
            .push(gate_dependents(
                dependents_veil(tokens, self.is_enabled),
                body.into(),
            ))
            .boxed()
    }
}

/// The veil over the dependent fields: none while the subsection is on,
/// the panel colour at [`DEPENDENT_VEIL_ALPHA`] while it is off.
pub(crate) fn dependents_veil(tokens: &ThemeTokens, is_enabled: bool) -> Option<Color> {
    (!is_enabled).then(|| Color {
        a: DEPENDENT_VEIL_ALPHA,
        ..ti(tokens.panel_bg)
    })
}

/// `body` as given when there is no veil; otherwise greyed under `veil`
/// and deaf to clicks, typing, hover and focus.
fn gate_dependents<'a, M: 'a, T: 'a, R: iced::advanced::Renderer + 'a>(
    veil: Option<Color>,
    body: Element<'a, M, T, R>,
) -> Element<'a, M, T, R> {
    match veil {
        Some(veil) => Inert::new(body, veil).into(),
        None => body,
    }
}

/// Ink of the header label: the secondary text token, a step above the
/// muted plain subheads, so the switch reads as a control.
fn header_ink(tokens: &ThemeTokens) -> Color {
    ti(tokens.text_secondary)
}

/// The header row: the enable checkbox, labelled with the subsection name.
fn header<'a, M: 'a>(
    label: Label<'a>,
    is_enabled: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M + 'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let ink = header_ink(tokens);
    let mut check = checkbox::<M, Theme, iced::Renderer>(is_enabled)
        .labelled(label)
        .size(TS.body)
        .text_size(TS.caption)
        .spacing(SP.s + SP.xs)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .style(move |theme: &Theme, status| header_style(theme, status, ink));
    if let Some(on_toggle) = on_toggle {
        check = check.on_toggle(on_toggle);
    }
    container(check)
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: SP.xs,
            left: SP.l,
        })
        .width(Length::Fill)
        .boxed()
}

/// The form checkbox look with the header ink; a read-only header that is
/// ticked fills with the disabled ink, so it does not read as clickable.
fn header_style(theme: &Theme, status: checkbox::Status, ink: Color) -> checkbox::Style {
    let mut style = form_look::checkbox_style(theme, status);
    style.text_color = Some(ink);
    if let checkbox::Status::Disabled { is_checked: true } = status {
        let ink4 = FormLook::of(theme).ink4;
        style.background = iced::Background::Color(ink4);
        style.border.color = ink4;
    }
    style
}

/// `header` with `tip` under it on hover.
fn with_tip<'a, M: 'a>(
    header: Element<'a, M>,
    tip: Label<'a>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    signex_widgets::tooltip(
        header,
        container(text(tip).size(TS.body)).padding([SP.xs, SP.s]),
        tooltip::Position::Bottom,
    )
    .style(crate::styles::context_menu(tokens))
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn tokens() -> ThemeTokens {
        theme_tokens(ThemeId::GraphiteDark)
    }

    #[test]
    fn an_enabled_subsection_has_no_veil() {
        assert_eq!(dependents_veil(&tokens(), true), None);
    }

    #[test]
    fn a_disabled_subsection_veils_its_fields_in_the_panel_colour() {
        let t = tokens();
        let veil = dependents_veil(&t, false).expect("off subsection is veiled");
        let panel = ti(t.panel_bg);
        assert_eq!((veil.r, veil.g, veil.b), (panel.r, panel.g, panel.b));
        assert_eq!(veil.a, DEPENDENT_VEIL_ALPHA);
    }

    // The veil greys without hiding.
    const _: () = assert!(DEPENDENT_VEIL_ALPHA > 0.0 && DEPENDENT_VEIL_ALPHA < 1.0);

    #[test]
    fn the_header_reads_in_the_secondary_ink() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        assert_eq!(header_ink(&t), ti(t.text_secondary));
    }

    #[test]
    fn a_read_only_ticked_header_fills_with_the_disabled_ink() {
        let theme = Theme::Dark;
        let ink = Color::WHITE;
        let ink4 = FormLook::of(&theme).ink4;
        let style = header_style(&theme, checkbox::Status::Disabled { is_checked: true }, ink);
        assert_eq!(style.background, iced::Background::Color(ink4));
        assert_eq!(style.border.color, ink4);
        assert_eq!(style.text_color, Some(ink));
    }

    #[test]
    fn a_live_ticked_header_keeps_the_form_look() {
        let theme = Theme::Dark;
        let status = checkbox::Status::Active { is_checked: true };
        let style = header_style(&theme, status, Color::WHITE);
        let form = form_look::checkbox_style(&theme, status);
        assert_eq!(style.background, form.background);
        assert_eq!(style.border.color, form.border.color);
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod input_tests {
    use super::*;
    use iced::advanced::Layout;
    use iced::advanced::Widget as _;
    use iced::advanced::layout::Limits;
    use iced::advanced::shell::{Bus, Shell, Waker};
    use iced::advanced::widget::Tree;
    use iced::mouse::{self, Cursor};
    use iced::widget::Space;
    use iced::window::Headless;
    use iced::{Event, Point, Rectangle, Size};
    use signex_widgets::button;

    /// Click the middle of a dependent button under `veil` and return the
    /// messages it produced.
    fn click_dependent(veil: Option<Color>) -> Vec<u8> {
        let field: Element<'static, u8, Theme, ()> = button(Space::new().width(40).height(20))
            .on_press(7)
            .boxed();
        let mut element = gate_dependents(veil, field);
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(200.0, 100.0));
        element.layout(&mut tree, &(), &limits);
        let layout = Layout::new(tree.size);
        let viewport = Rectangle::with_size(Size::new(200.0, 100.0));
        let cursor = Cursor::Available(Point::new(5.0, 5.0));
        let mut messages = Bus::new();
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            let mut shell = Shell::new(&Headless, Waker::noop(), &mut messages);
            element.update(
                &mut tree,
                &event,
                layout,
                cursor,
                &(),
                &mut shell,
                &viewport,
            );
        }
        messages.drain().map(|(message, _)| message).collect()
    }

    #[test]
    fn dependents_of_an_enabled_subsection_take_clicks() {
        assert_eq!(click_dependent(None), vec![7]);
    }

    #[test]
    fn dependents_of_a_disabled_subsection_ignore_clicks() {
        let veil = dependents_veil(&theme_tokens_dark(), false);
        assert!(click_dependent(veil).is_empty());
    }

    fn theme_tokens_dark() -> ThemeTokens {
        signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark)
    }
}
