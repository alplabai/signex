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
mod metrics;
mod skin;

pub use metrics::Metrics;

use iced::widget::{button, container, image, mouse_area, row, svg, text, tooltip};
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

    /// Width this slot renders at, excluding the row spacing around it.
    pub fn width(&self) -> f32 {
        self.width_with(&Metrics::LEGACY)
    }

    /// [`Self::width`] under `m` (Graphite buttons are wider).
    pub fn width_with(&self, m: &Metrics) -> f32 {
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
pub fn slot_offsets<M: 'static + Clone>(items: &[ActiveBarItem<M>]) -> (Vec<f32>, f32) {
    slot_offsets_with(items, &Metrics::LEGACY)
}

/// [`slot_offsets`] under `m`; the bar `view` draws with `Metrics::of(tokens)`.
pub fn slot_offsets_with<M: 'static + Clone>(
    items: &[ActiveBarItem<M>],
    m: &Metrics,
) -> (Vec<f32>, f32) {
    let widths: Vec<f32> = items.iter().map(|i| i.width_with(m)).collect();
    offsets_for_widths_with(&widths, m)
}

/// [`slot_offsets`] for a bar described only by its slot widths.
///
/// Lets a consumer that knows the bar's slot order but has no item
/// list (a hit-test running outside the view) use the same row maths
/// as the renderer instead of re-deriving it.
pub fn offsets_for_widths(widths: &[f32]) -> (Vec<f32>, f32) {
    offsets_for_widths_with(widths, &Metrics::LEGACY)
}

/// [`offsets_for_widths`] under `m`.
pub fn offsets_for_widths_with(widths: &[f32], m: &Metrics) -> (Vec<f32>, f32) {
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

/// Pixel sizes — match the schematic editor's existing visual
/// rhythm so the bar reads identically across every editor.
///
/// The four that decide *horizontal* layout are public: consumers
/// anchor dropdown panels under specific buttons, which they cannot do
/// without them. Prefer [`slot_offsets`] over doing that arithmetic by
/// hand — copies of these numbers in consumer crates are exactly the
/// drift this is here to prevent.
pub const BTN_SIZE: f32 = 26.0;
/// Width of a [`ActiveBarItem::Separator`].
pub const SEP_W: f32 = 1.0;
/// Padding inside the bar container, all four sides.
pub const BAR_PADDING: f32 = 2.0;
/// Gap the row puts between adjacent slots.
pub const ROW_SPACING: f32 = 2.0;

const BAR_RADIUS: f32 = 4.0;

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
    let bar_bg = theme_ext::to_color(&tokens.panel_bg);
    let border = theme_ext::border_color(tokens);

    let skin = skin::Skin::of(tokens);
    let m = Metrics::of(tokens);
    let mut row_widget = row![].spacing(m.spacing).align_y(iced::Alignment::Center);
    for item in items {
        row_widget = row_widget.push(view_item(item, tokens, &m));
    }

    container(row_widget)
        .padding(m.padding)
        .style(move |_: &Theme| match skin {
            Some(s) => iced::widget::container::Style {
                background: Some(iced::Background::Color(s.fill)),
                border: Border {
                    width: 1.0,
                    radius: s.radius.into(),
                    color: s.border,
                    ..Border::default()
                },
                shadow: s.shadow,
                ..iced::widget::container::Style::default()
            },
            None => iced::widget::container::Style {
                background: Some(iced::Background::Color(bar_bg)),
                border: Border {
                    width: 1.0,
                    radius: BAR_RADIUS.into(),
                    color: border,
                    ..Border::default()
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 8.0,
                },
                ..iced::widget::container::Style::default()
            },
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
    let hover_c = theme_ext::hover_color(tokens);
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
    } else if let Some(s) = skin {
        if b.selected { s.on_selected } else { s.ink }
    } else if b.selected {
        Color::WHITE
    } else {
        text_c
    };
    // Graphite icons are monochrome strokes tinted with the ink token
    // (the bar surface colour on the armed tool); legacy keeps natural
    // colours when enabled.
    let svg_tint: Option<Color> = if !b.enabled {
        Some(disabled_tint)
    } else {
        skin.map(|s| if b.selected { s.on_selected } else { s.ink })
    };

    let raw_icon: Element<'a, M> = match b.icon {
        ActiveBarIcon::Svg(handle) => svg(handle)
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .style(move |_: &Theme, _| iced::widget::svg::Style { color: svg_tint })
            .boxed(),
        ActiveBarIcon::Raster(handle) => image::<image::Handle>(handle)
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .boxed(),
        ActiveBarIcon::Glyph(s) => text(s.to_string()).size(14).color(glyph_color).boxed(),
    };

    // Optional dropdown chevron in the bottom-right corner.
    let icon_content: Element<'a, M> = if let Some(indicator) = b.dropdown_indicator {
        let indicator_el: Element<'a, M> = match indicator {
            ActiveBarIcon::Svg(handle) => svg(handle)
                .width(Length::Fixed(chevron_size))
                .height(Length::Fixed(chevron_size))
                .style(move |_: &Theme, _| iced::widget::svg::Style { color: svg_tint })
                .boxed(),
            ActiveBarIcon::Raster(handle) => image::<image::Handle>(handle)
                .width(Length::Fixed(chevron_size))
                .height(Length::Fixed(chevron_size))
                .boxed(),
            ActiveBarIcon::Glyph(s) => text(s.to_string()).size(10).color(glyph_color).boxed(),
        };
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
    // themes) and the 7 px control radius. Legacy themes keep their paint.
    let mut btn = button(icon_content).padding(0).style(
        move |_: &Theme, status: iced::widget::button::Status| {
            let selected_fill = skin.map_or(hover_c, |s| s.selected);
            let hover_fill = skin.map_or(Color::from_rgba(1.0, 1.0, 1.0, 0.06), |s| s.hover);
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
                    radius: skin.map_or(3.0, |s| s.button_radius).into(),
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
        container(text(tooltip_text).size(11).color(text_c))
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
