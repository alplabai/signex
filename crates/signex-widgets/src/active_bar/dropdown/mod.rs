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
//!   toggle chip used inside `DropdownEntry::Custom` for
//!   chip-grid layouts. Identical chrome across all editors.
//!
//! ## NOT included here
//!
//! - The trigger button (`signex_widgets::active_bar::ActiveBarButton`
//!   handles that — left-click action + right-click dropdown +
//!   chevron indicator).
//! - The toggle state (each editor's state owns it).
//! - The click-outside backstop layer (each editor mounts its own).

use crate::{Label, button, raw, text};
use iced::widget::svg;
use iced::widget::{Column, Row, Space, container, mouse_area};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

mod metrics;

pub use metrics::{MENU_LABEL_SIZE, MENU_SHORTCUT_SIZE, MONO_FAMILY, MenuMetrics};

/// Hover wash and corner radius of a menu row. Public so a `Custom` row
/// built by the app hovers like an item row.
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
            radius: MenuMetrics::GRAPHITE.row_radius,
        }
    }
}

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
    /// Leading colour swatch, drawn in the icon column in place of an
    /// icon (the net-colour menu). An `icon` wins when both are set.
    pub swatch: Option<Color>,
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
    /// Ink of the row icon in place of the menu's icon ink (a power port
    /// drawn in the power colour). A disabled row still draws it muted.
    pub icon_tint: Option<Color>,
    /// Right-aligned caption (a grid step's mm value), drawn like a
    /// shortcut hint but not one.
    pub caption: Option<String>,
    /// The row is the current choice: accent-soft fill and ink, plus the
    /// check when `checked`.
    pub selected: bool,
    /// The row is the menu's default action (the row a double-click runs):
    /// its label is semibold.
    pub strong: bool,
    /// Label and icon draw in the theme's danger ink (a destructive row);
    /// the hover wash stays the same as every other row.
    pub danger: bool,
    /// A small outlined pill right of the label (the "planned" marker), a
    /// catalog message like any other new text.
    pub badge: Option<Label<'static>>,
    /// `Some` draws an accent check in the icon column (`Some(true)`) or
    /// leaves the column empty (`Some(false)`), in place of the icon and
    /// the trailing check; `None` is an ordinary row.
    pub lead_check: Option<bool>,
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
            swatch: None,
            checked: false,
            shortcut: None,
            disabled: false,
            on_press: None,
            tooltip: None,
            command_id: None,
            submenu: false,
            on_hover: None,
            active: false,
            icon_tint: None,
            caption: None,
            selected: false,
            strong: false,
            danger: false,
            badge: None,
            lead_check: None,
        }
    }

    /// Builder: draw the icon in `ink` (still muted when disabled).
    pub fn icon_tint(mut self, ink: Color) -> Self {
        self.icon_tint = Some(ink);
        self
    }

    /// Builder: a right-aligned caption.
    pub fn caption(mut self, text: impl Into<String>) -> Self {
        self.caption = Some(text.into());
        self
    }

    /// Builder: mark the row as the current choice (accent-soft fill).
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    /// Builder: semibold label for the menu's default row.
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
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

    /// Builder: attach a leading colour swatch in the icon column.
    pub fn swatch(mut self, color: Color) -> Self {
        self.swatch = Some(color);
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

    /// Builder: draw the label and icon in the danger ink.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// Builder: show `text` in an outlined pill right of the label.
    pub fn badge(mut self, text: impl Into<Label<'static>>) -> Self {
        self.badge = Some(text.into());
        self
    }

    /// Builder: an accent check in the icon column when `on`, an empty
    /// column when not. The row reserves the column like an icon row and
    /// draws neither its icon nor the trailing check.
    pub fn lead_check(mut self, on: bool) -> Self {
        self.lead_check = Some(on);
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

/// SVG style for a row icon: monochrome, in `ink` (or `muted` when disabled).
fn icon_style(disabled: bool, muted: Color, ink: Color) -> svg::Style {
    svg::Style {
        color: Some(if disabled { muted } else { ink }),
        ..Default::default()
    }
}

/// Colours of one menu card, resolved once per view.
#[derive(Clone, Copy)]
struct Look {
    primary: Color,
    muted: Color,
    accent: Color,
    hover: Color,
    active: Color,
    /// Tint of a row icon.
    icon_ink: Color,
    /// Label and icon of a destructive row.
    danger: Color,
    /// Outline of a badge pill (the soft line token).
    line: Color,
    panel_bg: Color,
    border: Color,
    /// Fill and ink of the current-choice row.
    selected_fill: Color,
    selected_ink: Color,
    /// Hairline around a colour swatch.
    swatch_edge: Color,
}

/// Alpha of the accent wash a theme without an accent-soft token gets.
const SELECTED_FALLBACK_ALPHA: f32 = 0.15;

impl Look {
    fn of(tokens: &ThemeTokens) -> Self {
        let skin = super::skin::Skin::of(tokens);
        let e = &tokens.extended;
        let accent = theme_ext::to_color(&tokens.accent);
        let primary = theme_ext::text_primary(tokens);
        let border = theme_ext::border_color(tokens);
        Self {
            primary,
            muted: theme_ext::text_secondary(tokens),
            accent,
            hover: skin.hover,
            active: skin.hover,
            icon_ink: skin.icon_ink,
            danger: theme_ext::error_color(tokens),
            line: skin.line_soft,
            panel_bg: theme_ext::to_color(&tokens.panel_bg),
            border,
            selected_fill: e.accent_soft.as_ref().map_or(
                Color {
                    a: SELECTED_FALLBACK_ALPHA,
                    ..accent
                },
                theme_ext::to_color,
            ),
            selected_ink: e
                .accent_soft_ink
                .as_ref()
                .map_or(primary, theme_ext::to_color),
            swatch_edge: e.line_strong.as_ref().map_or(border, theme_ext::to_color),
        }
    }

    /// Ink of a row label: muted when disabled (a greyed danger row reads
    /// greyed, not red), the danger ink on a destructive row, else primary.
    fn label_ink(&self, disabled: bool, danger: bool) -> Color {
        if disabled {
            self.muted
        } else if danger {
            self.danger
        } else {
            self.primary
        }
    }

    /// Ink of a live row icon: the danger ink on a destructive row.
    fn row_icon_ink(&self, danger: bool) -> Color {
        if danger { self.danger } else { self.icon_ink }
    }
}

/// Face of a row label: the semibold weight for the strong (default) row.
fn label_font(strong: bool) -> Option<iced::Font> {
    strong.then_some(iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::ui_font::regular()
    })
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
    let metrics = MenuMetrics::GRAPHITE;
    let look = Look::of(tokens);
    // The card matches the bar (raised fill, 10 px radius, themed shadow,
    // soft separators).
    let skin = super::skin::Skin::of(tokens);
    let line = skin.line_soft;

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
                text(raw(label))
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
            background: Some(Background::Color(skin.fill)),
            border: Border {
                width: 1.0,
                radius: skin.radius.into(),
                color: look.border,
                ..Border::default()
            },
            // Drop shadow lifts the popup off the canvas — shared by every
            // editor's active-bar dropdown so the chrome reads identically.
            shadow: skin.shadow,
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
        swatch,
        checked,
        shortcut,
        disabled,
        on_press,
        tooltip: tooltip_text,
        command_id: _,
        submenu,
        on_hover,
        active,
        icon_tint,
        caption,
        selected,
        strong,
        danger,
        badge,
        lead_check,
    } = item;
    // The current choice reads in the accent-soft ink unless it is greyed
    // or destructive.
    let text_c = if selected && !disabled && !danger {
        look.selected_ink
    } else {
        look.label_ink(disabled, danger)
    };
    let mut cells: Row<Element<'a, M>> = Row::new()
        .spacing(m.row_spacing)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    // A leading check takes the icon column in place of the icon or swatch.
    match lead_check {
        Some(true) => {
            cells = cells.push(
                container(check_mark(m, look.accent))
                    .width(Length::Fixed(m.icon_size))
                    .align_x(iced::alignment::Horizontal::Center)
                    .boxed(),
            )
        }
        Some(false) => {
            cells = cells.push(Space::new().width(Length::Fixed(m.icon_size)).boxed());
        }
        None if shape.has_icons => {
            cells = cells.push(lead_cell(
                icon, swatch, icon_tint, disabled, danger, m, look,
            ));
        }
        None => {}
    }
    // Menu rows never wrap: a too-narrow card clips rather than reflowing a
    // label onto two lines.
    let mut label_text = text(raw(label))
        .size(m.label_size)
        .color(text_c)
        .wrapping(text::Wrapping::None);
    if let Some(font) = label_font(strong) {
        label_text = label_text.font(font);
    }
    cells = cells.push(label_text.boxed());
    // A fixed card pushes the shortcut column to its right edge; an unsized
    // one keeps the minimum gap (a Fill cell there would size to nothing).
    let gap = if shape.fixed {
        Length::Fill
    } else {
        Length::Fixed(m.cluster_gap)
    };
    cells = cells.push(Space::new().width(gap).boxed());
    if let Some(pill) = badge {
        cells = cells.push(badge_pill(pill, m, look));
    }
    for s in [caption, shortcut].into_iter().flatten() {
        let mut hint = text(raw(s))
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
    if checked && lead_check.is_none() {
        cells = cells.push(check_mark(m, look.accent));
    }
    if shape.has_chevrons {
        cells = cells.push(
            text(raw(if submenu { CHEVRON } else { "" }))
                .size(m.chevron_size)
                .color(look.muted)
                .width(Length::Fixed(m.chevron_width))
                .align_x(iced::alignment::Horizontal::Right)
                .boxed(),
        );
    }

    let (hover, radius) = (look.hover, m.row_radius);
    // The open launcher keeps the hover wash; the current choice its fill.
    let resting = if selected {
        Some(look.selected_fill)
    } else if active {
        Some(look.active)
    } else {
        None
    };
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
            button::Status::Hovered | button::Status::Pressed if !disabled && !selected => {
                Some(hover)
            }
            _ => resting,
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
        Some(hint) => crate::tooltip(
            btn,
            container(text(raw(hint)).size(11).color(look.primary))
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

/// The accent check of a checked row, trailing or in the icon column.
fn check_mark<'a, M: 'a>(m: &MenuMetrics, color: Color) -> Element<'a, M> {
    text(raw("\u{2713}"))
        .size(m.check_size)
        .color(color)
        .boxed()
}

/// The outlined pill of a row badge: caption text in the muted ink inside a
/// soft-line outline with round ends.
fn badge_pill<'a, M: 'a>(label: Label<'static>, m: &MenuMetrics, look: Look) -> Element<'a, M> {
    let line = look.line;
    container(
        text(label)
            .size(m.badge_size)
            .color(look.muted)
            .wrapping(text::Wrapping::None),
    )
    .padding([0.0, m.badge_pad_x])
    .height(m.badge_height)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: theme_ext::Radii::GRAPHITE.pill.into(),
            color: line,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The icon column of one row: an icon (in `tint` when set, in the danger
/// ink on a destructive row), a colour swatch or nothing, always in one
/// `icon_size` square with its content centred, so every label of a card
/// starts at the same x.
fn lead_cell<'a, M>(
    icon: Option<svg::Handle>,
    swatch: Option<Color>,
    tint: Option<Color>,
    disabled: bool,
    danger: bool,
    m: &MenuMetrics,
    look: Look,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let content: Element<'a, M> = match (icon, swatch) {
        (Some(handle), _) => {
            let ink = match tint {
                Some(tint) if !danger => tint,
                _ => look.row_icon_ink(danger),
            };
            let muted = look.muted;
            svg(handle)
                .width(m.icon_size)
                .height(m.icon_size)
                .style(move |_: &Theme, _| icon_style(disabled, muted, ink))
                .boxed()
        }
        (None, Some(color)) => {
            let (edge, side) = (look.swatch_edge, m.swatch_size());
            container(Space::new())
                .width(side)
                .height(side)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(color)),
                    border: Border {
                        width: 1.0,
                        radius: (side / 4.0).into(),
                        color: edge,
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed()
        }
        (None, None) => Space::new().boxed(),
    };
    container(content)
        .width(Length::Fixed(m.icon_size))
        .height(Length::Fixed(m.icon_size))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
}

/// Toggle chip — used inside `DropdownEntry::Custom`
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
        text(raw(label.into()))
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
