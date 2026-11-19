//! The header of a Properties panel: the kind of the object as the title, a
//! muted line the caller supplies (`net /SPI_CLK`, `covers 6 objects`, a
//! part's description), an optional ⋯ menu slot and an optional warning or
//! error banner with its action buttons under it (epic #2523).
//!
//! The single selection builds it through [`selection_header`]: a placed
//! part reads "Component" with its description; every other object shows
//! the engine's type name.

use super::super::*;
use crate::panels::form_look::FormLook;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};
use iced::{Alignment, Padding};
use signex_i18n::{UserText, ids, t};
use signex_widgets::Label;
use signex_widgets::theme_ext::Radii;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Header padding at a 12 px body: 8 px above, 6 px below.
const HEADER_PAD_TOP_EM: f32 = 8.0 / 12.0;
const HEADER_PAD_BOTTOM_EM: f32 = 6.0 / 12.0;
/// Smallest header height (36 px at 12 px), so a header with a ⋯ button and
/// one without line up.
const HEADER_MIN_HEIGHT_EM: f32 = 36.0 / 12.0;
/// Side of the ⋯ icon button (24 px at 12 px).
const ICON_BUTTON_EM: f32 = 24.0 / 12.0;
/// Banner padding (8 px by 10 px at 12 px).
const BANNER_PAD_Y_EM: f32 = 8.0 / 12.0;
const BANNER_PAD_X_EM: f32 = 10.0 / 12.0;
/// Height of a banner action button (22 px at 12 px).
const ACTION_HEIGHT_EM: f32 = 22.0 / 12.0;

/// The title a placed part's header shows.
pub(super) const COMPONENT_TITLE: &str = "Component";

/// The colour of a header banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "header kit of epic #2523; the g1 units adopt it")]
pub(in crate::panels) enum BannerTone {
    /// Something to act on (a part older than its library).
    Warn,
    /// Something broken (a symbol missing from its library).
    Error,
}

/// A banner action: its label, its message and whether it is the quiet
/// (ghost) one of the pair.
#[allow(dead_code, reason = "header kit of epic #2523; the g1 units adopt it")]
pub(in crate::panels) struct BannerAction<'a> {
    pub label: Label<'a>,
    pub on_press: PanelMsg,
    pub is_quiet: bool,
}

/// A banner under the header: one bold line and its actions.
#[allow(dead_code, reason = "header kit of epic #2523; the g1 units adopt it")]
pub(in crate::panels) struct HeaderBanner<'a> {
    pub tone: BannerTone,
    pub title: Label<'a>,
    pub actions: Vec<BannerAction<'a>>,
}

/// What a Properties header shows.
pub(in crate::panels) struct ObjectHeader<'a> {
    /// The kind of the object (Wire, Net Label, Component).
    pub title: Label<'a>,
    /// The muted line beside the title, worded by the caller.
    pub line: Option<Label<'a>>,
    /// The ⋯ button and its menu, built by the caller.
    pub menu: Option<Element<'a, PanelMsg>>,
    /// A warning or error banner under the header.
    pub banner: Option<HeaderBanner<'a>>,
}

impl<'a> ObjectHeader<'a> {
    /// A header with `title` and nothing else.
    pub fn new(title: impl Into<Label<'a>>) -> Self {
        Self {
            title: title.into(),
            line: None,
            menu: None,
            banner: None,
        }
    }

    /// Sets the muted line; an empty line shows nothing.
    pub fn line(mut self, line: impl Into<Label<'a>>) -> Self {
        let line = line.into();
        self.line = (!line.as_str().trim().is_empty()).then_some(line);
        self
    }

    #[allow(dead_code, reason = "header kit of epic #2523; the g1 units adopt it")]
    pub fn menu(mut self, menu: Element<'a, PanelMsg>) -> Self {
        self.menu = Some(menu);
        self
    }

    #[allow(dead_code, reason = "header kit of epic #2523; the g1 units adopt it")]
    pub fn banner(mut self, banner: HeaderBanner<'a>) -> Self {
        self.banner = Some(banner);
        self
    }
}

/// The ⋯ button of a header; `on_press` opens the caller's menu.
#[allow(dead_code, reason = "header kit of epic #2523; the g1 units adopt it")]
pub(in crate::panels) fn more_button<'a>(
    i18n: &signex_i18n::I18n,
    on_press: PanelMsg,
) -> Element<'a, PanelMsg> {
    let side = TS.body * ICON_BUTTON_EM;
    let glyph = text(t!(i18n, ids::PROPERTIES_HEADER_MORE_GLYPH))
        .size(TS.label)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .height(Length::Fill);
    let face = button(glyph)
        .width(side)
        .height(side)
        .padding(0)
        .on_press(on_press)
        .style(|theme: &Theme, status| {
            let l = FormLook::of(theme);
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: hovered.then_some(Background::Color(l.sunken)),
                text_color: l.ink3,
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        });
    signex_widgets::tooltip(
        face,
        container(text(t!(i18n, ids::PROPERTIES_HEADER_MORE)).size(TS.caption))
            .padding([SP.xs, SP.s]),
        iced::widget::tooltip::Position::Bottom,
    )
    .style(crate::panels::form_look::value_tip)
    .boxed()
}

/// The title row: the bold kind, the muted line filling the rest, the ⋯.
fn title_row<'a>(header: &mut ObjectHeader<'a>) -> Element<'a, PanelMsg> {
    let title = std::mem::replace(&mut header.title, Label::from(UserText::new(String::new())));
    let mut row = iced::widget::Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .push(
            text(title)
                .size(TS.label)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::DEFAULT
                })
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(FormLook::of(theme).ink),
                    ..Default::default()
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    row = match header.line.take() {
        Some(line) => row.push(
            text(line)
                .size(TS.caption)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(FormLook::of(theme).ink4),
                    ..Default::default()
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
        ),
        None => row.push(Space::new().width(Length::Fill).boxed()),
    };
    if let Some(menu) = header.menu.take() {
        row = row.push(menu);
    }
    container(row)
        .padding(Padding {
            top: TS.body * HEADER_PAD_TOP_EM,
            right: SP.s,
            bottom: TS.body * HEADER_PAD_BOTTOM_EM,
            left: SP.m,
        })
        .height(Length::Shrink)
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
}

/// One banner action: a small button, hairline or quiet (accent text).
fn banner_action<'a>(action: BannerAction<'a>, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let accent_text = resolve(tokens).accent_text;
    let is_quiet = action.is_quiet;
    button(text(action.label).size(TS.caption))
        .height(TS.body * ACTION_HEIGHT_EM)
        .padding([0.0, SP.m])
        .on_press(action.on_press)
        .style(move |theme: &Theme, status| {
            let mut style = crate::panels::form_look::value_field_button(theme, status);
            if is_quiet {
                let hovered = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                style.background = hovered.then_some(Background::Color(FormLook::of(theme).sunken));
                style.border.color = Color::TRANSPARENT;
                style.text_color = accent_text;
            }
            style.border.radius = Radii::GRAPHITE.small.into();
            style
        })
        .boxed()
}

/// The fill and ink of a banner of `tone`.
fn banner_colours(tone: BannerTone, tokens: &ThemeTokens) -> (Color, Color) {
    let r = resolve(tokens);
    match tone {
        BannerTone::Warn => (r.warn_soft, theme_ext::warning_color(tokens)),
        BannerTone::Error => (r.err_soft, theme_ext::error_color(tokens)),
    }
}

/// The banner under the header: tone fill, bold line, actions below.
fn banner_view<'a>(banner: HeaderBanner<'a>, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let (fill, ink) = banner_colours(banner.tone, tokens);
    let mut body = Column::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s + SP.xs)
        .width(Length::Fill)
        .push(
            text(banner.title)
                .size(TS.body)
                .color(ink)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::DEFAULT
                })
                .boxed(),
        );
    if !banner.actions.is_empty() {
        let mut actions = iced::widget::Row::<Element<'a, PanelMsg>>::new().spacing(SP.s + SP.xs);
        for action in banner.actions {
            actions = actions.push(banner_action(action, tokens));
        }
        body = body.push(actions.boxed());
    }
    container(
        container(body)
            .padding([TS.body * BANNER_PAD_Y_EM, TS.body * BANNER_PAD_X_EM])
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
    .padding([SP.s + SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A Properties header: the title row, its banner, never taller than the
/// content needs but at least the mockup's 36 px row.
pub(in crate::panels) fn object_header<'a>(
    ctx: &PanelContext,
    mut header: ObjectHeader<'a>,
) -> Element<'a, PanelMsg> {
    let title = container(title_row(&mut header))
        .height(Length::Shrink)
        .width(Length::Fill);
    let mut col = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(
            iced::widget::row![Space::new().height(TS.body * HEADER_MIN_HEIGHT_EM), title,]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .boxed(),
        );
    if let Some(banner) = header.banner.take() {
        col = col.push(banner_view(banner, &ctx.tokens));
    }
    col.boxed()
}

/// Title and the muted line beside it. A placed part is a "Component" with
/// its description; anything else keeps `elem_type` alone.
pub(super) fn header_text<'s>(
    elem_type: &'s str,
    is_placed_part: bool,
    description: &'s str,
) -> (&'s str, Option<&'s str>) {
    if is_placed_part {
        let line = description.trim();
        (COMPONENT_TITLE, (!line.is_empty()).then_some(line))
    } else {
        (elem_type, None)
    }
}

/// The header of the single selection. No "1 of N" counter: this view shows
/// one object, and a count is shown only where the panel steps through a
/// real list. `_primary` stays for the callers; the look comes from the form.
pub(super) fn selection_header<'a>(
    ctx: &PanelContext,
    elem_type: &str,
    is_placed_part: bool,
    description: &str,
    _primary: Color,
) -> Element<'a, PanelMsg> {
    let (title, line) = header_text(elem_type, is_placed_part, description);
    // The engine's type name has no catalog message yet (#2060).
    let title: Label<'a> = if is_placed_part {
        t!(ctx.i18n, ids::PANELS_PROPERTIES_COMPONENT).into()
    } else {
        raw(title.to_owned())
    };
    let mut header = ObjectHeader::new(title);
    if let Some(line) = line {
        header = header.line(UserText::new(line.to_owned()));
    }
    object_header(ctx, header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placed_part_is_a_component_with_its_description() {
        assert_eq!(
            header_text("Symbol", true, "Resistor (IEC rectangle). "),
            ("Component", Some("Resistor (IEC rectangle)."))
        );
    }

    #[test]
    fn a_part_without_a_description_shows_only_the_title() {
        assert_eq!(header_text("Symbol", true, "  "), ("Component", None));
    }

    #[test]
    fn other_objects_keep_the_type_name() {
        assert_eq!(header_text("Wire", false, ""), ("Wire", None));
    }

    fn panel_context() -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        app.document_state.panel_ctx.clone()
    }

    fn texts(element: Element<'_, PanelMsg>) -> Vec<String> {
        crate::layout_probe::lay_out_texts(element, iced::Size::new(320.0, 400.0))
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// The muted line the caller words sits beside the title (#43 wire:
    /// `net /SPI_CLK`); an empty line draws nothing.
    #[test]
    fn the_header_shows_the_callers_muted_line_beside_the_title() {
        let ctx = panel_context();
        let header = ObjectHeader::new(UserText::new("Wire".to_owned()))
            .line(UserText::new("net /SPI_CLK".to_owned()));
        assert_eq!(texts(object_header(&ctx, header)), ["Wire", "net /SPI_CLK"]);
        let bare = ObjectHeader::new(UserText::new("Wire".to_owned()))
            .line(UserText::new("  ".to_owned()));
        assert_eq!(texts(object_header(&ctx, bare)), ["Wire"]);
    }

    /// A banner shows its line and its actions, and an action press sends
    /// its message.
    #[test]
    fn a_banner_shows_its_line_and_its_actions() {
        let ctx = panel_context();
        let header = || {
            ObjectHeader::new(UserText::new("Component".to_owned())).banner(HeaderBanner {
                tone: BannerTone::Warn,
                title: UserText::new("Library: 3 instances use an older version".to_owned()).into(),
                actions: vec![BannerAction {
                    label: UserText::new("Update from library".to_owned()).into(),
                    on_press: PanelMsg::Noop,
                    is_quiet: false,
                }],
            })
        };
        let shown = texts(object_header(&ctx, header()));
        assert!(
            shown.iter().any(|t| t == "Update from library"),
            "{shown:?}"
        );
        let pressed = crate::layout_probe::messages_on_click(
            object_header(&ctx, header()),
            iced::Size::new(320.0, 400.0),
            "Update from library",
        );
        assert!(matches!(pressed.as_slice(), [PanelMsg::Noop]));
    }
}
