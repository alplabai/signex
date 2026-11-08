//! Graphite look of the Properties form controls.
//!
//! The form-row builders only receive resolved colours, so the controls
//! find the look in their style closures: `&Theme` names the Graphite
//! themes ([`FormLook::of`]). Every style falls back to the stock iced
//! style on any other theme, so legacy output is unchanged.

use super::graphite::PROPERTY_FIELD_RADIUS;
use super::*;
use crate::styles::tokens::resolve;
use crate::styles::{TypeScale, ti};
use iced::Widget;
use iced::widget::{button, checkbox, scrollable, text_input};
use signex_types::theme::{ThemeId, theme_tokens};

const TS: TypeScale = TypeScale::DEFAULT;

/// Corner radius of a segmented-control cell.
const SEGMENT_RADIUS: f32 = 6.0;
/// Corner radius of the frame around a segmented control.
const SEGMENT_FRAME_RADIUS: f32 = 8.0;
/// Alpha of the ink wash under a hovered, inactive segment: a neutral grey,
/// not the tinted accent pill.
const SEGMENT_HOVER_ALPHA: f32 = 0.08;
/// Corner radius of a checkbox box.
const CHECKBOX_RADIUS: f32 = 4.0;
/// Width of the themed scrollbar.
const SCROLLER_RADIUS: f32 = 4.0;

/// The colours a Graphite form control draws with.
#[derive(Debug, Clone, Copy)]
pub(in crate::panels) struct FormLook {
    pub field: Color,
    pub raised: Color,
    pub sunken: Color,
    pub line: Color,
    pub line_strong: Color,
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub primary: Color,
    pub primary_ink: Color,
}

impl FormLook {
    /// The look of `theme`, or `None` when it is not a Graphite theme.
    pub fn of(theme: &Theme) -> Option<Self> {
        let name = theme.to_string();
        let id = if name.starts_with("Graphite Light") {
            ThemeId::GraphiteLight
        } else if name.starts_with("Graphite Dark") {
            ThemeId::GraphiteDark
        } else {
            return None;
        };
        let tokens = theme_tokens(id);
        let r = resolve(&tokens);
        Some(Self {
            field: r.field,
            raised: r.raised,
            sunken: r.sunken,
            line: ti(tokens.border),
            line_strong: r.line_strong,
            ink: ti(tokens.text),
            ink3: r.ink3,
            ink4: r.ink4,
            accent: ti(tokens.accent),
            accent_soft: r.accent_soft,
            primary: r.primary,
            primary_ink: r.primary_ink,
        })
    }
}

/// Fill, border colour and radius of a read-only value box: Graphite's
/// field token and hairline, else the caller's legacy colours.
pub(in crate::panels) fn value_box(
    theme: &Theme,
    legacy_bg: Color,
    legacy_border: Color,
    legacy_radius: f32,
) -> (Color, Color, f32) {
    match FormLook::of(theme) {
        Some(l) => (l.field, l.line, PROPERTY_FIELD_RADIUS),
        None => (legacy_bg, legacy_border, legacy_radius),
    }
}

/// Graphite text field; `legacy` on any other theme.
pub(in crate::panels) fn text_input_or(
    theme: &Theme,
    status: text_input::Status,
    legacy: text_input::Style,
) -> text_input::Style {
    let Some(l) = FormLook::of(theme) else {
        return legacy;
    };
    graphite_text_input(l, status)
}

fn graphite_text_input(l: FormLook, status: text_input::Status) -> text_input::Style {
    let border = match status {
        text_input::Status::Focused { .. } => l.accent,
        text_input::Status::Hovered => l.line_strong,
        _ => l.line,
    };
    text_input::Style {
        background: Background::Color(l.field),
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: border,
            ..Border::default()
        },
        placeholder: l.ink4,
        value: l.ink,
        selection: Color { a: 0.4, ..l.accent },
    }
}

/// `text_input` style for a form field: Graphite, else the stock style.
pub(crate) fn text_input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    match FormLook::of(theme) {
        Some(l) => graphite_text_input(l, status),
        None => text_input::default(theme, status),
    }
}

/// `checkbox` style for a form toggle: Graphite, else the stock style.
pub(crate) fn checkbox_style(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let base = checkbox::primary(theme, status);
    let Some(l) = FormLook::of(theme) else {
        return base;
    };
    let checked = match status {
        checkbox::Status::Active { is_checked }
        | checkbox::Status::Hovered { is_checked }
        | checkbox::Status::Disabled { is_checked } => is_checked,
    };
    checkbox::Style {
        background: Background::Color(if checked { l.primary } else { l.field }),
        icon_color: l.primary_ink,
        border: Border {
            width: 1.0,
            radius: CHECKBOX_RADIUS.into(),
            color: if checked { l.primary } else { l.line_strong },
            ..Border::default()
        },
        text_color: base.text_color,
    }
}

/// `scrollable` style of a dock panel: a thin neutral scroller on Graphite,
/// else the stock style.
pub(in crate::panels) fn scrollbar_style(
    theme: &Theme,
    status: scrollable::Status,
) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    let Some(l) = FormLook::of(theme) else {
        return style;
    };
    let idle = matches!(status, scrollable::Status::Active { .. });
    let scroller = || scrollable::Scroller {
        background: Background::Color(if idle { l.line_strong } else { l.ink4 }),
        border: Border {
            radius: SCROLLER_RADIUS.into(),
            ..Border::default()
        },
    };
    for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
        rail.background = None;
        rail.border = Border::default();
        rail.scroller = scroller();
    }
    style
}

/// Frame of a segmented control: sunken fill, radius 8, 2 px inset.
/// Transparent (the stock look) on other themes.
pub(in crate::panels) fn segmented_frame(theme: &Theme) -> container::Style {
    match FormLook::of(theme) {
        Some(l) => container::Style {
            background: Some(Background::Color(l.sunken)),
            border: Border {
                radius: SEGMENT_FRAME_RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        },
        None => container::Style::default(),
    }
}

/// One cell of a Graphite segmented control: raised and bold-ink when on,
/// flat when off. `None` on other themes.
pub(in crate::panels) fn segment_cell(
    theme: &Theme,
    active: bool,
    hovered: bool,
) -> Option<button::Style> {
    let l = FormLook::of(theme)?;
    let fill = if active {
        Some(Background::Color(l.raised))
    } else if hovered {
        Some(Background::Color(Color {
            a: SEGMENT_HOVER_ALPHA,
            ..l.ink
        }))
    } else {
        None
    };
    Some(button::Style {
        background: fill,
        text_color: if active { l.ink } else { l.ink3 },
        border: Border {
            radius: SEGMENT_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
}

/// Ghost button: transparent, hairline, radius 7. `None` on other themes.
pub(crate) fn ghost_button(theme: &Theme, status: button::Status) -> Option<button::Style> {
    let l = FormLook::of(theme)?;
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    Some(button::Style {
        background: hovered.then_some(Background::Color(l.accent_soft)),
        text_color: l.ink,
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: l.line,
            ..Border::default()
        },
        ..button::Style::default()
    })
}

/// Option tile of a Properties row (fill, placement, a class list): a ghost
/// button, the chosen one in accent-soft with an accent hairline. The stock
/// secondary style on any other theme.
pub(crate) fn option_tile(
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |theme, status| {
        let Some(mut style) = ghost_button(theme, status) else {
            return button::secondary(theme, status);
        };
        if let (true, Some(l)) = (active, FormLook::of(theme)) {
            style.background = Some(Background::Color(l.accent_soft));
            style.border.color = l.accent;
        }
        style
    }
}

/// A value shown as a field that opens something when pressed (the canvas
/// font): field fill, hairline, ink text; the hairline strengthens on hover.
pub(crate) fn value_field_button(theme: &Theme, status: button::Status) -> button::Style {
    let Some(l) = FormLook::of(theme) else {
        return button::text(theme, status);
    };
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(Background::Color(l.field)),
        text_color: l.ink,
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: if hovered { l.line_strong } else { l.line },
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Active tab chip of the Properties tab strip: accent-soft fill, radius 7.
/// `None` on other themes.
pub(in crate::panels) fn tab_chip(
    theme: &Theme,
    active: bool,
    hovered: bool,
) -> Option<button::Style> {
    let l = FormLook::of(theme)?;
    Some(button::Style {
        background: (active || hovered).then_some(Background::Color(l.accent_soft)),
        text_color: if active { l.ink } else { l.ink3 },
        border: Border {
            radius: PROPERTY_FIELD_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
}

/// Section label of a Graphite form: uppercase, muted, 12 px side padding.
pub(in crate::panels) fn section_label<'a, M: 'a>(
    title: &str,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let ink = resolve(tokens).ink4;
    container(text(title.to_uppercase()).size(TS.section).color(ink))
        .padding(iced::Padding {
            top: 12.0,
            right: 12.0,
            bottom: 6.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .boxed()
}

/// Collapsible-section header of a Graphite form: chevron plus the
/// uppercase label, no rule.
fn graphite_section_header<'a>(
    key: &str,
    title: &str,
    is_collapsed: bool,
    tokens: &ThemeTokens,
) -> Column<Element<'a, PanelMsg>> {
    let ink = resolve(tokens).ink4;
    let label = row![
        signex_widgets::chevron::chevron(!is_collapsed, TS.section, ink),
        text(title.to_uppercase()).size(TS.section).color(ink),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let header = button(
        container(label)
            .padding(iced::Padding {
                top: 12.0,
                right: 12.0,
                bottom: 6.0,
                left: 12.0,
            })
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(key.to_string()))
    .style(|_: &Theme, _| button::Style::default());
    Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(header.boxed())
}

impl PanelContext {
    /// A collapsible form section on the Graphite uppercase-label chrome.
    pub(in crate::panels) fn collapsible<'a>(
        &self,
        key: &str,
        title: &str,
        collapsed: &CollapsedSections,
        content: impl FnOnce() -> Column<Element<'a, PanelMsg>>,
    ) -> Column<Element<'a, PanelMsg>> {
        let is_collapsed = collapsed.contains(key);
        let mut col = graphite_section_header(key, title, is_collapsed, &self.tokens);
        if !is_collapsed {
            col = col.push(content().boxed());
        }
        col
    }
}
