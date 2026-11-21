//! Shared panel widget helpers (chevrons, section rows, form rows).

use super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;

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
        signex_widgets::button(
            container(
                row![
                    signex_widgets::chevron::chevron(!is_collapsed, TS.label, header_color),
                    // raw: footprint-editor section titles move with #2060.
                    text(raw(title.to_string()))
                        .size(TS.label)
                        .color(header_color),
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

/// The entry `ToggleSection` flips for section `key`: the key itself for a
/// section open by default (recorded while closed), `<key>.open` for one
/// closed by default (recorded while open). The one encoding every panel
/// with default-closed sections uses.
pub fn section_toggle_key(key: &str, closed_by_default: bool) -> String {
    if closed_by_default {
        format!("{key}.open")
    } else {
        key.to_owned()
    }
}

/// Whether section `key` shows its body: a default-open one unless its key
/// is recorded, a default-closed one only while its open key is.
pub fn is_section_open(key: &str, closed_by_default: bool, collapsed: &CollapsedSections) -> bool {
    collapsed.contains(&section_toggle_key(key, closed_by_default)) == closed_by_default
}

/// Property key-value row: the key is UI text, the value is design data
/// (owned, to avoid lifetime issues in closures).
pub fn prop_kv_row<'a>(
    key: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    key_c: Color,
    val_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(key).size(TS.body).color(key_c).width(84),
            // raw: the value is design data (a position, a size), not language.
            text(raw(value.to_string())).size(TS.body).color(val_c),
        ]
        .spacing(SP.s),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

pub fn section_title<'a, 'b>(
    title: impl Into<signex_widgets::Label<'b>>,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> iced::widget::Text<'a> {
    let title = title.into();
    // raw: the title as given, upper-cased for the locale.
    text(raw(i18n.upper_for_display(title.as_str())))
        .size(TS.section)
        .style(crate::styles::section_label(tokens))
}

/// A planned panel (#1283): the shared planned card with the panel's icon,
/// its name (`title`), the planned pill and what it will do (`desc`). The
/// dock title is the panel's only title row.
pub fn view_stub<'a>(
    title: signex_i18n::MessageId,
    desc: signex_i18n::MessageId,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    super::empty_state::planned_card(
        &ctx.tokens,
        planned_icon(title),
        ctx.i18n.message(title, &[]),
        ctx.i18n
            .message(signex_i18n::menu_ids::MENU_PLANNED_TOOLTIP, &[]),
        ctx.i18n.message(desc, &[]),
    )
}

/// The planned card's glyph for the panel named `title`: Favorites a star,
/// Snippets a snippet, To-Do a check, any other a clock.
fn planned_icon(title: signex_i18n::MessageId) -> &'static [u8] {
    use signex_i18n::ids;
    match title {
        t if t == ids::PANELS_KIND_FAVORITES => GLYPH_STAR,
        t if t == ids::PANELS_KIND_SNIPPETS => GLYPH_SNIPPET,
        t if t == ids::PANELS_KIND_TODO => GLYPH_CHECK,
        _ => GLYPH_PLANNED,
    }
}

const GLYPH_STAR: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8 2.2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.4l-3.6 1.9.7-4L2.2 6.5l4-.6z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>"#;
const GLYPH_SNIPPET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M5.5 4L2 8l3.5 4M10.5 4L14 8l-3.5 4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const GLYPH_CHECK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="11" rx="2.5" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.2 8.2l1.9 1.9 3.7-4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const GLYPH_PLANNED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="5.8" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.8V8l2.2 1.4" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// Render one indented part row inside the SCH Library tree-expander.
/// Active part gets the selection background; otherwise the row hovers
/// like the symbol rows above it.
pub fn part_tree_row<'a>(
    label: signex_i18n::Localized,
    part: u8,
    is_active: bool,
    primary: Color,
    muted: Color,
    bg_active: Color,
) -> Element<'a, PanelMsg> {
    let label_color = if is_active { primary } else { muted };
    signex_widgets::button(
        row![
            // Tree-expander indent: 18 px gutter + a faint glyph so
            // the part rows visually nest under the symbol.
            // raw: a tree-branch glyph, not language.
            text(raw("\u{2514}"))
                .size(TS.body)
                .color(muted)
                .width(Length::Fixed(18.0)),
            text(label)
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

/// Property-row label: wraps at spaces inside the fixed label column, so a
/// narrow panel grows the row and a wide one widens the field; a single
/// over-long word is clipped, never split mid-word, and never reaches the
/// value column.
pub fn property_label<'a, M: 'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    color: Color,
) -> Element<'a, M> {
    container(
        text(label)
            .size(TS.body)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .width(PROPERTY_LABEL_COLUMN)
    .clip(true)
    .boxed()
}

/// Numeric edit row used by the shape pre-placement form. The typed text
/// stays as a draft while the box is edited, so `-` and `1.` survive; each
/// parsable keystroke is applied, and Enter shows the stored value again.
pub fn form_edit_row_f64<'a>(
    label: signex_i18n::Localized,
    value: f64,
    key: crate::panels::NumericFieldKey,
    draft: Option<&crate::panels::NumericDraft>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::row;
    row![
        property_label(label, muted),
        crate::panels::numeric_input("", key, value, draft).size(TS.body),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(PROPERTY_ROW_GAP)
    .align_y(iced::Alignment::Center)
    .boxed()
}

pub fn shape_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    use iced::widget::row;
    use signex_i18n::{MessageId, ids, t};
    use signex_types::schematic::FillType;
    use signex_widgets::{button, text};
    let tile = |label: MessageId, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        button(text(t!(i18n, label)).size(TS.body))
            .padding([3.0, SP.m])
            .on_press(PanelMsg::SetPrePlacementShapeFill(ft))
            .style(super::form_look::option_tile(active))
            .boxed()
    };
    row![
        text(t!(i18n, ids::PANELS_SHAPE_FILL))
            .size(TS.body)
            .color(muted)
            .width(PROPERTY_LABEL_COLUMN),
        row![
            tile(
                ids::PANELS_SHAPE_FILL_NONE,
                FillType::None,
                current == FillType::None
            ),
            tile(
                ids::PANELS_SHAPE_FILL_OUTLINE,
                FillType::Outline,
                current == FillType::Outline
            ),
            tile(
                ids::PANELS_SHAPE_FILL_BACKGROUND,
                FillType::Background,
                current == FillType::Background
            ),
        ]
        .spacing(SP.s)
        .width(Length::Fill),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(PROPERTY_ROW_GAP)
    .align_y(iced::Alignment::Center)
    .boxed()
}

#[cfg(test)]
mod label_tests {
    use super::*;

    /// #1283: Favorites, Snippets and To-Do have their own glyph on the
    /// planned card; every other planned panel the neutral one.
    #[test]
    fn each_planned_panel_has_its_glyph() {
        use signex_i18n::ids;
        let glyphs = [
            planned_icon(ids::PANELS_KIND_FAVORITES),
            planned_icon(ids::PANELS_KIND_SNIPPETS),
            planned_icon(ids::PANELS_KIND_TODO),
            planned_icon(ids::PANELS_KIND_WIKI),
        ];
        for (i, a) in glyphs.iter().enumerate() {
            for b in &glyphs[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(planned_icon(ids::PANELS_KIND_BOM_STUDIO), GLYPH_PLANNED);
    }

    /// #2347: every property label takes the fixed column, so a wide dock
    /// gives its extra width to the field, not the label.
    #[test]
    fn property_labels_take_the_fixed_column() {
        for window_w in [400.0, 1600.0] {
            let label: Element<'_, ()> = property_label(
                signex_i18n::t!(
                    signex_i18n::I18n::en_us(),
                    signex_i18n::ids::PROPERTIES_DESIGNATOR
                ),
                Color::BLACK,
            );
            let size = crate::layout_probe::root_size(label, iced::Size::new(window_w, 100.0));
            assert_eq!(
                size.width, PROPERTY_LABEL_WIDTH,
                "in a {window_w} px window"
            );
        }
    }
}
