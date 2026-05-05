//! Generic active-bar dropdown widget — used by every editor's active
//! bar (schematic, footprint, future PCB) so chrome stays identical
//! across surfaces while each editor supplies its own actions.
//!
//! Driven by a `Vec<DropdownEntry<M>>`; the widget knows how to draw
//! section headers, separators, disabled rows, glyph + label rows,
//! and an optional checkmark / right-aligned shortcut hint.
//!
//! NOT included here: the trigger button, the toggle state, the
//! click-outside backstop layer. Those live in the editor's view code
//! since they vary per editor (overlay anchoring, message types).

use iced::widget::svg;
use iced::widget::{Column, Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

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
}

impl<M: 'static + Clone> DropdownItem<M> {
    /// Convenience: simple label + on_press, no icon / shortcut.
    pub fn new(label: impl Into<String>, on_press: M) -> Self {
        Self {
            label: label.into(),
            icon: None,
            checked: false,
            shortcut: None,
            disabled: false,
            on_press: Some(on_press),
        }
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
    let panel_bg = theme_ext::to_color(&tokens.panel_bg);
    let border_c = theme_ext::border_color(tokens);
    let primary = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let hover_c = crate::theme_ext::to_color(&tokens.hover);
    let accent = theme_ext::to_color(&tokens.accent);

    let mut col: Column<'a, M> = Column::new().spacing(0).width(Length::Shrink);
    for entry in entries {
        match entry {
            DropdownEntry::Header(label) => {
                col = col.push(
                    container(text(label).size(10).color(muted))
                        .padding([4, 12])
                        .width(Length::Fill),
                );
            }
            DropdownEntry::Separator => {
                col = col.push(
                    container(Space::new())
                        .height(1.0)
                        .width(Length::Fill)
                        .style(move |_: &Theme| iced::widget::container::Style {
                            background: Some(Background::Color(border_c)),
                            ..iced::widget::container::Style::default()
                        }),
                );
            }
            DropdownEntry::Custom(element) => {
                col = col.push(element);
            }
            DropdownEntry::Item(item) => {
                let DropdownItem {
                    label,
                    icon,
                    checked,
                    shortcut,
                    disabled,
                    on_press,
                } = item;
                let row_text_c = if disabled { muted } else { primary };
                // Match the schematic dropdown's vocabulary: 20×20
                // icons (group-default & dropdown items render the
                // same size so the eye doesn't jump as the pointer
                // crosses from bar to menu) + 13 pt label + [5, 12]
                // row padding.
                let mut row_w = row![]
                    .spacing(8)
                    .align_y(iced::Alignment::Center)
                    .width(Length::Shrink);
                // Leading icon column — fixed-width for alignment.
                if let Some(handle) = icon {
                    row_w = row_w.push(
                        svg(handle).width(20).height(20).style(
                            move |_: &Theme, _| iced::widget::svg::Style {
                                color: Some(row_text_c),
                            },
                        ),
                    );
                } else {
                    row_w = row_w.push(Space::new().width(Length::Fixed(20.0)));
                }
                // Label.
                row_w = row_w.push(text(label).size(13).color(row_text_c));
                // Right-aligned cluster: shortcut + check.
                let mut right = row![]
                    .spacing(8)
                    .align_y(iced::Alignment::Center)
                    .width(Length::Shrink);
                if let Some(s) = shortcut {
                    right = right.push(text(s).size(11).color(muted));
                }
                if checked {
                    right = right.push(text("\u{2713}").size(13).color(accent));
                }
                row_w = row_w.push(Space::new().width(Length::Fixed(20.0))).push(right);

                // Wrap in a button that triggers on_press when armed.
                let mut btn = button(
                    container(row_w)
                        .padding([5, 12])
                        .width(Length::Fill),
                )
                .padding(0)
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered if !disabled => Some(Background::Color(hover_c)),
                        _ => None,
                    };
                    button::Style {
                        background: bg,
                        border: Border::default(),
                        text_color: row_text_c,
                        ..button::Style::default()
                    }
                });
                if let Some(msg) = on_press {
                    btn = btn.on_press(msg);
                }
                col = col.push(btn);
            }
        }
    }

    let mut panel = container(col).padding(4);
    if let Some(w) = width_hint {
        panel = panel.width(Length::Fixed(w));
    } else {
        panel = panel.width(Length::Shrink);
    }
    panel
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border_c,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}
