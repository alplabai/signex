//! The Width and Color rows of a wire, shared by the Tab form of the wire
//! tool and the Properties of a selected wire. Width is a 2 x 2 grid of the
//! four presets, so no name is cut off; Color is a field that opens a small
//! popover with the preset colours, Custom… and Reset to Default (#2523).

use std::rc::Rc;

use iced::widget::{Column, Row, Space, container};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget};
use signex_types::schematic::WireWidthPreset;

use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_widgets::theme_ext::Radii;
use signex_widgets::{button, text};

use super::PanelMsg;
use super::element_properties::rows::labelled_row;
use super::form_look::{self, FormLook};
use crate::choice::{Choice, chosen};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the colour popover (184 px at a 12 px body).
const POPOVER_WIDTH_EM: f32 = 184.0 / 12.0;
/// Side of a preset swatch in the popover (24 px at 12 px).
const SWATCH_EM: f32 = 24.0 / 12.0;
/// Side of the swatch inside the Color field (16 px at 12 px).
const FIELD_SWATCH_EM: f32 = 16.0 / 12.0;
/// Height of a popover menu row (24 px at 12 px).
const MENU_ROW_EM: f32 = 24.0 / 12.0;
/// Swatches per popover row.
const SWATCHES_PER_ROW: usize = 6;
/// Under the field. `BottomStart` of the dropdown widget opens to the
/// field's left, away from it.
const POPOVER_ALIGNMENT: iced_aw::drop_down::Alignment = iced_aw::drop_down::Alignment::Bottom;

/// Whether the wire colour picker is closed, shows the preset palette or the
/// full HSV / RGB picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireColorPicker {
    #[default]
    Closed,
    Palette,
    Advanced,
}

/// The wire colour as the colour field carries it.
pub fn color_to_rgba(color: signex_types::theme::Color) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

/// The colour a colour field pick means.
pub fn rgba_to_color(rgba: [u8; 4]) -> signex_types::theme::Color {
    signex_types::theme::Color::new(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// The catalog name of a wire width preset.
pub(in crate::panels) fn preset_label(preset: WireWidthPreset) -> MessageId {
    match preset {
        WireWidthPreset::Smallest => ids::PROPERTIES_WIRE_WIDTH_SMALLEST,
        WireWidthPreset::Small => ids::PROPERTIES_WIRE_WIDTH_SMALL,
        WireWidthPreset::Medium => ids::PROPERTIES_WIRE_WIDTH_MEDIUM,
        WireWidthPreset::Large => ids::PROPERTIES_WIRE_WIDTH_LARGE,
    }
}

/// The presets in the 2 x 2 grid, row by row.
const WIDTH_GRID: [[WireWidthPreset; 2]; 2] = [
    [WireWidthPreset::Smallest, WireWidthPreset::Small],
    [WireWidthPreset::Medium, WireWidthPreset::Large],
];

/// One cell of the width grid: raised when chosen, flat otherwise.
fn width_cell<'a>(
    choice: &Choice<WireWidthPreset>,
    is_chosen: bool,
    on_pick: &impl Fn(WireWidthPreset) -> PanelMsg,
) -> Element<'a, PanelMsg> {
    // The catalog name of the preset, rendered when the view was built.
    let label = text(UserText::new(choice.label.clone()))
        .size(TS.caption)
        .align_x(iced::alignment::Horizontal::Center)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End);
    button(label)
        .padding([SP.xs, SP.s])
        .on_press(on_pick(choice.value))
        .style(move |theme: &Theme, status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            form_look::segment_cell(theme, is_chosen, hovered && !is_chosen)
        })
        .boxed()
}

/// The Width row: the four presets as a 2 x 2 segmented grid. `on_pick`
/// builds the message for the chosen preset.
pub fn wire_width_row<'a>(
    current: WireWidthPreset,
    on_pick: impl Fn(WireWidthPreset) -> PanelMsg + 'a,
    _muted: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let choices = Choice::named(&WireWidthPreset::ALL, preset_label, i18n);
    let selected = chosen(&choices, &current).map_or(current, |c| c.value);
    let mut cells: Vec<Element<'a, PanelMsg>> = Vec::with_capacity(WireWidthPreset::ALL.len());
    for preset in WIDTH_GRID.into_iter().flatten() {
        if let Some(choice) = chosen(&choices, &preset) {
            cells.push(width_cell(&choice, preset == selected, &on_pick));
        }
    }
    let lines = signex_widgets::FitRow::new(cells)
        .spacing(SP.xs)
        .fill(true)
        .max_columns(2);
    let grid = container(lines)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed();
    labelled_row(t!(i18n, ids::PROPERTIES_WIRE_WIDTH), grid)
}

fn iced_color(rgba: [u8; 4]) -> Color {
    Color::from_rgba8(rgba[0], rgba[1], rgba[2], f32::from(rgba[3]) / 255.0)
}

/// The small swatch in the Color field: the colour, or the field fill with
/// a strong hairline for the theme default.
fn field_swatch<'a>(current: Option<[u8; 4]>) -> Element<'a, PanelMsg> {
    let side = TS.body * FIELD_SWATCH_EM;
    container(Space::new())
        .width(side)
        .height(side)
        .style(move |theme: &Theme| {
            let l = FormLook::of(theme);
            container::Style {
                background: Some(Background::Color(current.map_or(l.field, iced_color))),
                border: Border {
                    width: 1.0,
                    color: l.line_strong,
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        })
        .boxed()
}

/// The Color field: swatch, the hex code or "Default", a chevron. A press
/// opens or closes the popover.
fn color_field_face<'a>(
    current: Option<[u8; 4]>,
    is_open: bool,
    on_toggle: PanelMsg,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let name: signex_widgets::Label<'a> = match current {
        // A hex colour code is design data, not language.
        Some(c) => UserText::new(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])).into(),
        None => t!(i18n, ids::PROPERTIES_COLOR_DEFAULT).into(),
    };
    let face = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .push(field_swatch(current))
        .push(
            text(name)
                .size(TS.body)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .boxed(),
        )
        .push(
            signex_widgets::chevron::chevron(true, TS.body, Color::TRANSPARENT)
                .style(|theme: &Theme, _| iced::widget::svg::Style {
                    color: Some(FormLook::of(theme).ink4),
                    ..Default::default()
                })
                .boxed(),
        );
    button(face)
        .width(Length::Fill)
        .padding([SP.s, SP.s + SP.xs])
        .on_press(on_toggle)
        .style(move |theme: &Theme, status| {
            let mut style = form_look::value_field_button(theme, status);
            if is_open {
                style.border.color = FormLook::of(theme).accent;
            }
            style
        })
        .boxed()
}

/// One row of the popover menu (Custom…, Reset to Default); `None` draws it
/// in the faint ink with no action.
fn menu_row<'a>(
    label: signex_i18n::Localized,
    on_press: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let is_live = on_press.is_some();
    let mut row = button(
        text(label)
            .size(TS.body)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(TS.body * MENU_ROW_EM)
    .padding([0.0, SP.xs])
    .style(move |theme: &Theme, status| {
        let l = FormLook::of(theme);
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: (hovered && is_live).then_some(Background::Color(l.sunken)),
            text_color: if is_live { l.ink } else { l.ink4 },
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        }
    });
    if let Some(msg) = on_press {
        row = row.on_press(msg);
    }
    row.boxed()
}

/// The popover under a colour field: Preset Colours, Custom…, Reset to
/// Default. Reset is live only while the object has a colour of its own.
fn color_popover<'a>(
    current: Option<[u8; 4]>,
    on_pick: &Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static>,
    on_advanced: PanelMsg,
    on_clear: Option<PanelMsg>,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let side = TS.body * SWATCH_EM;
    let mut swatches = Column::<Element<'a, PanelMsg>>::new().spacing(SP.xs);
    for chunk in super::color_field::PRESETS.chunks(SWATCHES_PER_ROW) {
        let mut row = Row::<Element<'a, PanelMsg>>::new().spacing(SP.xs);
        for (_name, rgb) in chunk {
            let rgba = [rgb[0], rgb[1], rgb[2], 255];
            let is_current = current == Some(rgba);
            let swatch = button(Space::new())
                .width(Length::Fill)
                .height(side)
                .padding(0)
                .on_press(on_pick(rgba))
                .style(move |theme: &Theme, status| {
                    let l = FormLook::of(theme);
                    let hovered = matches!(status, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: Some(Background::Color(iced_color(rgba))),
                        border: Border {
                            width: if is_current || hovered { 2.0 } else { 1.0 },
                            color: if is_current { l.accent } else { l.line_strong },
                            radius: Radii::GRAPHITE.small.into(),
                            ..Border::default()
                        },
                        ..iced::widget::button::Style::default()
                    }
                });
            row = row.push(swatch.boxed());
        }
        swatches = swatches.push(row.boxed());
    }
    let rule = container(Space::new())
        .height(signex_widgets::theme_ext::HAIRLINE)
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(FormLook::of(theme).line)),
            ..container::Style::default()
        });
    let body = Column::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .width(Length::Fill)
        .push(
            text(t!(i18n, ids::PROPERTIES_COLOR_PRESETS))
                .size(TS.caption)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(FormLook::of(theme).ink3),
                    ..Default::default()
                })
                .boxed(),
        )
        .push(swatches.boxed())
        .push(rule.boxed())
        .push(menu_row(
            t!(i18n, ids::PROPERTIES_COLOR_CUSTOM),
            Some(on_advanced),
        ))
        .push(menu_row(t!(i18n, ids::PROPERTIES_COLOR_RESET), on_clear));
    container(body)
        .padding(SP.m)
        .width(TS.body * POPOVER_WIDTH_EM)
        .style(|theme: &Theme| {
            let l = FormLook::of(theme);
            container::Style {
                background: Some(Background::Color(l.raised)),
                border: Border {
                    width: 1.0,
                    color: l.line,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        })
        .boxed()
}

/// The Color row. `on_set` builds the message for a pick (`Some`) or for the
/// reset to the theme's wire colour (`None`). The popover and the HSV / RGB
/// picker use the wire colour picker messages; a click outside closes them.
pub fn wire_color_row<'a>(
    current: Option<signex_types::theme::Color>,
    picker: WireColorPicker,
    on_set: Rc<dyn Fn(Option<signex_types::theme::Color>) -> PanelMsg + 'static>,
    _muted: Color,
    _border_c: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> = {
        let on_set = Rc::clone(&on_set);
        Rc::new(move |rgba| on_set(Some(rgba_to_color(rgba))))
    };
    popover_color_row(
        PopoverColorRow {
            label: t!(i18n, ids::PROPERTIES_WIRE_COLOR),
            current: current.map(color_to_rgba),
            picker,
            on_toggle: PanelMsg::ToggleWireColorPicker,
            on_advanced: PanelMsg::OpenWireColorAdvancedPicker,
            on_cancel: PanelMsg::CancelWireColorPicker,
            on_pick,
            on_clear: current.is_some().then(|| on_set(None)),
        },
        i18n,
    )
}

/// What a colour field with the preset popover shows and sends.
pub struct PopoverColorRow {
    pub label: signex_i18n::Localized,
    pub current: Option<[u8; 4]>,
    pub picker: WireColorPicker,
    /// A press on the field: open or close the popover.
    pub on_toggle: PanelMsg,
    /// Custom…: open the HSV / RGB picker.
    pub on_advanced: PanelMsg,
    /// A press outside: close whatever is open.
    pub on_cancel: PanelMsg,
    pub on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static>,
    /// Reset to Default; `None` greys it.
    pub on_clear: Option<PanelMsg>,
}

/// A labelled colour field whose popover opens right under it (mockup rows
/// 43 and 46).
pub fn popover_color_row<'a>(row: PopoverColorRow, i18n: &I18n) -> Element<'a, PanelMsg> {
    let PopoverColorRow {
        label,
        current,
        picker,
        on_toggle,
        on_advanced,
        on_cancel,
        on_pick,
        on_clear,
    } = row;
    let face = color_field_face(current, picker != WireColorPicker::Closed, on_toggle, i18n);
    let control: Element<'a, PanelMsg> = match picker {
        WireColorPicker::Advanced => iced_aw::ColorPicker::new(
            true,
            current.map_or(Color::WHITE, iced_color),
            face,
            on_cancel,
            move |c: Color| {
                let [r, g, b, a] = c.into_rgba8();
                on_pick([r, g, b, a])
            },
        )
        .boxed(),
        WireColorPicker::Closed | WireColorPicker::Palette => iced_aw::DropDown::new(
            face,
            color_popover(current, &on_pick, on_advanced, on_clear, i18n),
            picker == WireColorPicker::Palette,
        )
        .alignment(POPOVER_ALIGNMENT)
        .offset(SP.xs)
        .on_dismiss(on_cancel)
        .boxed(),
    };
    labelled_row(label, control)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each preset reads as `WireWidthPreset::label`, the English the list
    /// showed before, and a choice is found by its value.
    #[test]
    fn each_width_preset_keeps_its_english_name() {
        let en = I18n::en_us();
        let choices = Choice::named(&WireWidthPreset::ALL, preset_label, &en);
        for preset in WireWidthPreset::ALL {
            assert_eq!(en.message_plain(preset_label(preset), &[]), preset.label());
            assert_eq!(
                chosen(&choices, &preset).map(|c| c.label).as_deref(),
                Some(preset.label())
            );
        }
    }

    /// The grid holds every preset once.
    #[test]
    fn the_width_grid_holds_every_preset_once() {
        let mut laid_out: Vec<_> = WIDTH_GRID.into_iter().flatten().collect();
        laid_out.sort_by_key(|p| WireWidthPreset::ALL.iter().position(|q| q == p));
        assert_eq!(laid_out, WireWidthPreset::ALL);
    }

    /// A press on a cell of the width grid sends that preset.
    #[test]
    fn a_width_cell_sends_its_preset() {
        let en = I18n::en_us();
        for preset in WireWidthPreset::ALL {
            let row = wire_width_row(
                WireWidthPreset::Small,
                PanelMsg::SetPrePlacementWireWidth,
                Color::BLACK,
                &en,
            );
            let pressed = crate::layout_probe::messages_on_click(
                row,
                iced::Size::new(320.0, 80.0),
                preset.label(),
            );
            assert!(
                matches!(pressed.as_slice(), [PanelMsg::SetPrePlacementWireWidth(p)] if *p == preset),
                "{preset:?}: {pressed:?}"
            );
        }
    }

    fn color_row(
        picker: WireColorPicker,
        current: Option<signex_types::theme::Color>,
    ) -> Vec<String> {
        let en = I18n::en_us();
        let row = wire_color_row(
            current,
            picker,
            Rc::new(PanelMsg::SetPrePlacementWireColor),
            Color::BLACK,
            Color::BLACK,
            &en,
        );
        crate::layout_probe::lay_out_texts(row, iced::Size::new(320.0, 400.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    /// Closed, the row shows its label and "Default"; the popover is not
    /// part of the panel.
    #[test]
    fn the_closed_colour_field_shows_the_default() {
        let shown = color_row(WireColorPicker::Closed, None);
        assert!(shown.iter().any(|t| t == "Color"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Default"), "{shown:?}");
        assert!(!shown.iter().any(|t| t == "Preset Colours"), "{shown:?}");
    }

    /// Mockup row 43: the popover opens under its field, not beside it.
    #[test]
    fn the_colour_popover_opens_under_its_field() {
        assert!(matches!(
            POPOVER_ALIGNMENT,
            iced_aw::drop_down::Alignment::Bottom
        ));
    }

    /// A press on a shared colour field sends the message of its owner.
    #[test]
    fn a_colour_field_sends_its_owners_toggle() {
        let en = I18n::en_us();
        let row = popover_color_row(
            PopoverColorRow {
                label: t!(en, ids::PROPERTIES_SHEET_BORDER),
                current: None,
                picker: WireColorPicker::Closed,
                on_toggle: PanelMsg::CancelChildSheetColorPicker,
                on_advanced: PanelMsg::Noop,
                on_cancel: PanelMsg::Noop,
                on_pick: Rc::new(|_| PanelMsg::Noop),
                on_clear: None,
            },
            &en,
        );
        let pressed =
            crate::layout_probe::messages_on_click(row, iced::Size::new(320.0, 80.0), "Default");
        assert!(
            matches!(pressed.as_slice(), [PanelMsg::CancelChildSheetColorPicker]),
            "{pressed:?}"
        );
    }

    /// A wire with its own colour shows the hex code.
    #[test]
    fn a_set_colour_shows_its_hex_code() {
        let red = signex_types::theme::Color::new(0xC0, 0x39, 0x2B, 255);
        let shown = color_row(WireColorPicker::Closed, Some(red));
        assert!(shown.iter().any(|t| t == "#C0392B"), "{shown:?}");
    }
}
