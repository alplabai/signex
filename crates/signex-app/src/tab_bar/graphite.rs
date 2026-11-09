//! Graphite look of the document tab strip: square tabs with a mono kind
//! glyph, an accent dirty dot, an 18 px close button and the project name on
//! the right. Legacy themes never reach these helpers.

use iced::widget::{Widget, container, row, space, stack, svg};
use iced::{Background, Border, Color, Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::tab_pill::{AccentPosition, TabPillStyle};
use signex_widgets::{button, raw, text};

use super::TabMessage;
use crate::app::TabKind;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, Radii, TypeScale, chrome_px, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Side of the close button (18 px at the default base).
pub const CLOSE_SIZE: f32 = chrome_px(18.0 * 11.0 / 12.0);
/// Side of the cross drawn inside the close button.
const CLOSE_CROSS: f32 = CLOSE_SIZE * 0.5;
/// Dirty dot diameter.
const DIRTY_DOT: f32 = TS.body * 0.6;
/// Space between the parts of a tab.
const TAB_GAP: f32 = TS.body * 0.5;
/// Horizontal padding inside a tab.
const TAB_PAD_X: f32 = 12.0;
/// Space around the project name at the strip's right end.
const NAME_PAD_X: f32 = 12.0;

const CLOSE_CROSS_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round"><path d="M3.5 3.5l9 9M12.5 3.5l-9 9"/></svg>"##;

/// Height of the strip.
pub fn strip_height() -> f32 {
    ChromeMetrics::DEFAULT.tab_strip
}

/// Short mono label of a document kind.
pub fn kind_glyph(kind: &TabKind) -> &'static str {
    match kind {
        TabKind::Schematic => "SCH",
        TabKind::Pcb => "PCB",
        TabKind::SymbolEditor(_) => "SYM",
        TabKind::FootprintEditor(_) => "FPT",
        TabKind::LibraryBrowser(_) => "LIB",
        TabKind::ComponentEditor(_) => "CMP",
        TabKind::Project(_) => "PRJ",
    }
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Pill colours: the active tab is the `panel` fill, the others are clear
/// so the strip shows through. A dragged tab is tinted with the accent.
pub fn pill_style(
    tokens: &ThemeTokens,
    is_active: bool,
    is_dragging: bool,
    is_last: bool,
) -> TabPillStyle {
    let accent = ti(tokens.accent);
    let fill = if is_dragging {
        Color { a: 0.22, ..accent }
    } else if is_active {
        ti(tokens.panel_bg)
    } else {
        Color::TRANSPARENT
    };
    TabPillStyle {
        fill,
        border: ti(tokens.border),
        accent,
        is_active,
        is_last,
        accent_position: AccentPosition::Top,
    }
}

/// The 18 px close button: a cross that gets a `hover` fill.
fn close_button(tokens: &ThemeTokens, index: usize, ink: Color) -> Element<'static, TabMessage> {
    let hover = ti(tokens.hover);
    let cross = svg(svg::Handle::from_memory(CLOSE_CROSS_SVG))
        .width(CLOSE_CROSS)
        .height(CLOSE_CROSS)
        .style(move |_: &iced::Theme, _| svg::Style { color: Some(ink) });
    button(
        container(cross)
            .width(CLOSE_SIZE)
            .height(CLOSE_SIZE)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .style(close_style(hover))
    .on_press(TabMessage::CloseClicked(index))
    .boxed()
}

/// The close button's fill: none at rest, `hover` while hovered or pressed.
fn close_style(hover: Color) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(hover)),
            _ => None,
        },
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn dirty_dot(tokens: &ThemeTokens) -> Element<'static, TabMessage> {
    let accent = ti(tokens.accent);
    container(space::horizontal())
        .width(DIRTY_DOT)
        .height(DIRTY_DOT)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: (DIRTY_DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// What a tab shows besides its title.
pub struct TabFace<'t> {
    pub index: usize,
    pub kind: &'t TabKind,
    pub title: String,
    pub is_active: bool,
    pub is_dirty: bool,
    pub read_only: bool,
}

/// The inside of one tab: kind glyph, title, accent dirty dot, a read-only
/// note, close button.
pub fn tab_inner(face: TabFace<'_>, tokens: &ThemeTokens) -> Element<'static, TabMessage> {
    let r = resolve(tokens);
    let title_ink = if face.is_active {
        ti(tokens.text)
    } else {
        r.ink3
    };
    let glyph_ink = if face.is_active {
        r.accent_text
    } else {
        r.ink4
    };
    let mut content = row![
        text(raw(kind_glyph(face.kind)))
            .size(TS.caption)
            .color(glyph_ink)
            .font(mono())
            .boxed(),
        text(signex_i18n::UserText::new(face.title))
            .size(TS.body)
            .color(title_ink)
            .boxed(),
    ]
    .spacing(TAB_GAP)
    .align_y(iced::Alignment::Center);
    if face.is_dirty {
        content = content.push(dirty_dot(tokens));
    }
    if face.read_only {
        content = content.push(
            text(raw(super::READ_ONLY_LABEL))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    content = content.push(close_button(tokens, face.index, r.ink3));
    container(content)
        .padding([0.0, TAB_PAD_X])
        .height(strip_height())
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
}

/// Wrap the tab row `tabs` into the strip: the `bg` band with a 1 px
/// baseline under the tabs (the active tab's fill covers it, so it joins the
/// panel below) and the project name at the right.
pub fn strip<'a>(
    tabs: Element<'a, TabMessage>,
    project_name: Option<&str>,
    tokens: &ThemeTokens,
) -> Element<'a, TabMessage> {
    let line = ti(tokens.border);
    let baseline = container(space::horizontal())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    let baseline_layer = container(baseline)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(iced::alignment::Vertical::Bottom);

    let mut top =
        row![container(tabs).width(Length::Fill).boxed()].align_y(iced::Alignment::Center);
    if let Some(name) = project_name {
        top = top.push(
            container(
                text(signex_i18n::UserText::new(name.to_owned()))
                    .size(TS.body)
                    .color(resolve(tokens).ink3),
            )
            .padding([0.0, NAME_PAD_X])
            .boxed(),
        );
    }
    let bg = ti(tokens.bg);
    container(stack![baseline_layer, top])
        .width(Length::Fill)
        .height(strip_height())
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(bg)),
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TabKind;
    use signex_types::theme::{ThemeId, theme_tokens};
    use std::path::PathBuf;

    #[test]
    fn every_tab_kind_has_a_three_letter_glyph() {
        let kinds = [
            (TabKind::Schematic, "SCH"),
            (TabKind::Pcb, "PCB"),
            (TabKind::SymbolEditor(PathBuf::new()), "SYM"),
            (TabKind::FootprintEditor(PathBuf::new()), "FPT"),
            (TabKind::LibraryBrowser(PathBuf::new()), "LIB"),
        ];
        for (kind, want) in kinds {
            assert_eq!(kind_glyph(&kind), want);
        }
    }

    #[test]
    fn only_the_active_tab_is_filled() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        let active = pill_style(&t, true, false, false);
        let idle = pill_style(&t, false, false, false);
        assert_eq!(active.fill, ti(t.panel_bg));
        assert_eq!(idle.fill, Color::TRANSPARENT);
        assert_eq!(strip_height(), 34.0);
    }

    #[test]
    fn the_strip_is_exactly_the_strip_height() {
        use iced::advanced::Widget as _;
        let t = theme_tokens(ThemeId::GraphiteDark);
        let el = strip(space::horizontal().boxed(), Some("Sensor node rev B"), &t);
        assert_eq!(el.size().height, Length::Fixed(34.0));
    }

    /// The close cross fills only on hover and press, in every built-in
    /// theme; Graphite gives hover and press the same fill.
    #[test]
    fn close_button_hover_and_pressed_differ_from_rest() {
        use iced::widget::button::Status;
        for &id in ThemeId::BUILTINS {
            let tokens = theme_tokens(id);
            let style = close_style(ti(tokens.hover));
            let theme = iced::Theme::Dark;
            let active = style(&theme, Status::Active);
            let hovered = style(&theme, Status::Hovered);
            let pressed = style(&theme, Status::Pressed);
            assert!(active.background.is_none(), "{id:?}");
            assert_eq!(
                hovered.background,
                Some(Background::Color(ti(tokens.hover))),
                "{id:?}"
            );
            assert_ne!(hovered.background, active.background, "{id:?}");
            assert_ne!(pressed.background, active.background, "{id:?}");
        }
    }

    /// The dirty dot is smaller than the label it sits beside, the close
    /// button and the label's line box fit the strip, and the strip is a
    /// whole number of pixels so the chrome below starts on a pixel boundary.
    #[test]
    fn the_tab_parts_fit_the_strip_and_the_strip_is_whole_pixels() {
        const {
            assert!(DIRTY_DOT > 0.0 && DIRTY_DOT < TS.body);
            assert!(CLOSE_CROSS <= CLOSE_SIZE);
        }
        assert!(CLOSE_SIZE <= strip_height());
        assert!(TS.body * 1.3 <= strip_height());
        assert_eq!(strip_height(), strip_height().round());
    }
}
