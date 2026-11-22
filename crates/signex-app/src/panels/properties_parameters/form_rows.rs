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

/// `field` with its whole `value` in a tooltip, so a value the field cuts
/// off can still be read (#2468). An empty value gets no tooltip.
pub fn with_value_tip<'a, M: 'a>(field: Element<'a, M>, value: &str) -> Element<'a, M> {
    if value.is_empty() {
        return field;
    }
    signex_widgets::tooltip(
        field,
        iced::widget::container(
            container(
                text(signex_i18n::UserText::new(value.to_string()))
                    .size(TS.body)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            )
            .padding([SP.xs, SP.s]),
        )
        .padding([SP.xs, SP.s])
        .style(form_look::value_tip),
        iced::widget::tooltip::Position::Bottom,
    )
    .boxed()
}

/// Form row: label | a read-only value on the sunken fill. A value too long
/// for the field ends in an ellipsis and shows whole in a tooltip (#2468).
pub fn form_input_row<'a, M: 'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    label_c: Color,
    input_bg: Color,
) -> Element<'a, M> {
    value_row(label, value, label_c, input_bg, true)
}

/// [`form_input_row`] without the value tooltip, for a row that carries a
/// tooltip of its own.
pub fn form_input_row_untipped<'a, M: 'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    label_c: Color,
    input_bg: Color,
) -> Element<'a, M> {
    value_row(label, value, label_c, input_bg, false)
}

fn value_row<'a, M: 'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    label_c: Color,
    input_bg: Color,
    tip: bool,
) -> Element<'a, M> {
    let field = container(
        container(
            // raw: the value as the caller gives it; each caller marks
            // what it is (design data, a reading, catalog text or an
            // engine word with its own issue).
            text(raw(value.to_string()))
                .size(TS.body)
                .color(text_on(input_bg))
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .clip(true),
    )
    .padding([3, 6])
    .width(Length::Fill)
    .style(form_look::read_only_value)
    .boxed();
    let field = if tip {
        with_value_tip(field, value)
    } else {
        field
    };
    container(
        row![
            property_label(label, label_c),
            container(field).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A typed custom page edge in mm. `None` is refused: not a finite number
/// (#828), or outside the edges a custom page can have (#2792).
pub fn typed_mm(text: &str) -> Option<f64> {
    use signex_types::schematic::{MAX_PAGE_EDGE_MM, MIN_CUSTOM_EDGE_MM};
    crate::numeric_field::typed_in_range(text, MIN_CUSTOM_EDGE_MM..=MAX_PAGE_EDGE_MM)
}

/// Which custom paper edge a draft belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperEdge {
    Width,
    Height,
}

/// A half-typed custom paper edge, tied to the sheet it was typed on. It is
/// committed as one undoable edit on Enter, so partial input such as the `4`
/// of `400` is never applied mid-typing.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperEdgeDraft {
    pub sheet: std::path::PathBuf,
    pub edge: PaperEdge,
    pub text: String,
}

/// Form row: label | pick_list (dropdown).
pub fn form_pick_row<'a, T>(
    label: impl Into<signex_widgets::Label<'a>>,
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
            property_label(label, label_c),
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

/// Form row: label | pick_list of typed choices, each named from the
/// catalog; picking one reports its value, never its text.
pub(crate) fn form_choice_row<'a, T>(
    label: signex_i18n::Localized,
    choices: Vec<crate::choice::Choice<T>>,
    selected: crate::choice::Choice<T>,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: Clone + PartialEq + 'static,
{
    container(
        row![
            property_label(label, label_c),
            iced::widget::pick_list(Some(selected), choices, |c: &crate::choice::Choice<T>| c
                .label
                .clone())
            .graphite()
            .on_select(move |c: crate::choice::Choice<T>| on_change(c.value))
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
    label: impl Into<signex_widgets::Label<'a>>,
    checked: bool,
    msg: PanelMsg,
    label_c: Color,
    _surface: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            property_label(label, label_c),
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

/// Form row: checkbox with a keyboard shortcut hint on the right.
pub fn form_check_row_shortcut<'a>(
    label: signex_i18n::Localized,
    value: bool,
    on_toggle: PanelMsg,
    shortcut: &'a str,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    let shortcut_owned = shortcut.to_string();
    container(
        row![
            property_label(label, label_c),
            row![
                signex_widgets::checkbox(value)
                    .on_toggle(move |_| on_toggle.clone())
                    .size(TS.label)
                    .spacing(SP.s)
                    .style(form_look::checkbox_style),
                Space::new().width(Length::Fill),
                // raw: a key chord, as the caller spells it.
                text(raw(shortcut_owned))
                    .size(TS.caption)
                    .color(label_c)
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
    label: signex_i18n::Localized,
    current_family: &str,
    current_size_px: f32,
    _bold: bool,
    _italic: bool,
    label_c: Color,
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
/// The whole value shows in a tooltip, for one longer than the box (#2468).
pub fn form_edit_row<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    form_edit_row_refusable(label, value, label_c, None, on_input)
}

/// [`form_edit_row`] whose box is outlined in `refused` while it holds text
/// the field refused.
pub fn form_edit_row_refusable<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    value: &str,
    label_c: Color,
    refused: Option<Color>,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let input =
        signex_widgets::text_input(signex_i18n::UserText::new(String::new()), value.to_string())
            .on_input(on_input)
            .size(TS.body)
            .padding(SP.s)
            .style(form_look::refusable_input_style(refused))
            .width(Length::Fill)
            .boxed();
    container(
        row![
            property_label(label, label_c),
            container(with_value_tip(input, value))
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
            // raw: a font-style glyph (B, I, U, T), drawn in its own style.
            text(raw(glyph.to_string()))
                .size(TS.label)
                .color(primary)
                .font(iced::Font {
                    weight: style,
                    ..signex_widgets::ui_font::regular()
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

    /// #828 — a typed `nan` must never reach the canvas paper size.
    #[test]
    fn typed_mm_keeps_the_current_value_on_refused_or_half_typed_text() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "", "-", "1,5"] {
            assert_eq!(typed_mm(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn typed_mm_clamps_a_finite_value() {
        assert_eq!(typed_mm(" 210 "), Some(210.0));
        assert_eq!(typed_mm("0"), None);
        assert_eq!(typed_mm("9000"), None);
        // Finite as f64 but infinite as f32: still outside the range.
        assert_eq!(typed_mm("1e300"), None);
    }

    #[test]
    fn typed_mm_takes_both_ends_of_the_custom_range() {
        assert_eq!(typed_mm("10"), Some(10.0));
        assert_eq!(typed_mm("5080"), Some(5080.0));
        assert_eq!(typed_mm("9.99"), None);
        assert_eq!(typed_mm("5080.1"), None);
    }
}
