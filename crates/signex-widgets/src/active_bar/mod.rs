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

use iced::widget::{button, container, image, mouse_area, row, svg, text, tooltip};
use iced::{Border, Color, Element, Length, Theme};
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
        match self {
            Self::Button(_) => BTN_SIZE,
            Self::Separator => SEP_W,
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
    let mut offsets = Vec::with_capacity(items.len());
    let mut x = BAR_PADDING;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            x += ROW_SPACING;
        }
        offsets.push(x);
        x += item.width();
    }
    (offsets, x + BAR_PADDING)
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

const ICON_SIZE: f32 = 20.0;
const SEP_H: f32 = 18.0;
const BAR_RADIUS: f32 = 4.0;

/// Render the bar + an open dropdown overlay (when one is open).
///
/// `open_menu` indicates which dropdown is currently open (or `None`
/// when nothing is open). When `Some(key)`, the widget renders the
/// bar AND a dropdown panel below it AND a transparent backstop
/// layer behind the bar that fires `close_msg` on any click outside
/// the bar / panel — Altium-style click-outside-to-dismiss.
///
/// `entries_for(key)` is called only when a menu is open and produces
/// the rows for that menu. `width_hint_for(key)` controls the per-
/// menu fixed width (None = auto-size from the chip-wrap layout).
///
/// Both the trigger button on the bar AND the dropdown items emit
/// the editor's own message type `M`, so the editor wires its own
/// state mutations + dispatch arms.
///
/// Single-call API: each editor (schematic / footprint / symbol /
/// upcoming PCB) builds its bar items + a `dropdown_for` closure +
/// a `close_msg`, then mounts THIS widget directly inside its canvas
/// Stack. No per-editor overlay-composition code needed.
pub fn view_with_overlay<'a, M, K>(
    items: Vec<ActiveBarItem<M>>,
    open_menu: Option<K>,
    close_msg: M,
    entries_for: impl Fn(K) -> Vec<crate::active_bar_dropdown::DropdownEntry<M>>,
    width_hint_for: impl Fn(K) -> Option<f32>,
    tokens: &'a ThemeTokens,
) -> Element<'a, M>
where
    M: 'static + Clone,
    K: 'static + Copy,
{
    // The bar centred horizontally at the top of its parent layer.
    // CALLER owns the centring container + vertical offset so the
    // structure matches the schematic's `container(view_bar(...).
    // map(Msg)).width(Fill).align_x(Center)` chain byte-for-byte
    // (preventing a 2 px layout-pass shift from `Element::map`
    // wrapping a container vs being wrapped by a container).
    let bar: Element<'a, M> = view(items, tokens);

    let Some(menu) = open_menu else {
        return bar;
    };

    // Build the dropdown panel + backstop layer.
    let entries = entries_for(menu);
    let width_hint = width_hint_for(menu);
    let panel = crate::active_bar_dropdown::view(entries, tokens, width_hint);
    let panel_anchor = container(panel)
        .padding([46, 10])
        .center_x(Length::Fill)
        .align_y(iced::alignment::Vertical::Top);

    // Backstop: full-area transparent button. Click-outside dismisses
    // the menu via `close_msg`. Mounted UNDER the panel + bar so
    // panel/bar clicks don't fall through to the canvas.
    let backstop = iced::widget::mouse_area(
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(close_msg);

    // When a menu is open, wrap the centred bar in a Stack with the
    // backstop + panel. The bar gets a centring container HERE only;
    // when no menu is open, the caller owns the centring (see early
    // return above) so the no-menu path matches the schematic chain
    // exactly.
    let centred_bar = container(bar)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center);
    iced::widget::Stack::new()
        .push(backstop)
        .push(centred_bar)
        .push(panel_anchor)
        .into()
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
    let bar_bg = theme_ext::to_color(&tokens.panel_bg);
    let border = theme_ext::border_color(tokens);

    let mut row_widget = row![].spacing(ROW_SPACING).align_y(iced::Alignment::Center);
    for item in items {
        row_widget = row_widget.push(view_item(item, tokens));
    }

    container(row_widget)
        .padding(BAR_PADDING)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bar_bg)),
            border: Border {
                width: 1.0,
                radius: BAR_RADIUS.into(),
                color: border,
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

fn view_item<'a, M>(item: ActiveBarItem<M>, tokens: &'a ThemeTokens) -> Element<'a, M>
where
    M: 'static + Clone,
{
    match item {
        ActiveBarItem::Button(b) => view_button(b, tokens),
        ActiveBarItem::Separator => {
            let sep_color = theme_ext::border_color(tokens);
            container(iced::widget::Space::new())
                .width(Length::Fixed(SEP_W))
                .height(Length::Fixed(SEP_H))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(sep_color)),
                    ..iced::widget::container::Style::default()
                })
                .into()
        }
        // Pinned to the declared width so `slot_offsets` can't be lied
        // to: whatever the producer says it is, that is what draws.
        ActiveBarItem::Custom { content, width } => {
            container(content).width(Length::Fixed(width)).into()
        }
    }
}

fn view_button<'a, M>(b: ActiveBarButton<M>, tokens: &'a ThemeTokens) -> Element<'a, M>
where
    M: 'static + Clone,
{
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
    } else if b.selected {
        Color::WHITE
    } else {
        text_c
    };

    let enabled_for_svg = b.enabled;
    let raw_icon: Element<'a, M> = match b.icon {
        ActiveBarIcon::Svg(handle) => svg(handle)
            .width(Length::Fixed(ICON_SIZE))
            .height(Length::Fixed(ICON_SIZE))
            .style(move |_: &Theme, _| iced::widget::svg::Style {
                color: if enabled_for_svg {
                    None
                } else {
                    Some(disabled_tint)
                },
            })
            .into(),
        ActiveBarIcon::Raster(handle) => image(handle)
            .width(Length::Fixed(ICON_SIZE))
            .height(Length::Fixed(ICON_SIZE))
            .into(),
        ActiveBarIcon::Glyph(s) => text(s.to_string()).size(14).color(glyph_color).into(),
    };

    // Optional dropdown chevron in the bottom-right corner.
    let icon_content: Element<'a, M> = if let Some(indicator) = b.dropdown_indicator {
        let indicator_enabled = b.enabled;
        let indicator_el: Element<'a, M> = match indicator {
            ActiveBarIcon::Svg(handle) => svg(handle)
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(14.0))
                .style(move |_: &Theme, _| iced::widget::svg::Style {
                    color: if indicator_enabled {
                        None
                    } else {
                        Some(disabled_tint)
                    },
                })
                .into(),
            ActiveBarIcon::Raster(handle) => image(handle)
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(14.0))
                .into(),
            ActiveBarIcon::Glyph(s) => text(s.to_string()).size(10).color(glyph_color).into(),
        };
        iced::widget::Stack::new()
            .push(
                container(raw_icon)
                    .width(Length::Fixed(BTN_SIZE))
                    .height(Length::Fixed(BTN_SIZE))
                    .center_x(Length::Fixed(BTN_SIZE))
                    .center_y(Length::Fixed(BTN_SIZE)),
            )
            .push(
                container(indicator_el)
                    .width(Length::Fixed(BTN_SIZE))
                    .height(Length::Fixed(BTN_SIZE))
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
            )
            .into()
    } else {
        container(raw_icon)
            .width(Length::Fixed(BTN_SIZE))
            .height(Length::Fixed(BTN_SIZE))
            .center_x(Length::Fixed(BTN_SIZE))
            .center_y(Length::Fixed(BTN_SIZE))
            .into()
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
    let mut btn = button(icon_content).padding(0).style(
        move |_: &Theme, status: iced::widget::button::Status| {
            let bg = if !enabled {
                None
            } else if selected {
                Some(iced::Background::Color(hover_c))
            } else {
                match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                    )),
                    _ => None,
                }
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: border,
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
        mouse_area(btn).on_right_press(rc).into()
    } else {
        btn.into()
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
                },
                ..iced::widget::container::Style::default()
            }),
        tooltip::Position::Bottom,
    )
    .into()
}
