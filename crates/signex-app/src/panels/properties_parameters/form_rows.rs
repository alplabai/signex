//! Generic Properties-panel form-field row builders — label/value rows,
//! integer/mm/pick/check editors, grid rows, the canvas-font popup, and
//! the B/I/U/T font-style row — shared by every Properties surface.
//! Moved verbatim from the former single-file `properties_parameters`
//! module.

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::column;
use signex_widgets::dropdown_style::GraphiteDropdown as _;

use crate::panels::form_look;
use signex_i18n::{ids, t};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Value-text colour for an input drawn on `bg`: white whenever it
/// reaches 4.5:1 against `bg`, otherwise whichever of black / white
/// contrasts more. These row builders receive resolved colours rather
/// than `ThemeTokens`, so the input background decides.
fn text_on(bg: Color) -> Color {
    crate::styles::tokens::readable_on(bg)
}

/// Colour of the On/Off word beside a checkbox drawn on `surface`: the
/// readable text colour for "On", the muted label colour for "Off".
#[cfg(test)]
pub(crate) fn check_state_color(checked: bool, label_c: Color, surface: Color) -> Color {
    if checked {
        text_on(surface)
    } else {
        crate::styles::tokens::keep_readable_on(label_c, surface)
    }
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
        .boxed()
}

/// Section header: bold label + separator line.
pub fn section_hdr<'a, M: 'a>(title: &str, text_c: Color, border_c: Color) -> Element<'a, M> {
    column![
        container(text(raw(title.to_string())).size(TS.label).color(text_c))
            .padding([6.0, SP.m])
            .width(Length::Fill),
        thin_sep(border_c),
    ]
    .spacing(0)
    .boxed()
}

/// Form row: label | styled input-like value display.
pub fn form_input_row<'a, M: 'a>(
    label: &str,
    value: &str,
    label_c: impl Into<LabelInk>,
    input_bg: Color,
) -> Element<'a, M> {
    container(
        row![
            property_label(label.to_string(), label_c),
            container(
                container(
                    text(raw(value.to_string()))
                        .size(TS.body)
                        .color(text_on(input_bg))
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
            )
            .padding([3, 6])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(move |theme: &Theme| {
                let (bg, bdr, radius) = form_look::value_box(theme);
                container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        width: 1.0,
                        radius: radius.into(),
                        color: bdr,
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            }),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
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
    label_c: impl Into<LabelInk>,
    _input_bg: Color,
    _input_border: Color,
) -> Element<'a, PanelMsg> {
    let text_value = draft.map_or_else(|| format!("{value:.1}"), str::to_string);
    container(
        row![
            property_label(label.to_string(), label_c),
            signex_widgets::text_input(signex_widgets::raw(""), text_value)
                .on_input(move |s| PanelMsg::CustomPaperEdgeTyping(edge, s))
                .on_submit(PanelMsg::CommitCustomPaperEdge(edge))
                .size(TS.body)
                .padding([3, 6])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
                .style(form_look::text_input_style),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Form row: label | pick_list (dropdown).
pub fn form_pick_row<'a, T>(
    label: &str,
    options: Vec<T>,
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: impl Into<LabelInk>,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            property_label(label.to_string(), label_c),
            iced::widget::pick_list(Some(selected), options, |t: &T| t.to_string())
                .graphite()
                .on_select(on_change)
                .padding([SP.xs, 6.0])
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Checkbox form row: label, then the checkbox.
pub fn form_check_row<'a>(
    label: &str,
    checked: bool,
    msg: PanelMsg,
    label_c: impl Into<LabelInk>,
    _surface: Color,
) -> Element<'a, PanelMsg> {
    let ink: LabelInk = label_c.into();
    container(
        row![
            property_label(label.to_string(), ink),
            container(
                signex_widgets::checkbox(checked)
                    .on_toggle(move |_| msg.clone())
                    .size(TS.heading)
                    .spacing(SP.s)
                    .style(form_look::checkbox_style),
            )
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
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
    label_c: impl Into<LabelInk>,
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

    let pick = iced::widget::pick_list(selected, labels, |l: &&'static str| (*l).to_string())
        .graphite()
        .on_select(move |lbl: &'static str| {
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
        .width(Length::Fill);

    let label_widget = property_label(label.to_string(), label_c);

    let content: Element<PanelMsg> = if has_checkbox {
        // Snap Grid row: checkbox before pick_list
        row![
            label_widget,
            signex_widgets::checkbox(active)
                .on_toggle(move |_| on_toggle.clone())
                .size(TS.label)
                .spacing(SP.s)
                .style(form_look::checkbox_style),
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    } else {
        // Visible Grid row: no checkbox
        row![
            label_widget,
            container(pick).width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    };

    container(content)
        .padding([2, PROPERTY_ROW_PAD_X])
        .width(Length::Fill)
        .boxed()
}

/// Form row: checkbox with a keyboard shortcut hint on the right.
pub fn form_check_row_shortcut<'a>(
    label: &'a str,
    value: bool,
    on_toggle: PanelMsg,
    shortcut: &'a str,
    label_c: impl Into<LabelInk>,
) -> Element<'a, PanelMsg> {
    let label_c: LabelInk = label_c.into();
    let shortcut_owned = shortcut.to_string();
    container(
        row![
            property_label(label.to_string(), label_c),
            row![
                signex_widgets::checkbox(value)
                    .on_toggle(move |_| on_toggle.clone())
                    .size(TS.label)
                    .spacing(SP.s)
                    .style(form_look::checkbox_style),
                Space::new().width(Length::Fill),
                text(raw(shortcut_owned))
                    .size(TS.caption)
                    .color(label_c.color)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(SP.s)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .clip(true)
    .boxed()
}

pub fn form_font_link_row<'a>(
    label: &'static str,
    current_family: &str,
    current_size_px: f32,
    _bold: bool,
    _italic: bool,
    label_c: impl Into<LabelInk>,
) -> Element<'a, PanelMsg> {
    let summary = format!("{current_family}, {:.0}px", current_size_px);

    container(
        row![
            property_label(label, label_c),
            signex_widgets::button(
                text(signex_i18n::UserText::new(summary))
                    .size(TS.body)
                    .width(Length::Fill),
            )
            .on_press(PanelMsg::OpenCanvasFontPopup)
            .padding([SP.xs, 6.0])
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .style(form_look::value_field_button),
        ]
        .spacing(SP.s)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

pub fn canvas_font_popup<'a>(
    current_family: &str,
    current_size_px: f32,
    bold: bool,
    italic: bool,
    label_c: Color,
    palette: PanelPalette,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let PanelPalette {
        input_bg,
        input_bdr,
        ..
    } = palette;
    let families = crate::fonts::system_font_families();
    let family_pick = iced::widget::pick_list(
        Some(current_family.to_string()),
        families.as_slice(),
        |f: &String| f.clone(),
    )
    .graphite()
    .on_select(PanelMsg::SetCanvasFont)
    .width(Length::Fill);

    let size_input = NumberInput::new(&current_size_px, 6.0..=36.0, PanelMsg::SetCanvasFontSize)
        .step(1.0)
        .width(Length::Fill)
        .padding(SP.s);

    container(
        column![
            row![
                text(t!(i18n, ids::PANELS_CANVAS_FONT_TITLE))
                    .size(TS.body)
                    .color(label_c),
                Space::new().width(Length::Fill),
                signex_widgets::button(text(t!(i18n, ids::PANELS_CANVAS_FONT_CLOSE)).size(TS.body))
                    .on_press(PanelMsg::CloseCanvasFontPopup)
                    .padding([SP.xs, 6.0])
            ]
            .align_y(iced::Alignment::Center),
            row![
                text(t!(i18n, ids::PANELS_CANVAS_FONT_FAMILY))
                    .size(TS.body)
                    .color(label_c)
                    .width(56),
                family_pick,
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
            row![
                text(t!(i18n, ids::PANELS_CANVAS_FONT_SIZE))
                    .size(TS.body)
                    .color(label_c)
                    .width(56),
                size_input,
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
            row![
                text(t!(i18n, ids::PANELS_CANVAS_FONT_STYLE))
                    .size(TS.body)
                    .color(label_c)
                    .width(56),
                row![
                    signex_widgets::checkbox(bold)
                        .on_toggle(PanelMsg::SetCanvasFontBold)
                        .size(TS.label)
                        .spacing(SP.s),
                    text(t!(i18n, ids::PANELS_CANVAS_FONT_BOLD))
                        .size(TS.body)
                        .color(label_c),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center),
                row![
                    signex_widgets::checkbox(italic)
                        .on_toggle(PanelMsg::SetCanvasFontItalic)
                        .size(TS.label)
                        .spacing(SP.s),
                    text(t!(i18n, ids::PANELS_CANVAS_FONT_ITALIC))
                        .size(TS.body)
                        .color(label_c),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(SP.l)
            .align_y(iced::Alignment::Center),
            text(t!(i18n, ids::PANELS_CANVAS_FONT_HINT))
                .size(TS.caption)
                .color(label_c),
        ]
        .spacing(6),
    )
    .padding([6.0, SP.m])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(Background::Color(input_bg)),
        border: Border {
            color: input_bdr,
            width: 1.0,
            radius: 4.0.into(),
            ..Border::default()
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .boxed()
}

/// Form row: label | editable text_input that emits a PanelMsg on input.
pub fn form_edit_row<'a>(
    label: &str,
    value: &str,
    label_c: impl Into<LabelInk>,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label.to_string(), label_c),
            signex_widgets::text_input(signex_widgets::raw(""), value.to_string())
                .on_input(on_input)
                .size(TS.body)
                .padding(SP.s)
                .style(form_look::text_input_style)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Subsection label: muted caption with room above, no divider.
pub fn form_label<'a, M: 'a>(label: signex_i18n::Localized) -> Element<'a, M> {
    container(
        text(label)
            .size(TS.caption)
            .style(move |theme: &Theme| iced::widget::text::Style {
                color: Some(form_look::FormLook::of(theme).ink3),
                ..Default::default()
            }),
    )
    .padding(iced::Padding {
        top: 14.0,
        right: SP.m,
        bottom: 4.0,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// Altium-style B/I/U/T (Bold / Italic / Underline / Strikethrough) row.
pub fn font_style_row<'a>(
    _label_c: Color,
    primary: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let btn = |glyph: &'static str, style: iced::font::Weight| -> Element<'static, PanelMsg> {
        signex_widgets::button(
            text(raw(glyph.to_string()))
                .size(TS.label)
                .color(primary)
                .font(iced::Font {
                    weight: style,
                    ..iced::Font::DEFAULT
                })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([SP.s, 6.0])
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
                    ..Border::default()
                },
                text_color: primary,
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
    };
    container(
        row![
            btn("B", iced::font::Weight::Bold),
            btn("I", iced::font::Weight::Normal),
            btn("U", iced::font::Weight::Normal),
            btn("T", iced::font::Weight::Normal),
        ]
        .spacing(SP.xs)
        .width(Length::Fill),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_on_each_builtin_selection_is_the_expected_readable_colour() {
        use crate::styles::tokens::contrast_ratio;
        use signex_types::theme::{ThemeId, theme_tokens};
        let expected = |id: ThemeId| match id {
            // GraphiteLight selection #EFECE6 is near-white; GraphiteDark #2A2C32 is near-black.
            ThemeId::GraphiteLight => Color::BLACK,
            ThemeId::GraphiteDark => Color::WHITE,
        };
        for &id in ThemeId::BUILTINS {
            let bg = crate::styles::ti(theme_tokens(id).selection);
            let got = text_on(bg);
            assert_eq!(got, expected(id), "{id:?}");
            assert!(contrast_ratio(got, bg) >= 3.0, "{id:?}");
        }
    }

    /// #1786 — the On word is drawn on the panel surface, so it must be
    /// readable there even when the iced background is the opposite lightness.
    #[test]
    fn on_label_is_readable_on_a_panel_bg_unlike_the_iced_background() {
        use crate::styles::tokens::{MIN_TEXT_CONTRAST, contrast_ratio};
        let iced_background = Color::from_rgb(0.05, 0.05, 0.08);
        for panel_bg in [
            Color::from_rgb(0.96, 0.95, 0.90),
            Color::from_rgb(0.12, 0.13, 0.16),
        ] {
            let on = check_state_color(true, Color::from_rgb(0.5, 0.5, 0.5), panel_bg);
            assert!(
                contrast_ratio(on, panel_bg) >= MIN_TEXT_CONTRAST,
                "{panel_bg:?}"
            );
        }
        // The old choice (white, tuned to the dark iced background) fails here.
        let light_panel = Color::from_rgb(0.96, 0.95, 0.90);
        assert_ne!(
            check_state_color(true, Color::BLACK, light_panel),
            crate::styles::tokens::readable_on(iced_background)
        );
        // Off keeps the muted label colour.
        let muted = Color::from_rgb(0.5, 0.5, 0.5);
        assert_eq!(check_state_color(false, muted, light_panel), muted);
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
