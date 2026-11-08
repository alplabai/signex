//! Generic active-bar dropdown widget — used by every editor's active
//! bar (schematic, footprint, future PCB) so chrome stays identical
//! across surfaces while each editor supplies its own actions.
//!
//! ## Contract for new editors (PCB integration target)
//!
//! 1. Define a `*_active_bar_menu: Option<MenuKind>` field on the
//!    editor's state to track which dropdown is open.
//! 2. Add a `ToggleActiveBarMenu(MenuKind)` message variant + a
//!    `CloseActiveBarMenu` message; the dispatcher toggles / clears
//!    the field.
//! 3. Build a `dropdowns.rs` module exposing
//!    `entries(menu, state, path, theme_id, ...) ->
//!    Vec<DropdownEntry<EditorMessage>>` with one match arm per menu.
//! 4. In the editor's active-bar `view` function, render the open
//!    dropdown via `signex_widgets::active_bar_dropdown::view(entries,
//!    tokens, width_hint)` and stack it in a `Stack` overlay layer
//!    above the bar with a transparent backstop layer for click-
//!    outside-to-dismiss.
//!
//! ## What this widget renders
//!
//! Driven by a `Vec<DropdownEntry<M>>`; the widget knows how to draw
//! section headers, separators, disabled rows, glyph + label rows,
//! and an optional checkmark / right-aligned shortcut hint. The
//! `Custom(Element<M>)` escape hatch lets editors compose chip-grid
//! layouts (the Selection Filter dropdown's pill grid) without
//! re-implementing the panel chrome.
//!
//! ## Helpers
//!
//! - `chip_btn(label, on_press, enabled, accent) -> Element<M>` —
//!   Altium-style toggle chip used inside `DropdownEntry::Custom` for
//!   chip-grid layouts. Identical chrome across all editors.
//!
//! ## NOT included here
//!
//! - The trigger button (`signex_widgets::active_bar::ActiveBarButton`
//!   handles that — left-click action + right-click dropdown +
//!   chevron indicator).
//! - The toggle state (each editor's state owns it).
//! - The click-outside backstop layer (each editor mounts its own).

use iced::widget::svg;
use iced::widget::{Column, Row, Space, button, container, mouse_area, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

mod metrics;

pub use metrics::{MONO_FAMILY, MenuMetrics};

/// Hover wash and corner radius of a menu row. Public so a `Custom` row
/// built by the app (the net-colour swatches) hovers like an item row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowPaint {
    pub hover: Color,
    pub radius: f32,
}

impl RowPaint {
    /// The same hover and radius the dropdown's own item rows use.
    pub fn of(tokens: &ThemeTokens) -> Self {
        Self {
            hover: Look::of(tokens).hover,
            radius: MenuMetrics::of(tokens).row_radius,
        }
    }
}

/// Text size of a menu row label on the legacy look.
pub const LABEL_SIZE: f32 = MenuMetrics::LEGACY.label_size;

/// Glyph of the submenu chevron column.
const CHEVRON: &str = "\u{203A}";

/// One row inside the dropdown. The widget walks the slice in order
/// so semantic grouping (section header → items → separator → next
/// section) reads top-down at the call site.
pub enum DropdownEntry<M>
where
    M: 'static + Clone,
{
    /// Bold non-clickable header above a logical group of items.
    Header(String),
    /// 1-px horizontal separator between groups.
    Separator,
    /// Clickable row with label, optional leading icon, optional
    /// checkmark indicator (rendered to the right of the label),
    /// optional shortcut hint, and optional disabled flag.
    Item(DropdownItem<M>),
    /// Arbitrary user-supplied widget — escape hatch for menus that
    /// can't be expressed as a vertical list of `Item` rows (e.g. the
    /// Selection Filter chip-wrap grid). Padding/border are still
    /// applied by the panel container.
    Custom(Element<'static, M>),
}

pub struct DropdownItem<M>
where
    M: 'static + Clone,
{
    pub label: String,
    /// Leading icon. `None` for items without a glyph.
    pub icon: Option<svg::Handle>,
    /// `true` paints a small "✓" to the right of the label so the
    /// user sees which row is currently active (toggle items).
    pub checked: bool,
    /// Optional right-aligned shortcut text (e.g. "Shift+E").
    pub shortcut: Option<String>,
    /// `true` greys the row out and ignores clicks.
    pub disabled: bool,
    /// Message published when the user clicks this row. `None` =
    /// passive (used for "coming soon" stubs).
    pub on_press: Option<M>,
    /// Hover text for a row that cannot be used yet (e.g. "Planned").
    pub tooltip: Option<String>,
    /// Id of the command this row runs, when it runs one. The widget never
    /// reads it; menu builders set it so a test can walk the rows and check
    /// each command row's label and shortcut against the command catalog.
    pub command_id: Option<&'static str>,
    /// The row opens a submenu: it draws the chevron in the chevron column.
    pub submenu: bool,
    /// Messages published when the pointer enters and leaves the row (a
    /// submenu launcher's hover timer).
    pub on_hover: Option<(M, M)>,
    /// The row's submenu is open: it keeps a highlight, and the flyout
    /// lines up with it ([`MenuMetrics::active_submenu_top`]).
    pub active: bool,
}

impl<M: 'static + Clone> DropdownItem<M> {
    /// Convenience: simple label + on_press, no icon / shortcut.
    pub fn new(label: impl Into<String>, on_press: M) -> Self {
        Self::passive(label).on_press(on_press)
    }

    /// A row that publishes nothing, for a stub or a gated action.
    pub fn passive(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            checked: false,
            shortcut: None,
            disabled: false,
            on_press: None,
            tooltip: None,
            command_id: None,
            submenu: false,
            on_hover: None,
            active: false,
        }
    }

    /// Builder: publish `message` on click.
    fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    /// Builder: record the id of the command this row runs.
    pub fn command(mut self, id: &'static str) -> Self {
        self.command_id = Some(id);
        self
    }

    /// Builder: attach a hover tooltip (shown on disabled rows too).
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Builder: mark this item as currently active (paints a "✓").
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }

    /// Builder: attach a leading SVG icon.
    pub fn icon(mut self, handle: svg::Handle) -> Self {
        self.icon = Some(handle);
        self
    }

    /// Builder: attach a right-aligned shortcut hint.
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }

    /// Builder: disable the row (greys + drops on_press).
    pub fn disabled(mut self, off: bool) -> Self {
        self.disabled = off;
        if off {
            self.on_press = None;
        }
        self
    }

    /// Builder: the row opens a submenu (draws the chevron).
    pub fn submenu(mut self) -> Self {
        self.submenu = true;
        self
    }

    /// Builder: publish `enter` / `leave` as the pointer crosses the row.
    pub fn hover(mut self, enter: M, leave: M) -> Self {
        self.on_hover = Some((enter, leave));
        self
    }

    /// Builder: the row's submenu is open.
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }
}

/// Chrome of the small hover hint shown on a planned row, shared by the
/// dropdowns here and the application menu bar so the two cannot drift.
pub fn tooltip_style(
    background: Color,
    border: Color,
) -> impl Fn(&Theme) -> container::Style + 'static {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// SVG style for a row icon. On the legacy look an enabled row keeps the
/// SVG's own colours (`color: None`) and a disabled one is tinted muted;
/// `Some(c)` recolours the whole SVG. The Graphite look draws every icon
/// monochrome, in `ink` (or `muted` when disabled).
fn icon_style(disabled: bool, muted: Color, ink: Option<Color>) -> svg::Style {
    svg::Style {
        color: if disabled { Some(muted) } else { ink },
    }
}

/// Width a menu of `entries` needs in `tokens`' look, at most `max_w`; see
/// [`MenuMetrics::fitted_width`].
pub fn fitted_width<M>(
    entries: &[DropdownEntry<M>],
    tokens: &ThemeTokens,
    max_w: f32,
    ui_family: &str,
) -> f32
where
    M: Clone,
{
    MenuMetrics::of(tokens).fitted_width(entries, max_w, ui_family)
}

/// Colours of one menu card, resolved once per view.
#[derive(Clone, Copy)]
struct Look {
    primary: Color,
    muted: Color,
    accent: Color,
    hover: Color,
    active: Color,
    /// Tint of an enabled icon; `None` keeps the SVG's own colours.
    icon_ink: Option<Color>,
    panel_bg: Color,
    border: Color,
}

impl Look {
    fn of(tokens: &ThemeTokens) -> Self {
        let skin = super::skin::Skin::of(tokens);
        let hover = skin.map_or(theme_ext::to_color(&tokens.hover), |s| s.hover);
        Self {
            primary: theme_ext::text_primary(tokens),
            muted: theme_ext::text_secondary(tokens),
            accent: theme_ext::to_color(&tokens.accent),
            hover,
            active: skin.map_or(theme_ext::to_color(&tokens.selection), |s| s.hover),
            icon_ink: skin.map(|s| s.icon_ink),
            panel_bg: theme_ext::to_color(&tokens.panel_bg),
            border: theme_ext::border_color(tokens),
        }
    }
}

/// Render the dropdown panel as an `Element<M>`. `width_hint`
/// specifies a fixed panel width in px (e.g. 220) when the menu is
/// list-style; `None` lets the panel auto-size (used for the Filter
/// chip-grid that drives its own width). Caller wraps the result in a
/// Translate / Stack overlay layer at the chevron's anchor and pairs
/// it with a transparent backstop for click-outside-to-dismiss.
pub fn view<'a, M>(
    entries: Vec<DropdownEntry<M>>,
    tokens: &'a ThemeTokens,
    width_hint: Option<f32>,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let metrics = MenuMetrics::of(tokens);
    let look = Look::of(tokens);
    // Graphite: the card matches the bar (raised fill, 10 px radius, themed
    // shadow, soft separators). Legacy themes keep the 4 px panel.
    let skin = super::skin::Skin::of(tokens);
    let line = skin.map_or(look.border, |s| s.line_soft);

    // Fill children inside a Shrink column lay out at zero width in the
    // fork, so a fixed-width panel gets a Fill column (buttons stay clickable)
    // and an unsized panel fits its content.
    let fixed = width_hint.is_some();
    let col_width = if fixed { Length::Fill } else { Length::Fit };
    let mut col: Column<Element<'a, M>> = Column::new().spacing(0).width(col_width);
    let shape = RowShape {
        has_icons: metrics::has_icons(&entries),
        has_chevrons: metrics::has_chevrons(&entries),
        fixed,
    };
    for entry in entries {
        let row = match entry {
            DropdownEntry::Header(label) => container(
                text(label)
                    .size(metrics.header_size)
                    .color(look.muted)
                    .wrapping(text::Wrapping::None),
            )
            .padding([0.0, metrics.row_pad_x])
            .height(metrics.header_row_height())
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .boxed(),
            DropdownEntry::Separator => container(
                container(Space::new())
                    .height(1.0)
                    .width(Length::Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(line)),
                        ..container::Style::default()
                    }),
            )
            .padding([metrics.separator_margin, 0.0])
            .width(Length::Fill)
            .boxed(),
            DropdownEntry::Custom(element) => element,
            DropdownEntry::Item(item) => item_row(item, &metrics, look, shape),
        };
        col = col.push(row);
    }

    let mut panel = container(col).padding(metrics.panel_pad);
    if let Some(w) = width_hint {
        panel = panel.width(Length::Fixed(w));
    } else {
        panel = panel.width(Length::Fit);
    }
    panel
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(skin.map_or(look.panel_bg, |s| s.fill))),
            border: Border {
                width: 1.0,
                radius: skin.map_or(4.0, |s| s.radius).into(),
                color: look.border,
                ..Border::default()
            },
            // Drop shadow lifts the popup off the canvas — shared by every
            // editor's active-bar dropdown so the chrome reads identically.
            shadow: skin.map_or(
                iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                |s| s.shadow,
            ),
            ..container::Style::default()
        })
        .boxed()
}

/// Which columns every row of one card reserves.
#[derive(Clone, Copy)]
struct RowShape {
    has_icons: bool,
    has_chevrons: bool,
    /// The card has a fixed width, so the shortcut column can be pushed to
    /// the right edge.
    fixed: bool,
}

/// One item row: icon column, label, the right-aligned shortcut, the check
/// and the chevron column, inside a button exactly one row tall and as wide
/// as the card, so the hover fill drawn is the rect that takes the click.
fn item_row<'a, M>(
    item: DropdownItem<M>,
    m: &MenuMetrics,
    look: Look,
    shape: RowShape,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let DropdownItem {
        label,
        icon,
        checked,
        shortcut,
        disabled,
        on_press,
        tooltip: tooltip_text,
        command_id: _,
        submenu,
        on_hover,
        active,
    } = item;
    let text_c = if disabled { look.muted } else { look.primary };
    let mut cells: Row<Element<'a, M>> = Row::new()
        .spacing(m.row_spacing)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    if let Some(handle) = icon {
        let (muted, ink) = (look.muted, look.icon_ink);
        cells = cells.push(
            svg(handle)
                .width(m.icon_size)
                .height(m.icon_size)
                .style(move |_: &Theme, _| icon_style(disabled, muted, ink))
                .boxed(),
        );
    } else if shape.has_icons {
        cells = cells.push(Space::new().width(Length::Fixed(m.icon_size)).boxed());
    }
    // Menu rows never wrap: a too-narrow card clips rather than reflowing a
    // label onto two lines.
    cells = cells.push(
        text(label)
            .size(m.label_size)
            .color(text_c)
            .wrapping(text::Wrapping::None)
            .boxed(),
    );
    // A fixed card pushes the shortcut column to its right edge; an unsized
    // one keeps the minimum gap (a Fill cell there would size to nothing).
    let gap = if shape.fixed {
        Length::Fill
    } else {
        Length::Fixed(m.cluster_gap)
    };
    cells = cells.push(Space::new().width(gap).boxed());
    if let Some(s) = shortcut {
        let mut hint = text(s)
            .size(m.shortcut_size)
            .color(look.muted)
            .wrapping(text::Wrapping::None);
        if m.mono_shortcut {
            hint = hint.font(iced::Font::with_family(iced::font::Family::name(
                MONO_FAMILY,
            )));
        }
        cells = cells.push(hint.boxed());
    }
    if checked {
        cells = cells.push(
            text("\u{2713}")
                .size(m.check_size)
                .color(look.accent)
                .boxed(),
        );
    }
    if shape.has_chevrons {
        cells = cells.push(
            text(if submenu { CHEVRON } else { "" })
                .size(m.chevron_size)
                .color(look.muted)
                .width(Length::Fixed(m.chevron_width))
                .align_x(iced::alignment::Horizontal::Right)
                .boxed(),
        );
    }

    let (hover, active_c, radius) = (look.hover, look.active, m.row_radius);
    let mut btn = button(
        container(cells)
            .padding([0.0, m.row_pad_x])
            .height(m.item_height(shape.has_icons))
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed if !disabled => Some(hover),
            _ if active => Some(active_c),
            _ => None,
        };
        button::Style {
            background: bg.map(Background::Color),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            text_color: text_c,
            ..button::Style::default()
        }
    });
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    let row = match tooltip_text {
        Some(hint) => iced::widget::tooltip(
            btn,
            container(text(hint).size(11).color(look.primary))
                .padding([3, 8])
                .style(tooltip_style(look.panel_bg, look.border)),
            iced::widget::tooltip::Position::Right,
        )
        .gap(4)
        .boxed(),
        None => btn.boxed(),
    };
    match on_hover {
        Some((enter, leave)) => mouse_area(row).on_enter(enter).on_exit(leave).boxed(),
        None => row,
    }
}

/// Altium-style toggle chip — used inside `DropdownEntry::Custom`
/// to build chip-wrap layouts (Selection Filter pill grids in the
/// schematic / footprint / future PCB editors).
///
/// `enabled = true` paints the chip with the accent border + active
/// background; `false` shows the muted inactive treatment. Click
/// fires `on_press`.
///
/// Caller composes a `Wrap` or `column![row![...], row![...]]`
/// from these chips and feeds the result into
/// `DropdownEntry::Custom(...)` so the same chrome lights up in
/// every editor.
pub fn chip_btn<M>(
    label: impl Into<String>,
    on_press: M,
    enabled: bool,
    accent: Color,
) -> Element<'static, M>
where
    M: 'static + Clone,
{
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    button(
        text(label.into())
            .size(11)
            .color(if enabled { text_on } else { text_off }),
    )
    .padding([4, 10])
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)),
            _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
        };
        button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: accent,
                ..Border::default()
            },
            text_color: if enabled { text_on } else { text_off },
            ..button::Style::default()
        }
    })
    .boxed()
}

#[cfg(test)]
mod tests;
