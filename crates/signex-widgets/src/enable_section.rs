//! Enable-header subsection of a Properties form (epic #2523, BB3).
//!
//! The subsection header is its own enable checkbox ("Paste", "Thermal
//! Relief"). While it is off, the fields under it are greyed and take no
//! input: the disable-when-off rule. The fields keep their values, so
//! turning the switch back on restores them as they were.
//!
//! The panel owns the on/off state and the message the toggle sends; this
//! module only draws it. The caller passes its form look in
//! [`EnableSectionStyle`].

use iced::widget::{Column, Row, container};
use iced::{Color, Element, Font, Length, Padding, Theme, Widget as _};

use crate::inert::Inert;
use crate::theme_ext::{DISABLED_OPACITY, Spacing, TypeScale};
use crate::{CheckboxLabel, Label, checkbox, text, tooltip};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Alpha of the panel-coloured veil over the fields of a switched-off
/// subsection: the fields show through at [`DISABLED_OPACITY`], like any
/// disabled control.
pub const DEPENDENT_VEIL_ALPHA: f32 = 1.0 - DISABLED_OPACITY;

/// A checkbox style function, boxed so the caller can capture its tokens.
pub type CheckboxLook<'a> = Box<dyn Fn(&Theme, checkbox::Status) -> checkbox::Style + 'a>;

/// The caller's form look for an [`EnableSection`].
pub struct EnableSectionStyle<'a> {
    /// Ink of the header label: the secondary text token, a step above the
    /// muted plain subheads, so the switch reads as a control.
    pub header_ink: Color,
    /// Fill of a read-only header that is ticked (the disabled ink), so it
    /// does not read as clickable.
    pub disabled_ink: Color,
    /// The panel colour the veil over switched-off fields is made of.
    pub panel: Color,
    /// Face of the header label; it is drawn at medium weight.
    pub font: Font,
    /// The form checkbox look the header starts from.
    pub checkbox: CheckboxLook<'a>,
    /// Frame of the header's hover note.
    pub tip: Box<dyn Fn(&Theme) -> container::Style + 'a>,
}

/// A subsection whose header is its enable checkbox.
///
/// ```rust,ignore
/// EnableSection::new(t!(i18n, ids::PCB_PRIM_PASTE), pad.paste_enabled)
///     .on_toggle(|on| PanelMsg::PcbPrimitive(PcbPrimitiveMsg::PasteEnabledToggled(on)))
///     .view(enable_section_style(tokens), paste_rows)
/// ```
pub struct EnableSection<'a, M> {
    label: Label<'a>,
    is_enabled: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M + 'a>>,
    tip: Option<Label<'a>>,
    is_live_while_off: bool,
    wrap_header: Option<HeaderWrap<'a, M>>,
    trailing: Option<Element<'a, M>>,
}

/// A wrapper put around the header row.
type HeaderWrap<'a, M> = Box<dyn FnOnce(Element<'a, M>) -> Element<'a, M> + 'a>;

impl<'a, M: 'a> EnableSection<'a, M> {
    /// A subsection headed by `label`, switched on when `is_enabled`.
    /// Without [`Self::on_toggle`] the header is read-only.
    pub fn new(label: impl Into<Label<'a>>, is_enabled: bool) -> Self {
        Self {
            label: label.into(),
            is_enabled,
            on_toggle: None,
            tip: None,
            is_live_while_off: false,
            wrap_header: None,
            trailing: None,
        }
    }

    /// `trailing` on the header line right of the label, such as a size
    /// or a reason. It takes only the room the label leaves, so a long
    /// one wraps there and never runs into the label.
    pub fn trailing(mut self, trailing: impl Into<Element<'a, M>>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    /// The message the header sends when it is ticked or cleared.
    pub fn on_toggle(mut self, on_toggle: impl Fn(bool) -> M + 'a) -> Self {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    /// A hover note on the header: what turning it off does.
    pub fn tip(mut self, tip: impl Into<Label<'a>>) -> Self {
        self.tip = Some(tip.into());
        self
    }

    /// Wraps the header row only, not the dependent fields: a row hover
    /// region for the header, so it does not nest the dependents' own.
    pub fn wrap_header(mut self, wrap: impl FnOnce(Element<'a, M>) -> Element<'a, M> + 'a) -> Self {
        self.wrap_header = Some(Box::new(wrap));
        self
    }

    /// Keeps the fields live and unveiled while the header is off, for a
    /// field whose typing is what turns the header on.
    pub fn live_while_off(mut self, is_live: bool) -> Self {
        self.is_live_while_off = is_live;
        self
    }

    /// The header above `body`, the subsection's dependent fields.
    pub fn view(
        self,
        style: EnableSectionStyle<'a>,
        body: impl Into<Element<'a, M>>,
    ) -> Element<'a, M> {
        let EnableSectionStyle {
            header_ink,
            disabled_ink,
            panel,
            font,
            checkbox: checkbox_look,
            tip: tip_look,
        } = style;
        let header = header(
            self.label,
            self.is_enabled,
            self.on_toggle,
            self.trailing,
            font,
            move |theme: &Theme, status| {
                header_style(
                    checkbox_look(theme, status),
                    status,
                    header_ink,
                    disabled_ink,
                )
            },
        );
        let header = match self.tip {
            Some(tip) => tooltip(
                header,
                container(text(tip).size(TS.body))
                    .padding([SP.xs, SP.s])
                    .style(move |theme: &Theme| tip_look(theme)),
                tooltip::Position::Bottom,
            )
            .boxed(),
            None => header,
        };
        let header = match self.wrap_header {
            Some(wrap) => wrap(header),
            None => header,
        };
        Column::new()
            .width(Length::Fill)
            .push(header)
            .push(gate_dependents(
                dependents_veil(panel, self.is_enabled || self.is_live_while_off),
                body.into(),
            ))
            .boxed()
    }
}

/// The veil over the dependent fields: none while the subsection is on,
/// `panel` at [`DEPENDENT_VEIL_ALPHA`] while it is off.
pub fn dependents_veil(panel: Color, is_enabled: bool) -> Option<Color> {
    (!is_enabled).then_some(Color {
        a: DEPENDENT_VEIL_ALPHA,
        ..panel
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

/// The header row: the enable checkbox, labelled with the subsection name,
/// then `trailing` in the room the label leaves.
fn header<'a, M: 'a>(
    label: Label<'a>,
    is_enabled: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M + 'a>>,
    trailing: Option<Element<'a, M>>,
    font: Font,
    look: impl Fn(&Theme, checkbox::Status) -> checkbox::Style + 'a,
) -> Element<'a, M> {
    let mut check = checkbox::<M, Theme, iced::Renderer>(is_enabled)
        .labelled(label)
        .size(TS.body)
        .text_size(TS.caption)
        .spacing(SP.s + SP.xs)
        .font(Font {
            weight: iced::font::Weight::Medium,
            ..font
        })
        .style(look);
    if let Some(on_toggle) = on_toggle {
        check = check.on_toggle(on_toggle);
    }
    let line: Element<'a, M> = match trailing {
        Some(trailing) => Row::new()
            .push(check)
            .push(trailing)
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .into(),
        None => check.into(),
    };
    container(line)
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: SP.xs,
            left: SP.l,
        })
        .width(Length::Fill)
        .boxed()
}

/// `base` (the form checkbox look) with the header ink; a read-only header
/// that is ticked fills with `disabled_ink`, so it does not read as
/// clickable.
fn header_style(
    base: checkbox::Style,
    status: checkbox::Status,
    ink: Color,
    disabled_ink: Color,
) -> checkbox::Style {
    let mut style = base;
    style.text_color = Some(ink);
    if let checkbox::Status::Disabled { is_checked: true } = status {
        style.background = iced::Background::Color(disabled_ink);
        style.border.color = disabled_ink;
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{Background, Border};

    fn form_look() -> checkbox::Style {
        checkbox::Style {
            background: Background::Color(Color::BLACK),
            icon_color: Color::WHITE,
            border: Border {
                color: Color::BLACK,
                ..Border::default()
            },
            text_color: None,
        }
    }

    #[test]
    fn an_enabled_subsection_has_no_veil() {
        assert_eq!(dependents_veil(Color::WHITE, true), None);
    }

    #[test]
    fn a_disabled_subsection_veils_its_fields_in_the_panel_colour() {
        let panel = Color::from_rgb(0.2, 0.3, 0.4);
        let veil = dependents_veil(panel, false).expect("off subsection is veiled");
        assert_eq!((veil.r, veil.g, veil.b), (panel.r, panel.g, panel.b));
        assert_eq!(veil.a, DEPENDENT_VEIL_ALPHA);
    }

    // The veil greys without hiding.
    const _: () = assert!(DEPENDENT_VEIL_ALPHA > 0.0 && DEPENDENT_VEIL_ALPHA < 1.0);

    #[test]
    fn a_read_only_ticked_header_fills_with_the_disabled_ink() {
        let ink = Color::WHITE;
        let ink4 = Color::from_rgb(0.5, 0.5, 0.5);
        let status = checkbox::Status::Disabled { is_checked: true };
        let style = header_style(form_look(), status, ink, ink4);
        assert_eq!(style.background, Background::Color(ink4));
        assert_eq!(style.border.color, ink4);
        assert_eq!(style.text_color, Some(ink));
    }

    #[test]
    fn a_live_ticked_header_keeps_the_form_look() {
        let status = checkbox::Status::Active { is_checked: true };
        let form = form_look();
        let style = header_style(form, status, Color::WHITE, Color::BLACK);
        assert_eq!(style.background, form.background);
        assert_eq!(style.border.color, form.border.color);
        assert_eq!(style.text_color, Some(Color::WHITE));
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

    use crate::button;

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
        let veil = dependents_veil(Color::from_rgb(0.1, 0.1, 0.1), false);
        assert!(click_dependent(veil).is_empty());
    }
}
