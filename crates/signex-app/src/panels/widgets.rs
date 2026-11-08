//! Shared panel widget helpers (chevrons, section rows, form rows).

use super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::column;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

// ─── Helpers ──────────────────────────────────────────────────

pub fn shape_icon_handle(
    elem_type: &str,
    theme: signex_types::theme::ThemeId,
) -> Option<svg::Handle> {
    match elem_type {
        "Line" => Some(crate::icons::icon_shape_line(theme)),
        "Rectangle" => Some(crate::icons::icon_shape_rect(theme)),
        "Circle" => Some(crate::icons::icon_shape_circle(theme)),
        "Arc" => Some(crate::icons::icon_shape_arc(theme)),
        "Polygon" => Some(crate::icons::icon_shape_polygon(theme)),
        _ => None,
    }
}

/// Just the header part of a collapsible section — clickable button
/// with SVG chevron + 1px rule. Returns whether the section is
/// collapsed via `is_collapsed_section(...)` so callers can guard
/// their body push without using a closure.
pub fn collapsible_section_header<'a>(
    key: &str,
    title: &str,
    collapsed: &CollapsedSections,
    header_color: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    let is_collapsed = collapsed.contains(key);
    let key_owned = key.to_string();

    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(Widget::boxed(
        iced::widget::button(
            container(
                row![
                    signex_widgets::chevron::chevron(!is_collapsed, TS.label, header_color),
                    text(title.to_string()).size(TS.label).color(header_color),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([6.0, SP.m])
            .width(Length::Fill),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key_owned))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(border_c)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            }
        }),
    ));
    col = col.push(thin_sep(border_c));
    col
}

pub fn is_section_collapsed(key: &str, collapsed: &CollapsedSections) -> bool {
    collapsed.contains(key)
}

/// Property key-value row (owned strings to avoid lifetime issues in closures).
pub fn prop_kv_row<'a>(
    key: &str,
    value: &str,
    key_c: Color,
    val_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(key.to_string()).size(TS.body).color(key_c).width(84),
            text(value.to_string()).size(TS.body).color(val_c),
        ]
        .spacing(SP.s),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

pub fn section_title<'a>(title: &str, tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    text(title.to_uppercase())
        .size(TS.section)
        .style(crate::styles::section_label(tokens))
}

pub fn separator<'a>(tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    // Use a cached static string instead of allocating on every render
    const SEP: &str = "──────────────────────────────";
    text(SEP)
        .size(TS.micro)
        .color(theme_ext::border_color(tokens))
}

pub fn view_stub<'a>(title: &str, desc: &str, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    container(
        column![
            section_title(title, &ctx.tokens),
            separator(&ctx.tokens),
            text(crate::active_bar::PLANNED_TOOLTIP)
                .size(TS.caption)
                .color(theme_ext::text_primary(&ctx.tokens)),
            text(desc.to_string())
                .size(TS.caption)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .spacing(SP.s)
        .padding(6),
    )
    .width(Length::Fill)
    .boxed()
}

/// Render one indented part row inside the SCH Library tree-expander.
/// Active part gets the selection background; otherwise the row hovers
/// like the symbol rows above it.
pub fn part_tree_row<'a>(
    label: &str,
    part: u8,
    is_active: bool,
    primary: Color,
    muted: Color,
    bg_active: Color,
) -> Element<'a, PanelMsg> {
    let label_color = if is_active { primary } else { muted };
    iced::widget::button(
        row![
            // Tree-expander indent: 18 px gutter + a faint glyph so
            // the part rows visually nest under the symbol.
            text("\u{2514}")
                .size(TS.body)
                .color(muted)
                .width(Length::Fixed(18.0)),
            text(label.to_string())
                .size(TS.body)
                .color(label_color)
                .width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .on_press(PanelMsg::SymEditorSelectPart(part))
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| iced::widget::button::Style {
            background: if is_active {
                Some(iced::Background::Color(bg_active))
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                )))
            } else {
                None
            },
            border: iced::Border::default(),
            text_color: label_color,
            ..iced::widget::button::Style::default()
        },
    )
    .boxed()
}

/// Width of the Properties label column on Graphite: the mock's fixed grid
/// column, so a label such as "Designator" is never clipped by a narrow dock.
pub const GRAPHITE_PROPERTY_LABEL_WIDTH: f32 = 92.0;

/// Colour and column width of a property-row label. A bare [`Color`] keeps
/// the legacy fill-portion column; [`LabelInk::graphite`] picks the
/// fixed Graphite column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelInk {
    pub color: Color,
    pub width: Length,
}

impl From<Color> for LabelInk {
    fn from(color: Color) -> Self {
        Self {
            color,
            width: Length::FillPortion(PROPERTY_LABEL_PORTION),
        }
    }
}

impl LabelInk {
    /// The Graphite label look: the fixed column.
    pub fn graphite(color: Color) -> Self {
        Self {
            color,
            width: Length::Fixed(GRAPHITE_PROPERTY_LABEL_WIDTH),
        }
    }
}

/// Property-row label: wraps at spaces inside its column, so a narrow panel
/// grows the row; a single over-long word is clipped, never split mid-word,
/// and never reaches the value column.
pub fn property_label<'a, M: 'a>(
    label: impl Into<String>,
    ink: impl Into<LabelInk>,
) -> Element<'a, M> {
    let ink = ink.into();
    property_label_sized(label, ink.color, TS.body, ink.width)
}

/// Like `property_label`, but with a narrower label column (used by `form_edit_row_f64`).
fn property_label_small<'a, M: 'a>(label: impl Into<String>, color: Color) -> Element<'a, M> {
    property_label_sized(label, color, TS.body, Length::FillPortion(2))
}

fn property_label_sized<'a, M: 'a>(
    label: impl Into<String>,
    color: Color,
    size: f32,
    width: Length,
) -> Element<'a, M> {
    container(
        text(label.into())
            .size(size)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .width(width)
    .clip(true)
    .boxed()
}

/// Numeric edit row used by the shape pre-placement form. The typed text
/// stays as a draft while the box is edited, so `-` and `1.` survive; each
/// parsable keystroke is applied, and Enter shows the stored value again.
pub fn form_edit_row_f64<'a>(
    label: &'a str,
    value: f64,
    key: crate::panels::NumericFieldKey,
    draft: Option<&crate::panels::NumericDraft>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::row;
    row![
        property_label_small(label, muted),
        crate::panels::numeric_input("", key, value, draft).size(TS.body),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .boxed()
}

pub fn shape_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{button, row, text};
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        button(text(label).size(TS.body))
            .padding([3.0, SP.m])
            .on_press(PanelMsg::SetPrePlacementShapeFill(ft))
            .style(super::form_look::option_tile(active))
            .boxed()
    };
    row![
        text("Fill")
            .size(TS.body)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile("None", FillType::None, current == FillType::None),
            tile("Outline", FillType::Outline, current == FillType::Outline),
            tile(
                "Background",
                FillType::Background,
                current == FillType::Background
            ),
        ]
        .spacing(SP.s)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .boxed()
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn graphite_labels_get_the_fixed_92_px_column() {
        let ink = LabelInk::graphite(Color::BLACK);
        assert_eq!(ink.width, Length::Fixed(92.0));
    }
}
