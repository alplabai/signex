//! Generic Active Bar — a floating row of icon buttons used by every
//! Signex editor surface (schematic, schematic library, PCB, PCB
//! library) to surface the primary place / select tools.
//!
//! Altium-parity affordance: the bar floats over the canvas at the
//! top, the active tool reads with the accent background, disabled
//! tools (e.g. v0.9.x stubs) read greyed-out, and tooltips spell
//! out the tool name. Each editor builds its own
//! `Vec<ActiveBarItem<M>>` with the editor-specific message type;
//! this widget just renders.
//!
//! Item variants:
//! - `Button(ActiveBarButton<M>)` — clickable tool button. Optional
//!   right-press message + chevron indicator for buttons that open
//!   dropdowns (the dropdown overlay itself is rendered separately
//!   by the consumer as a Stack overlay layer).
//! - `Separator` — thin vertical divider between groups.
//! - `Custom(Element<M>)` — escape hatch for special slots (e.g.
//!   the schematic editor's wire draw-mode 90°/45°/Any cycle pill).
//!
//! # Example
//!
//! ```ignore
//! use signex_widgets::active_bar::{view, ActiveBarItem, ActiveBarButton, ActiveBarIcon};
//!
//! let items = vec![
//!     ActiveBarItem::Button(ActiveBarButton {
//!         icon: ActiveBarIcon::Svg(SELECT_SVG.clone()),
//!         tooltip: "Select".into(),
//!         enabled: true,
//!         selected: matches!(tool, Tool::Select),
//!         on_press: Some(Msg::SetTool(Tool::Select)),
//!         on_right_press: Some(Msg::OpenSelectMenu),
//!         dropdown_indicator: Some(ActiveBarIcon::Svg(CHEVRON_SVG.clone())),
//!         on_chevron_press: Some(Msg::OpenSelectMenu),
//!     }),
//!     ActiveBarItem::Separator,
//!     // …
//! ];
//! view(items, &tokens)
//! ```
//!
//! Render the resulting `Element<M>` into a `Stack` overlay so it
//! floats over the canvas content.

pub mod dropdown;
mod fit;
mod metrics;
mod skin;

pub use fit::{slot_offsets_within, view_within};
pub use metrics::Metrics;

use crate::{button, raw, text, tooltip};
use iced::widget::{container, image, mouse_area, row, svg};
use iced::{Border, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

/// One slot in the Active Bar — either a clickable button, a thin
/// vertical separator, or an arbitrary user-supplied widget (escape
/// hatch for special cases like the schematic's draw-mode pill).
pub enum ActiveBarItem<M: 'static + Clone> {
    Button(ActiveBarButton<M>),
    /// 1 px-wide vertical separator between groups.
    Separator,
    /// Arbitrary user-supplied widget — drops into the row as-is.
    ///
    /// `width` is the width the element renders at, and the producer
    /// has to give the element that exact width (a `Length::Fixed`, not
    /// a Shrink that happens to land there). [`slot_offsets`] can't
    /// measure an arbitrary `Element` without a layout pass, so this
    /// number is the only thing standing between a Custom slot and
    /// every dropdown on the bar being anchored wrong.
    Custom {
        content: Element<'static, M>,
        width: f32,
    },
}

impl<M: 'static + Clone> ActiveBarItem<M> {
    /// Convenience — build a Button item.
    pub fn button(button: ActiveBarButton<M>) -> Self {
        Self::Button(button)
    }

    /// Convenience — build a Custom item of a declared width.
    pub fn custom(content: Element<'static, M>, width: f32) -> Self {
        Self::Custom { content, width }
    }

    /// Width this slot renders at under `m`, excluding the row spacing
    /// around it.
    pub fn width(&self, m: &Metrics) -> f32 {
        match self {
            Self::Button(_) => m.btn,
            Self::Separator => m.sep_w,
            Self::Custom { width, .. } => *width,
        }
    }
}

/// Left-edge x of every slot (measured from the bar's own left edge)
/// plus the bar's total width.
///
/// The bar is `Length::Shrink` and the caller centres it, so a consumer
/// that wants to anchor an overlay under button N has to know both
/// numbers: `bar_left = (window_w - total) / 2`, then
/// `offsets[n] + bar_left`. Deriving it here rather than in each
/// consumer is the point — the layout constants are private and the
/// item list is the only truth about what got drawn, so a bar that
/// gains, loses, or reorders a slot moves its dropdowns automatically.
///
/// Measured under `m`; the bar `view` draws with [`Metrics::GRAPHITE`].
pub fn slot_offsets<M: 'static + Clone>(
    items: &[ActiveBarItem<M>],
    m: &Metrics,
) -> (Vec<f32>, f32) {
    let widths: Vec<f32> = items.iter().map(|i| i.width(m)).collect();
    offsets_for_widths(&widths, m)
}

/// [`slot_offsets`] for a bar described only by its slot widths.
///
/// Lets a consumer that knows the bar's slot order but has no item
/// list (a hit-test running outside the view) use the same row maths
/// as the renderer instead of re-deriving it.
pub fn offsets_for_widths(widths: &[f32], m: &Metrics) -> (Vec<f32>, f32) {
    let mut offsets = Vec::with_capacity(widths.len());
    let mut x = m.padding;
    for (i, width) in widths.iter().enumerate() {
        if i > 0 {
            x += m.spacing;
        }
        offsets.push(x);
        x += width;
    }
    (offsets, x + m.padding)
}

/// One clickable button in the Active Bar.
///
/// `M` is the editor's message type; the bar is generic over it so
/// every editor can publish its own messages without sharing a
/// common message enum.
pub struct ActiveBarButton<M: 'static + Clone> {
    /// Glyph or SVG to render in the button.
    pub icon: ActiveBarIcon,
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// `false` → button greys out and ignores left-click. The
    /// optional right-click (dropdown trigger) still works so the
    /// user can discover what's greyed out via the menu.
    pub enabled: bool,
    /// `true` → button paints with the accent background (Altium's
    /// "armed tool" highlight).
    pub selected: bool,
    /// Message published on left-press. `None` = passive (no
    /// left-click action even when enabled).
    pub on_press: Option<M>,
    /// Optional message published on right-press. Used by editors
    /// that pair a tool action with a dropdown menu (e.g. the
    /// schematic's Wiring button: left-click draws, right-click
    /// opens the wire/bus/label picker).
    pub on_right_press: Option<M>,
    /// Optional dropdown indicator rendered in the bottom-right
    /// corner of the button. Pass an `ActiveBarIcon::Svg(...)` with
    /// the editor's themed chevron handle to advertise the
    /// secondary (right-click) action; pass `None` for buttons that
    /// have no dropdown.
    pub dropdown_indicator: Option<ActiveBarIcon>,
    /// Message published on a click on the corner indicator, in place of
    /// `on_press` (mockup row 108): a click on the chevron opens
    /// the dropdown, a click elsewhere on the button still runs its tool.
    /// Ignored without a `dropdown_indicator`.
    pub on_chevron_press: Option<M>,
}

impl<M: 'static + Clone> Default for ActiveBarButton<M> {
    fn default() -> Self {
        Self {
            icon: ActiveBarIcon::Glyph(""),
            tooltip: String::new(),
            enabled: true,
            selected: false,
            on_press: None,
            on_right_press: None,
            dropdown_indicator: None,
            on_chevron_press: None,
        }
    }
}

/// What to render inside a button — SVG (preferred), bitmap image
/// (PNG/JPEG via raster Handle), or a Unicode glyph fallback for
/// items that have no icon yet.
pub enum ActiveBarIcon {
    /// `iced::widget::svg::Handle`. Use for any SVG asset shipped in
    /// `assets/icons/*.svg`.
    Svg(svg::Handle),
    /// Raster image handle (PNG/JPEG). Reserved for icons that don't
    /// have an SVG version.
    Raster(image::Handle),
    /// Unicode glyph fallback. Use for tools whose SVG icon hasn't
    /// been authored yet.
    Glyph(&'static str),
}

/// Render the bar.
///
/// Returns an `Element<M>` ready to push into a `Stack` overlay
/// layer. The bar is `Length::Shrink` width-wise so the caller
/// controls horizontal positioning (centred via a parent
/// `container.align_x(Center)`, etc.).
pub fn view<'a, M>(items: Vec<ActiveBarItem<M>>, tokens: &'a ThemeTokens) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let m = Metrics::GRAPHITE;
    let mut row_widget = row![].spacing(m.spacing).align_y(iced::Alignment::Center);
    for item in items {
        row_widget = row_widget.push(view_item(item, tokens, &m));
    }
    bar_shell(row_widget.boxed(), iced::Padding::new(m.padding), tokens)
}

/// The bar's padded, bordered surface around its row of slots.
fn bar_shell<'a, M>(
    slots: Element<'a, M>,
    padding: iced::Padding,
    tokens: &'a ThemeTokens,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let skin = skin::Skin::of(tokens);
    container(slots)
        .padding(padding)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(skin.fill)),
            border: Border {
                width: 1.0,
                radius: skin.radius.into(),
                color: skin.border,
                ..Border::default()
            },
            shadow: skin.shadow,
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

fn view_item<'a, M>(
    item: ActiveBarItem<M>,
    tokens: &'a ThemeTokens,
    metrics: &Metrics,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    match item {
        ActiveBarItem::Button(b) => view_button(b, tokens, metrics),
        ActiveBarItem::Separator => {
            let sep_color = theme_ext::border_color(tokens);
            container(iced::widget::Space::new())
                .width(Length::Fixed(metrics.sep_w))
                .height(Length::Fixed(metrics.sep_h))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(sep_color)),
                    ..iced::widget::container::Style::default()
                })
                .boxed()
        }
        // Pinned to the declared width so `slot_offsets` can't be lied
        // to: whatever the producer says it is, that is what draws.
        ActiveBarItem::Custom { content, width } => {
            container(content).width(Length::Fixed(width)).boxed()
        }
    }
}

fn view_button<'a, M>(
    b: ActiveBarButton<M>,
    tokens: &'a ThemeTokens,
    metrics: &Metrics,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let btn_size = metrics.btn;
    let icon_size = metrics.icon;
    let chevron_size = metrics.chevron;
    let skin = skin::Skin::of(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let muted_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    // Disabled icons are tinted to a muted gray so they read as
    // inactive. Enabled icons render with their natural SVG colors —
    // overriding `svg::Style::color` would collapse multi-colored
    // icons (e.g. the orange-fill / gray-stroke filter glyph) into a
    // monochrome silhouette and erase per-icon brand colors.
    let disabled_tint = Color { a: 0.4, ..muted_c };
    // Glyph fallback (Unicode characters rendered as text) needs an
    // explicit color in every state because text widgets do not
    // inherit a "natural" fill the way SVGs do.
    let glyph_color = if !b.enabled {
        disabled_tint
    } else if b.selected {
        skin.on_selected
    } else {
        skin.ink
    };
    // Graphite icons are monochrome strokes tinted with the ink token
    // (the bar surface colour on the armed tool).
    let svg_tint: Option<Color> = if !b.enabled {
        Some(disabled_tint)
    } else {
        Some(if b.selected {
            skin.on_selected
        } else {
            skin.ink
        })
    };

    let raw_icon: Element<'a, M> = match b.icon {
        ActiveBarIcon::Svg(handle) => svg(handle)
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .style(move |_: &Theme, _| iced::widget::svg::Style {
                color: svg_tint,
                ..Default::default()
            })
            .boxed(),
        ActiveBarIcon::Raster(handle) => image::<image::Handle>(handle)
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .boxed(),
        ActiveBarIcon::Glyph(s) => text(raw(s.to_string())).size(14).color(glyph_color).boxed(),
    };

    // Optional dropdown chevron in the bottom-right corner.
    let icon_content: Element<'a, M> = if let Some(indicator) = b.dropdown_indicator {
        let indicator_el = corner_indicator(
            indicator,
            chevron_size,
            svg_tint,
            glyph_color,
            b.on_chevron_press,
        );
        iced::widget::Stack::new()
            .push(
                container(raw_icon)
                    .width(Length::Fixed(btn_size))
                    .height(Length::Fixed(btn_size))
                    .center_x(Length::Fixed(btn_size))
                    .center_y(Length::Fixed(btn_size))
                    .boxed(),
            )
            .push(
                container(indicator_el)
                    .width(Length::Fixed(btn_size))
                    .height(Length::Fixed(btn_size))
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom)
                    .boxed(),
            )
            .boxed()
    } else {
        container(raw_icon)
            .width(Length::Fixed(btn_size))
            .height(Length::Fixed(btn_size))
            .center_x(Length::Fixed(btn_size))
            .center_y(Length::Fixed(btn_size))
            .boxed()
    };

    let selected = b.selected;
    let enabled = b.enabled;

    // Selected uses the theme's muted `hover` slate (Altium-style
    // armed-tool look) — never the bright accent, since the accent is
    // reserved for active-project markers + brand highlights and would
    // compete visually with the canvas content. Hover uses a slightly
    // brighter version of the same slate so the affordance reads
    // through without changing color register. Same palette across
    // every editor (schematic, SchLib, eventual PCB / PCB-lib) so the
    // bar reads identically wherever it appears.
    // Graphite: token washes instead of white at 6 % (invisible on light
    // themes) and the 7 px control radius.
    let mut btn = button(icon_content).padding(0).style(
        move |_: &Theme, status: iced::widget::button::Status| {
            let selected_fill = skin.selected;
            let hover_fill = skin.hover;
            let bg = if !enabled {
                None
            } else if selected {
                Some(iced::Background::Color(selected_fill))
            } else {
                match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Background::Color(hover_fill))
                    }
                    _ => None,
                }
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 0.0,
                    radius: skin.button_radius.into(),
                    color: border,
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        },
    );
    if enabled && let Some(msg) = b.on_press {
        btn = btn.on_press(msg);
    }

    // Wrap in mouse_area for right-click → dropdown trigger. The
    // dropdown overlay itself is rendered separately by the
    // consumer as a Stack overlay layer.
    let interactive: Element<'a, M> = if let Some(rc) = b.on_right_press {
        mouse_area(btn).on_right_press(rc).boxed()
    } else {
        btn.boxed()
    };

    let tooltip_text = b.tooltip.clone();
    tooltip(
        interactive,
        container(text(raw(tooltip_text)).size(11).color(text_c))
            .padding([2, 6])
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme_ext::to_color(
                    &tokens.panel_bg,
                ))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                    ..Border::default()
                },
                ..iced::widget::container::Style::default()
            }),
        tooltip::Position::Bottom,
    )
    .boxed()
}

/// The dropdown indicator, `size` square. With `on_press` it is a button of
/// its own inside the bar button: a click on it publishes that message, and
/// it takes the press and the release, so the bar button does not run and
/// the canvas under the bar sees neither (mockup row 108).
fn corner_indicator<'a, M>(
    indicator: ActiveBarIcon,
    size: f32,
    svg_tint: Option<Color>,
    glyph_color: Color,
    on_press: Option<M>,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let drawn: Element<'a, M> = match indicator {
        ActiveBarIcon::Svg(handle) => svg(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .style(move |_: &Theme, _| iced::widget::svg::Style {
                color: svg_tint,
                ..Default::default()
            })
            .boxed(),
        ActiveBarIcon::Raster(handle) => image::<image::Handle>(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .boxed(),
        ActiveBarIcon::Glyph(s) => text(raw(s.to_string())).size(10).color(glyph_color).boxed(),
    };
    let Some(message) = on_press else {
        return drawn;
    };
    let area = container(drawn)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size));
    // No fill of its own: the bar button under it draws the hover.
    iced::widget::button(area)
        .padding(0)
        .style(|_: &Theme, _| iced::widget::button::Style::default())
        .on_press(message)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::layout::Limits;
    use iced::advanced::shell::{Bus, Waker};
    use iced::advanced::widget::Tree;
    use iced::advanced::{Layout, Shell, renderer};
    use iced::mouse;
    use iced::{Event, Font, Pixels, Point, Rectangle, Size};
    use signex_types::theme::{ThemeId, theme_tokens};

    const RUN: u8 = 1;
    const RIGHT: u8 = 2;
    const CHEVRON: u8 = 3;

    fn group_button(on_chevron_press: Option<u8>) -> ActiveBarItem<u8> {
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph("T"),
            on_press: Some(RUN),
            on_right_press: Some(RIGHT),
            dropdown_indicator: Some(ActiveBarIcon::Glyph("v")),
            on_chevron_press,
            ..ActiveBarButton::default()
        })
    }

    /// The messages a press and release of `button` at `at` publish on a
    /// bar holding only `item`.
    fn press(item: ActiveBarItem<u8>, button: mouse::Button, at: Point) -> Vec<u8> {
        let tokens = theme_tokens(ThemeId::DEFAULT);
        let mut element = view(vec![item], &tokens);
        let renderer =
            iced::Renderer::Secondary(iced_vello_cpu::Renderer::new(renderer::Settings {
                font: Font::DEFAULT,
                text_size: Pixels(16.0),
                ..renderer::Settings::default()
            }));
        let window = Size::new(200.0, 100.0);
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        element.layout(&mut tree, &renderer, &Limits::new(Size::ZERO, window));
        let layout = Layout::new(tree.size);
        let cursor = mouse::Cursor::Available(at);
        let mut bus = Bus::new();
        for event in [
            mouse::Event::CursorMoved { position: at },
            mouse::Event::ButtonPressed(button),
            mouse::Event::ButtonReleased(button),
        ] {
            // One shell per event, as the runtime does.
            let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut bus);
            element.update(
                &mut tree,
                &Event::Mouse(event),
                layout,
                cursor,
                &renderer,
                &mut shell,
                &Rectangle::with_size(window),
            );
        }
        bus.drain().map(|(message, _)| message).collect()
    }

    /// The centre of the bar's one button, and the centre of its chevron.
    fn button_points() -> (Point, Point) {
        let m = Metrics::GRAPHITE;
        let centre = m.padding + m.btn / 2.0;
        let corner = m.padding + m.btn - m.chevron / 2.0;
        (Point::new(centre, centre), Point::new(corner, corner))
    }

    /// Mockup row 108: a click on the chevron opens the dropdown and does
    /// not run the tool; a click elsewhere still runs it.
    #[test]
    fn a_click_on_the_chevron_sends_its_message_not_the_tool() {
        let (centre, corner) = button_points();
        let left = mouse::Button::Left;
        assert_eq!(press(group_button(Some(CHEVRON)), left, corner), [CHEVRON]);
        assert_eq!(press(group_button(Some(CHEVRON)), left, centre), [RUN]);
    }

    /// Right-click keeps opening the dropdown, on the chevron too.
    #[test]
    fn a_right_click_still_sends_the_right_press_message() {
        let (centre, corner) = button_points();
        let right = mouse::Button::Right;
        assert_eq!(press(group_button(Some(CHEVRON)), right, centre), [RIGHT]);
        assert_eq!(press(group_button(Some(CHEVRON)), right, corner), [RIGHT]);
    }

    /// Without a chevron message a click on the corner runs the tool, as
    /// every bar did before.
    #[test]
    fn without_a_chevron_message_the_corner_runs_the_tool() {
        let (_, corner) = button_points();
        assert_eq!(
            press(group_button(None), mouse::Button::Left, corner),
            [RUN]
        );
    }
}
