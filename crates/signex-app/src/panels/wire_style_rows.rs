//! The Width and Color rows of a wire, shared by the Tab form of the wire
//! tool and the Properties of a selected wire. Width is a 2 x 2 grid of the
//! four presets, so no name is cut off; Color is a field that opens a small
//! popover with the preset colours, Custom… and Reset to Default (#2523),
//! and takes a typed hex code (mockup row 46, `color_hex`).

use std::rc::Rc;

use iced::widget::{Column, Row, Space, container, text_input};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget};
use signex_types::schematic::WireWidthPreset;

use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_widgets::focused_escape::FocusedEscape;
use signex_widgets::theme_ext::Radii;
use signex_widgets::{button, text};

use super::element_properties::rows::labelled_row;
use super::form_look::{self, FormLook};
use super::graphite::PROPERTY_FIELD_RADIUS;
use super::{HexEntry, PanelMsg};
use crate::choice::{Choice, chosen};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Gap between the swatch, the hex box and the chevron of a Color field.
const FACE_SPACING: f32 = SP.s;

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

/// The hex text box inside the field: no frame of its own, the field draws
/// it.
fn bare_input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        ..form_look::text_input_style(theme, status)
    }
}

/// A swatch or chevron of the field: no fill or frame, a press opens or
/// closes the popover.
fn bare_toggle<'a>(content: Element<'a, PanelMsg>, on_toggle: PanelMsg) -> Element<'a, PanelMsg> {
    button(content)
        .padding(0)
        .on_press(on_toggle)
        .style(|_: &Theme, _| iced::widget::button::Style::default())
        .boxed()
}

/// What the Color field shows and sends besides its hex box.
struct FieldFace<'r> {
    current: Option<[u8; 4]>,
    hex: &'r HexEntry,
    on_pick: &'r Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static>,
    default_name: MessageId,
    is_open: bool,
    on_toggle: PanelMsg,
}

/// The Color field (mockup row 46): the swatch, the hex code in a mono
/// box (the default name while the object has no colour of its own), a
/// chevron. The swatch and the chevron open or close the popover; a code
/// typed in the box applies on Enter or on leaving it, Esc restores.
fn color_field_face<'a>(face: FieldFace<'_>, i18n: &I18n) -> Element<'a, PanelMsg> {
    let FieldFace {
        current,
        hex,
        on_pick,
        default_name,
        is_open,
        on_toggle,
    } = face;
    let left = hex.left(current, on_pick.as_ref());
    let is_refused = hex.is_refused();
    let input = signex_widgets::text_input(t!(i18n, default_name), hex.text(current))
        .font(Font::MONOSPACE)
        .size(TS.body)
        .padding([SP.s, 0.0])
        .width(Length::Fill)
        .on_input(hex.typed())
        .on_submit(left.clone())
        .on_blur(left)
        .style(bare_input_style)
        .boxed();
    let input: Element<'a, PanelMsg> = FocusedEscape::new(input, hex.restored())
        .ahead_of_blur()
        .into();
    let chevron = signex_widgets::chevron::chevron(true, TS.body, Color::TRANSPARENT)
        .style(|theme: &Theme, _| iced::widget::svg::Style {
            color: Some(FormLook::of(theme).ink4),
            ..Default::default()
        })
        .boxed();
    let row = Row::<Element<'a, PanelMsg>>::new()
        .spacing(FACE_SPACING)
        .align_y(Alignment::Center)
        .push(bare_toggle(field_swatch(current), on_toggle.clone()))
        .push(input)
        .push(bare_toggle(chevron, on_toggle));
    container(row)
        .width(Length::Fill)
        .padding([0.0, SP.s + SP.xs])
        .style(move |theme: &Theme| {
            let l = FormLook::of(theme);
            let edge = if is_refused {
                theme.palette().danger.base.color
            } else if is_open {
                l.accent
            } else {
                l.line
            };
            container::Style {
                background: Some(Background::Color(l.field)),
                text_color: Some(l.ink),
                border: Border {
                    width: 1.0,
                    radius: PROPERTY_FIELD_RADIUS.into(),
                    color: edge,
                    ..Border::default()
                },
                ..container::Style::default()
            }
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
    hex: HexEntry,
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
            hex,
        },
        i18n,
    )
}

/// What a colour field with the preset popover shows and sends.
pub struct PopoverColorRow {
    pub label: signex_i18n::Localized,
    pub current: Option<[u8; 4]>,
    pub picker: WireColorPicker,
    /// The hex box: its row and what is typed in it.
    pub hex: HexEntry,
    /// A press on the swatch or the chevron: open or close the popover.
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
    popover_color_row_named(row, ids::PROPERTIES_COLOR_DEFAULT, i18n)
}

/// [`popover_color_row`] whose field reads `default_name` while the object
/// has no colour of its own (a drawing's Auto, mockup row 48).
pub fn popover_color_row_named<'a>(
    row: PopoverColorRow,
    default_name: MessageId,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let PopoverColorRow {
        label,
        current,
        picker,
        on_toggle,
        on_advanced,
        on_cancel,
        on_pick,
        on_clear,
        hex,
    } = row;
    let is_open = picker != WireColorPicker::Closed;
    let face = color_field_face(
        FieldFace {
            current,
            hex: &hex,
            on_pick: &on_pick,
            default_name,
            is_open,
            on_toggle,
        },
        i18n,
    );
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

/// Where the swatch and the chevron of a Color field sit, by the bounds of
/// its laid-out hex box: the centres a test presses.
#[cfg(test)]
pub(in crate::panels) fn toggle_points(hex_box: iced::Rectangle) -> (iced::Point, iced::Point) {
    let swatch = TS.body * FIELD_SWATCH_EM;
    let chevron = signex_widgets::chevron::side(TS.body);
    let y = hex_box.center_y();
    (
        iced::Point::new(hex_box.x - FACE_SPACING - swatch / 2.0, y),
        iced::Point::new(hex_box.x + hex_box.width + FACE_SPACING + chevron / 2.0, y),
    )
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

    use crate::layout_probe::{self, LaidOutText};
    use crate::panels::{ColorHexDraft, ColorRowKey};

    const KEY: ColorRowKey = ColorRowKey::PrePlacementWire;

    fn wire_row<'a>(
        picker: WireColorPicker,
        current: Option<signex_types::theme::Color>,
        draft: Option<&ColorHexDraft>,
        en: &I18n,
    ) -> Element<'a, PanelMsg> {
        wire_color_row(
            current,
            picker,
            Rc::new(PanelMsg::SetPrePlacementWireColor),
            HexEntry::of(KEY, draft),
            Color::BLACK,
            Color::BLACK,
            en,
        )
    }

    /// The texts and the hex boxes of the wire Color row in a `width` dock.
    fn color_row(
        picker: WireColorPicker,
        current: Option<signex_types::theme::Color>,
        width: f32,
    ) -> (Vec<LaidOutText>, Vec<LaidOutText>) {
        let en = I18n::en_us();
        let size = iced::Size::new(width, 400.0);
        let texts = layout_probe::lay_out_texts(wire_row(picker, current, None, &en), size);
        let inputs = layout_probe::lay_out_inputs(wire_row(picker, current, None, &en), size);
        (texts, inputs)
    }

    fn shown(laid_out: &[LaidOutText]) -> Vec<&str> {
        laid_out.iter().map(|t| t.text.as_str()).collect()
    }

    /// Closed, the row shows its label and the hex box reads "Default"; the
    /// popover is not part of the panel.
    #[test]
    fn the_closed_colour_field_shows_the_default() {
        let (texts, inputs) = color_row(WireColorPicker::Closed, None, 320.0);
        assert!(shown(&texts).contains(&"Color"), "{texts:?}");
        assert_eq!(shown(&inputs), ["Default"], "the placeholder of the box");
        assert!(!shown(&texts).contains(&"Preset Colors"), "{texts:?}");
    }

    /// What is typed shows in the box until it is applied.
    #[test]
    fn a_typed_draft_shows_in_the_box() {
        let en = I18n::en_us();
        let draft = ColorHexDraft {
            key: KEY,
            text: "#12".into(),
            refused: true,
        };
        let row = wire_row(WireColorPicker::Closed, None, Some(&draft), &en);
        let inputs = layout_probe::lay_out_inputs(row, iced::Size::new(320.0, 80.0));
        assert_eq!(shown(&inputs), ["#12"]);
    }

    /// Mockup row 46: in a 300 px dock the swatch, the hex box and the
    /// chevron share the field column, and the box is wide enough for a
    /// whole code in the mono face.
    #[test]
    fn the_swatch_box_and_chevron_fit_a_300_px_dock() {
        let dock = 300.0;
        let red = signex_types::theme::Color::new(0xFF, 0xF7, 0xE0, 255);
        let (texts, inputs) = color_row(WireColorPicker::Closed, Some(red), dock);
        let label = layout_probe::find(&texts, "Color").bounds;
        let [field] = inputs.as_slice() else {
            panic!("one hex box: {inputs:?}");
        };
        let swatch = TS.body * FIELD_SWATCH_EM;
        assert!(
            field.bounds.x >= label.x + label.width + swatch,
            "the swatch sits between the label and the box: {field:?}"
        );
        let chevron_end = field.bounds.x
            + field.bounds.width
            + FACE_SPACING
            + signex_widgets::chevron::side(TS.body);
        assert!(chevron_end <= dock, "the chevron ends at {chevron_end}");
        let code = layout_probe::natural::<PanelMsg>(
            text(UserText::new("#FFF7E0".to_string()))
                .font(Font::MONOSPACE)
                .size(TS.body)
                .boxed(),
            "#FFF7E0",
        );
        assert!(field.bounds.width >= code.width, "{field:?} holds {code:?}");
    }

    /// Mockup row 43: the popover opens under its field, not beside it.
    #[test]
    fn the_colour_popover_opens_under_its_field() {
        assert!(matches!(
            POPOVER_ALIGNMENT,
            iced_aw::drop_down::Alignment::Bottom
        ));
    }

    fn sheet_border_row<'a>(en: &I18n) -> Element<'a, PanelMsg> {
        popover_color_row(
            PopoverColorRow {
                label: t!(en, ids::PROPERTIES_SHEET_BORDER),
                current: None,
                picker: WireColorPicker::Closed,
                hex: HexEntry::of(ColorRowKey::ChildSheetBorder(uuid::Uuid::nil()), None),
                on_toggle: PanelMsg::CancelChildSheetColorPicker,
                on_advanced: PanelMsg::Noop,
                on_cancel: PanelMsg::Noop,
                on_pick: Rc::new(|_| PanelMsg::Noop),
                on_clear: None,
            },
            en,
        )
    }

    /// A press on the swatch or the chevron of a shared colour field sends
    /// the message of its owner; a press in the hex box sends nothing.
    #[test]
    fn a_colour_fields_swatch_and_chevron_send_its_owners_toggle() {
        let en = I18n::en_us();
        let size = iced::Size::new(320.0, 80.0);
        let inputs = layout_probe::lay_out_inputs(sheet_border_row(&en), size);
        let field = layout_probe::find(&inputs, "Default").bounds;
        let (swatch, chevron) = toggle_points(field);
        for at in [swatch, chevron] {
            let pressed = layout_probe::messages_on_click_at(sheet_border_row(&en), size, None, at);
            assert!(
                matches!(pressed.as_slice(), [PanelMsg::CancelChildSheetColorPicker]),
                "{at:?}: {pressed:?}"
            );
        }
        let in_box =
            layout_probe::messages_on_click_at(sheet_border_row(&en), size, None, field.center());
        assert!(in_box.is_empty(), "{in_box:?}");
    }

    /// A wire with its own colour shows the hex code in the box.
    #[test]
    fn a_set_colour_shows_its_hex_code() {
        let red = signex_types::theme::Color::new(0xC0, 0x39, 0x2B, 255);
        let (_texts, inputs) = color_row(WireColorPicker::Closed, Some(red), 320.0);
        assert_eq!(shown(&inputs), ["#C0392B"]);
    }
}
