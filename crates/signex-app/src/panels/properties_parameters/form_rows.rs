//! Generic Properties-panel form-field row builders — label/value rows,
//! integer/mm/pick/check editors, grid rows, the canvas-font popup, and
//! the B/I/U/T font-style row — shared by every Properties surface.
//! Moved verbatim from the former single-file `properties_parameters`
//! module.

use super::super::*;
use iced::widget::column;

/// Value-text colour for an input drawn on `bg`: white whenever it
/// reaches 4.5:1 against `bg`, otherwise whichever of black / white
/// contrasts more. These row builders receive resolved colours rather
/// than `ThemeTokens`, so the input background decides.
fn text_on(bg: Color) -> Color {
    crate::styles::tokens::readable_on(bg)
}

/// Thin 1px separator line. `pub(super)` so sibling modules
/// (footprint_editor_properties, symbol_editor_properties, etc.)
/// extracted from this file can share the single implementation.
pub fn thin_sep<'a, M: 'a>(border_c: Color) -> Element<'a, M> {
    container(Space::new())
        .height(1.0)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(border_c)),
            ..container::Style::default()
        })
        .into()
}

/// Section header: bold label + separator line.
pub fn section_hdr<'a, M: 'a>(title: &str, text_c: Color, border_c: Color) -> Column<'a, M> {
    column![
        container(text(title.to_string()).size(12).color(text_c))
            .padding([6, 8])
            .width(Length::Fill),
        container(Space::new())
            .height(1.0)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            }),
    ]
    .spacing(0)
}

/// Form row: label | styled input-like value display.
pub fn form_input_row<'a, M: 'a>(
    label: &str,
    value: &str,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, M> {
    container(
        row![
            property_label(label.to_string(), label_c),
            container(
                container(
                    text(value.to_string())
                        .size(11)
                        .color(text_on(input_bg))
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
            )
            .padding([3, 6])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(input_bg)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_border,
                },
                ..container::Style::default()
            }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// A typed mm value clamped to 1..=MAX_PAGE_EDGE_MM. Refused or half-typed
/// text keeps `current`: `clamp` passes NaN through, so a typed `nan` would
/// otherwise reach the canvas paper size (#828). The sheet-size model then
/// raises it to the smallest custom edge.
pub fn typed_mm(text: &str, current: f32) -> f32 {
    crate::numeric_field::parse_field_f64(text)
        .map_or(current, |v| v as f32)
        .clamp(1.0, signex_types::schematic::MAX_PAGE_EDGE_MM as f32)
}

/// Which custom paper edge a draft belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperEdge {
    Width,
    Height,
}

/// A half-typed custom paper edge, tied to the sheet it was typed on. It is
/// committed as one undoable edit on Enter, so partial input such as the `4`
/// of `400` is never clamped to the minimum edge mid-typing.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperEdgeDraft {
    pub sheet: std::path::PathBuf,
    pub edge: PaperEdge,
    pub text: String,
}

/// Form row: label | floating-point mm text_input (no spinner buttons).
/// Shows `draft` while one is being typed, else the model `value`; typing
/// only reports the text, and Enter commits it.
pub fn form_mm_edit_row<'a>(
    label: &str,
    value: f32,
    draft: Option<&str>,
    edge: PaperEdge,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, PanelMsg> {
    let text_value = draft.map_or_else(|| format!("{value:.1}"), str::to_string);
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::text_input("", &text_value)
                .on_input(move |s| PanelMsg::CustomPaperEdgeTyping(edge, s))
                .on_submit(PanelMsg::CommitCustomPaperEdge(edge))
                .size(11)
                .padding([3, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: Background::Color(input_bg),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: input_border,
                    },
                    icon: Color::TRANSPARENT,
                    placeholder: Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0),
                    value: text_on(input_bg),
                    selection: Color::from_rgba8(0x4D, 0x52, 0x66, 0.6),
                }),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list (dropdown).
pub fn form_pick_row<'a, T>(
    label: &str,
    options: Vec<T>,
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::pick_list(options, Some(selected), on_change)
                .text_size(11)
                .padding([2, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | custom widget.
#[expect(
    dead_code,
    reason = "form-row builder kept for panels still being migrated to the shared rows"
)]
pub fn form_label_row<'a>(
    label: &str,
    control: Row<'a, PanelMsg>,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            container(control).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Checkbox form row.
pub fn form_check_row<'a>(
    label: &str,
    checked: bool,
    msg: PanelMsg,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            row![
                iced::widget::checkbox(checked)
                    .on_toggle(move |_| msg.clone())
                    .size(14)
                    .spacing(4),
                text(if checked { "On" } else { "Off" })
                    .size(11)
                    .color(if checked { Color::WHITE } else { label_c }),
            ]
            .spacing(8)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | pick_list for grid size presets (2.54 mm multiples).
#[expect(
    dead_code,
    reason = "grid-size preset row kept for panels still being migrated to the shared rows"
)]
pub fn form_grid_size_row(current_mm: f32, label_c: Color) -> Element<'static, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZES_MM};
    // Find the label that matches the current value (fallback to first).
    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(GRID_SIZE_LABELS.iter())
        .find(|(sz, _)| (**sz - current_mm).abs() < 1e-4)
        .map(|(_, lbl)| *lbl);
    container(
        row![
            container(
                text("Visible Grid".to_string())
                    .size(11)
                    .color(label_c)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(LABEL_W)
            .clip(true),
            iced::widget::pick_list(GRID_SIZE_LABELS, selected, |lbl: &'static str| {
                // Map label back to mm value
                let mm = GRID_SIZES_MM
                    .iter()
                    .zip(GRID_SIZE_LABELS.iter())
                    .find(|(_, l)| **l == lbl)
                    .map(|(v, _)| *v)
                    .unwrap_or(2.54);
                PanelMsg::SetGridSize(mm)
            },)
            .text_size(11)
            .width(Length::Fill),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// Altium-style grid row: [Label] [checkbox toggle] [pick_list] [shortcut hint]
/// Used for both "Visible Grid" (eye/visible toggle) and "Snap Grid" (snap enable toggle).
/// Labels and values are shown in the current `unit` (mm or mil).
#[expect(
    clippy::too_many_arguments,
    reason = "a generic form-row builder: label, value, bounds, styling and the message constructor are all independent"
)]
pub fn form_grid_row(
    label: &'static str,
    current_mm: f32,
    unit: Unit,
    has_checkbox: bool,
    on_size: impl Fn(f32) -> PanelMsg + 'static,
    label_c: Color,
    active: bool,
    on_toggle: PanelMsg,
) -> Element<'static, PanelMsg> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};

    let labels: &'static [&'static str] = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };

    let selected: Option<&'static str> = GRID_SIZES_MM
        .iter()
        .zip(labels.iter())
        .find(|(sz, _)| (**sz - current_mm).abs() < 1e-4)
        .map(|(_, lbl)| *lbl);

    let pick = iced::widget::pick_list(labels, selected, move |lbl: &'static str| {
        // Map label back to mm value (labels and GRID_SIZES_MM are parallel arrays)
        let mm = GRID_SIZE_LABELS
            .iter()
            .chain(GRID_SIZE_LABELS_MIL.iter())
            .zip(GRID_SIZES_MM.iter().chain(GRID_SIZES_MM.iter()))
            .find(|(l, _)| **l == lbl)
            .map(|(_, v)| *v)
            .unwrap_or(2.54);
        on_size(mm)
    })
    .text_size(11)
    .width(Length::Fill);

    let label_widget = text(label.to_string())
        .size(11)
        .color(label_c)
        .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
        .wrapping(iced::widget::text::Wrapping::None);

    let content: Element<PanelMsg> = if has_checkbox {
        // Snap Grid row: checkbox before pick_list
        row![
            label_widget,
            iced::widget::checkbox(active)
                .on_toggle(move |_| on_toggle.clone())
                .size(12)
                .spacing(4),
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        // Visible Grid row: no checkbox
        row![
            label_widget,
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    };

    container(content)
        .padding([2, PROPERTY_ROW_PAD_X])
        .width(Length::Fill)
        .into()
}

/// Form row: checkbox with a keyboard shortcut hint on the right.
pub fn form_check_row_shortcut<'a>(
    label: &'a str,
    value: bool,
    on_toggle: PanelMsg,
    shortcut: &'a str,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let shortcut_owned = shortcut.to_string();
    container(
        row![
            text(label.to_string())
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::Word),
            row![
                iced::widget::checkbox(value)
                    .on_toggle(move |_| on_toggle.clone())
                    .size(12)
                    .spacing(4),
                Space::new().width(Length::Fill),
                text(shortcut_owned)
                    .size(9)
                    .color(label_c)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(4)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .clip(true)
    .into()
}

pub fn form_font_link_row<'a>(
    label: &'static str,
    current_family: &str,
    current_size_px: f32,
    _bold: bool,
    _italic: bool,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let summary = format!("{current_family}, {:.0}px", current_size_px);

    container(
        row![
            text(label)
                .size(11)
                .color(label_c)
                .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
                .wrapping(iced::widget::text::Wrapping::None),
            iced::widget::button(
                text(summary)
                    .size(11)
                    .color(Color::from_rgb(0.35, 0.7, 1.0))
                    .width(Length::Fill),
            )
            .on_press(PanelMsg::OpenCanvasFontPopup)
            .padding([1, 0])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let underline = matches!(status, iced::widget::button::Status::Hovered);
                iced::widget::button::Style {
                    background: None,
                    text_color: if underline {
                        Color::from_rgb(0.55, 0.82, 1.0)
                    } else {
                        Color::from_rgb(0.35, 0.7, 1.0)
                    },
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    ..Default::default()
                }
            }),
        ]
        .spacing(4)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

pub fn canvas_font_popup<'a>(
    current_family: &str,
    current_size_px: f32,
    bold: bool,
    italic: bool,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let families = crate::fonts::system_font_families();
    let family_pick = iced::widget::pick_list(
        families.as_slice(),
        Some(current_family.to_string()),
        PanelMsg::SetCanvasFont,
    )
    .text_size(11)
    .width(Length::Fill);

    let size_input = NumberInput::new(&current_size_px, 6.0..=36.0, PanelMsg::SetCanvasFontSize)
        .step(1.0)
        .width(Length::Fill)
        .padding(4);

    container(
        column![
            row![
                text("Canvas Font Settings").size(11).color(label_c),
                Space::new().width(Length::Fill),
                iced::widget::button(text("Close").size(10))
                    .on_press(PanelMsg::CloseCanvasFontPopup)
                    .padding([2, 6])
            ]
            .align_y(iced::Alignment::Center),
            row![
                text("Family").size(10).color(label_c).width(56),
                family_pick,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            row![text("Size").size(10).color(label_c).width(56), size_input,]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            row![
                text("Style").size(10).color(label_c).width(56),
                row![
                    iced::widget::checkbox(bold)
                        .on_toggle(PanelMsg::SetCanvasFontBold)
                        .size(12)
                        .spacing(4),
                    text("Bold").size(10).color(label_c),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
                row![
                    iced::widget::checkbox(italic)
                        .on_toggle(PanelMsg::SetCanvasFontItalic)
                        .size(12)
                        .spacing(4),
                    text("Italic").size(10).color(label_c),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            text("Applies immediately to canvas text rendering.")
                .size(9)
                .color(label_c),
        ]
        .spacing(6),
    )
    .padding([6, 8])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(Background::Color(input_bg)),
        border: Border {
            color: input_bdr,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

/// Form row: label | NumberInput (iced_aw) with step/bounds.
#[expect(
    dead_code,
    reason = "NumberInput row kept for panels still being migrated to the shared rows"
)]
fn form_number_row<'a, T>(
    label: &str,
    value: T,
    bounds: impl std::ops::RangeBounds<T> + 'a,
    step: T,
    on_change: impl Fn(T) -> PanelMsg + 'static + Clone,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + num_traits::Bounded
        + 'static,
{
    container(
        row![
            property_label(label.to_string(), label_c),
            NumberInput::new(&value, bounds, on_change)
                .step(step)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .padding(4),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Form row: label | editable text_input that emits a PanelMsg on input.
pub fn form_edit_row<'a>(
    label: &str,
    value: &str,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::text_input("", value)
                .on_input(on_input)
                .size(11)
                .padding(4)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(8.0)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// Standalone label row (no value, used before segmented controls).
pub fn form_label<'a, M: 'a>(label: &str, label_c: Color) -> Element<'a, M> {
    container(text(label.to_string()).size(11).color(label_c))
        .padding([4, 8])
        .width(Length::Fill)
        .into()
}

/// Altium-style B/I/U/T (Bold / Italic / Underline / Strikethrough) row.
pub fn font_style_row<'a>(
    _label_c: Color,
    primary: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let btn = |glyph: &'static str, style: iced::font::Weight| -> Element<'static, PanelMsg> {
        iced::widget::button(
            text(glyph.to_string())
                .size(12)
                .color(primary)
                .font(iced::Font {
                    weight: style,
                    ..iced::Font::DEFAULT
                })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([4, 6])
        .on_press(PanelMsg::Noop)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if hovered {
                    input_bdr
                } else {
                    input_bg
                })),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: input_bdr,
                },
                text_color: primary,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    container(
        row![
            btn("B", iced::font::Weight::Bold),
            btn("I", iced::font::Weight::Normal),
            btn("U", iced::font::Weight::Normal),
            btn("T", iced::font::Weight::Normal),
        ]
        .spacing(2.0)
        .width(Length::Fill),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_on_each_builtin_selection_is_the_expected_readable_colour() {
        use crate::styles::tokens::contrast_ratio;
        use signex_types::theme::{ThemeId, theme_tokens};
        let expected = |id: ThemeId| match id {
            // GitHubDark #388BFD and SolarizedLight #268BD2: white < 4.5:1.
            ThemeId::GitHubDark | ThemeId::SolarizedLight => Color::BLACK,
            ThemeId::Signex
            | ThemeId::Alplab
            | ThemeId::VsCodeDark
            | ThemeId::CatppuccinMocha
            | ThemeId::Nord => Color::WHITE,
            ThemeId::Custom => unreachable!("BUILTINS excludes Custom"),
        };
        for &id in ThemeId::BUILTINS {
            let bg = crate::styles::ti(theme_tokens(id).selection);
            let got = text_on(bg);
            assert_eq!(got, expected(id), "{id:?}");
            assert!(contrast_ratio(got, bg) >= 3.0, "{id:?}");
        }
    }

    #[test]
    fn text_on_follows_background_lightness() {
        assert_eq!(text_on(Color::from_rgb(0.12, 0.13, 0.16)), Color::WHITE);
        assert_eq!(text_on(Color::from_rgb(0.99, 0.96, 0.89)), Color::BLACK);
    }

    /// #828 — `f32::clamp(NaN)` is NaN, so a typed `nan` must be refused
    /// before the clamp, not by it.
    #[test]
    fn typed_mm_keeps_the_current_value_on_refused_or_half_typed_text() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "", "-", "1,5"] {
            assert_eq!(typed_mm(bad, 297.0), 297.0, "{bad:?}");
        }
    }

    #[test]
    fn typed_mm_clamps_a_finite_value() {
        assert_eq!(typed_mm(" 210 ", 297.0), 210.0);
        assert_eq!(typed_mm("0", 297.0), 1.0);
        assert_eq!(typed_mm("9000", 297.0), 5080.0);
        // Finite as f64 but infinite as f32: the clamp still bounds it.
        assert_eq!(typed_mm("1e300", 297.0), 5080.0);
    }
}
