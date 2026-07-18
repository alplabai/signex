//! Shared panel widget helpers (chevrons, section rows, form rows).

use super::*;
use iced::widget::column;

// ─── Helpers ──────────────────────────────────────────────────

// SVG chevrons (same as tree_view for consistency)
const SVG_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M3 1l5 4-5 4z" fill="currentColor"/></svg>"#;
const SVG_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M1 3l4 5 4-5z" fill="currentColor"/></svg>"#;

fn chevron_right() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_RIGHT))
        .clone()
}
fn chevron_down() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_DOWN))
        .clone()
}

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
) -> Column<'a, PanelMsg> {
    let is_collapsed = collapsed.contains(key);
    let chevron_handle = if is_collapsed {
        chevron_right()
    } else {
        chevron_down()
    };
    let key_owned = key.to_string();

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        iced::widget::button(
            container(
                row![
                    svg(chevron_handle)
                        .width(10)
                        .height(10)
                        .style(move |_: &Theme, _| iced::widget::svg::Style {
                            color: Some(header_color),
                        }),
                    text(title.to_string()).size(12).color(header_color),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([6, 8])
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
    );
    col = col.push(thin_sep(border_c));
    col
}

pub fn is_section_collapsed(key: &str, collapsed: &CollapsedSections) -> bool {
    collapsed.contains(key)
}

/// Collapsible section: clickable header with SVG chevron, hides content when collapsed.
pub fn collapsible_section<'a>(
    key: &str,
    title: &str,
    collapsed: &CollapsedSections,
    header_color: Color,
    border_c: Color,
    content: impl FnOnce() -> Column<'a, PanelMsg>,
) -> Column<'a, PanelMsg> {
    let is_collapsed = collapsed.contains(key);
    let mut col = collapsible_section_header(key, title, collapsed, header_color, border_c);
    if !is_collapsed {
        col = col.push(content());
    }
    col
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
            text(key.to_string()).size(10).color(key_c).width(84),
            text(value.to_string()).size(10).color(val_c),
        ]
        .spacing(4),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

pub fn section_title<'a>(title: &str, tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    text(title.to_uppercase())
        .size(9)
        .color(theme_ext::text_secondary(tokens))
}

pub fn separator<'a>(tokens: &ThemeTokens) -> iced::widget::Text<'a> {
    // Use a cached static string instead of allocating on every render
    const SEP: &str = "──────────────────────────────";
    text(SEP).size(4).color(theme_ext::border_color(tokens))
}

pub fn view_stub<'a>(title: &str, desc: &str, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    container(
        column![
            section_title(title, &ctx.tokens),
            separator(&ctx.tokens),
            text(desc.to_string())
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        ]
        .spacing(4)
        .padding(6),
    )
    .width(Length::Fill)
    .into()
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
                .size(10)
                .color(muted)
                .width(Length::Fixed(18.0)),
            text(label.to_string())
                .size(10)
                .color(label_color)
                .width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
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
    .into()
}

/// Wrap a property-row label `text` in a clipped fill-portion container.
/// Plain `text(...).width(FillPortion(...)).wrapping(None)` lays out at
/// the text's intrinsic width and bleeds past the allotted column when
/// the panel is narrow — covering the value column or the panel edge.
/// This helper enforces the FillPortion bound and clips visual overflow
/// inside it. Used by every `form_*_row` and inline property row.
pub fn property_label<'a, M: 'a>(label: impl Into<String>, color: Color) -> Element<'a, M> {
    container(
        text(label.into())
            .size(11)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
    .clip(true)
    .into()
}

/// Like `property_label`, but for size-10 labels (used by `form_edit_row_f64`).
fn property_label_small<'a, M: 'a>(label: impl Into<String>, color: Color) -> Element<'a, M> {
    container(
        text(label.into())
            .size(10)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(Length::FillPortion(2))
    .clip(true)
    .into()
}

/// Numeric edit row used by the shape pre-placement form. Writes on
/// submit — partial text mid-type doesn't panic via parse failure.
pub fn form_edit_row_f64<'a>(
    label: &'a str,
    value: f64,
    muted: Color,
    on_submit: impl Fn(f64) -> PanelMsg + 'a + Clone,
) -> Element<'a, PanelMsg> {
    use iced::widget::{row, text, text_input};
    let buf = format!("{value:.3}");
    let on_submit_cb = on_submit.clone();
    row![
        property_label_small(label, muted),
        text_input("", &buf)
            .size(11)
            .on_input(move |s| {
                if let Ok(v) = s.parse::<f64>() {
                    on_submit_cb(v)
                } else {
                    PanelMsg::Noop
                }
            })
            .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn shape_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{button, row, text};
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        button(text(label).size(10))
            .padding([3, 8])
            .on_press(PanelMsg::SetPrePlacementShapeFill(ft))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(if active {
                    Color::from_rgb(0.20, 0.36, 0.58)
                } else {
                    Color::from_rgba(0.25, 0.25, 0.28, 0.4)
                })),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: Color::from_rgb(0.28, 0.28, 0.32),
                },
                text_color: if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    muted
                },
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    row![
        text("Fill")
            .size(10)
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
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

