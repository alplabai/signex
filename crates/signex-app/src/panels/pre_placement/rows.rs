//! The Graphite rows of the pre-placement form: the header with its key
//! caption, label | control rows, X / Y coordinate pairs, the read-only
//! Length, the 4-step Rotation, the 2 x 2 wire Width presets and the
//! justification grid. Sizes come from the type and spacing scales, colours
//! from the theme tokens.

use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Space, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::schematic::{HAlign, VAlign, WireWidthPreset};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text, text_input};

use super::super::{PROPERTY_LABEL_COLUMN, PanelMsg, form_look};
use super::position::{
    ANCHOR_COLUMNS, ANCHOR_ROWS, PositionField, PrePlacementPositionMsg, ROTATION_STEPS,
    anchor_name, rotation_step,
};
use crate::styles::tokens::{ResolvedTokens, resolve};
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The tokens a row needs, resolved once per view.
#[derive(Clone, Copy)]
pub(super) struct Look {
    pub r: ResolvedTokens,
    pub ink: Color,
    pub error: Color,
}

impl Look {
    pub fn of(tokens: &ThemeTokens) -> Self {
        Self {
            r: resolve(tokens),
            ink: ti(tokens.text),
            error: ti(tokens.error),
        }
    }
}

/// Header: "Place <tool>" and the primary OK, then the caption naming the
/// keys: Enter is OK, Esc goes back to placing.
pub(super) fn header<'a>(
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
    tool_name: &str,
) -> Element<'a, PanelMsg> {
    let title = text(t!(i18n, ids::PROPERTIES_PRE_PLACEMENT_TITLE, "tool" => tool_name))
        .size(TS.label)
        .color(look.ink)
        .wrapping(Wrapping::None);
    let ok = crate::styles::dialog_button(
        t!(i18n, ids::DIALOG_BUTTON_OK),
        crate::styles::DialogButtonKind::Primary,
        Some(PanelMsg::ConfirmPrePlacement),
        tokens,
    );
    let title_row = container(
        row![title, Space::new().width(Length::Fill), ok]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill);

    let caption = |id: MessageId| -> Element<'a, PanelMsg> {
        text(t!(i18n, id))
            .size(TS.caption)
            .color(look.r.ink3)
            .wrapping(Wrapping::None)
            .boxed()
    };
    let key = |id: MessageId| -> Element<'a, PanelMsg> {
        container(
            text(t!(i18n, id))
                .size(TS.caption)
                .font(mono())
                .wrapping(Wrapping::None),
        )
        .padding([0.0, SP.xs])
        .style(crate::styles::key_cap(tokens))
        .boxed()
    };
    let keys: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .push(caption(ids::PROPERTIES_PRE_PLACEMENT_APPLIES))
        .push(caption(ids::PROPERTIES_PRE_PLACEMENT_SEPARATOR))
        .push(key(ids::PROPERTIES_PRE_PLACEMENT_KEY_ENTER))
        .push(caption(ids::DIALOG_BUTTON_OK))
        .push(caption(ids::PROPERTIES_PRE_PLACEMENT_SEPARATOR))
        .push(key(ids::PROPERTIES_PRE_PLACEMENT_KEY_ESC))
        .push(caption(ids::PROPERTIES_PRE_PLACEMENT_BACK_TO_PLACING));
    let caption_row = container(keys.wrap().vertical_spacing(SP.xs))
        .padding(iced::Padding {
            top: 0.0,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .width(Length::Fill);

    Column::new()
        .width(Length::Fill)
        .push(title_row.boxed())
        .push(caption_row.boxed())
        .push(hairline(look).boxed())
        .boxed()
}

/// A 1 px rule under the header.
fn hairline<'a>(look: Look) -> Element<'a, PanelMsg> {
    let line = look.r.line_soft;
    container(Space::new())
        .height(crate::styles::HAIRLINE)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// Label | control: the Graphite property row.
pub(super) fn field<'a>(
    look: Look,
    label: Localized,
    content: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let label = text(label)
        .size(TS.body)
        .color(look.r.ink3)
        .wrapping(Wrapping::None);
    container(
        row![
            container(label).width(PROPERTY_LABEL_COLUMN).clip(true),
            container(content).width(Length::Fill),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// The mono face of coordinates and keys.
fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The unit a length field shows beside its number: the panel's unit.
fn unit_label(i18n: &I18n, unit: Unit) -> Localized {
    t!(i18n, ids::PROPERTIES_UNIT_SUFFIX, "unit" => unit.to_string())
}

/// One coordinate as the form shows it: its field, its text and whether
/// that text does not read as a length (the field is then marked).
pub(super) struct Coordinate {
    pub field: PositionField,
    pub text: String,
    pub invalid: bool,
}

/// One editable coordinate: the axis letter, the number and its unit.
/// Typing reports the text; Enter is OK. Text that does not read is marked
/// with the error colour.
fn coordinate<'a>(
    look: Look,
    i18n: &I18n,
    unit: Unit,
    axis: Localized,
    shown: Coordinate,
) -> Element<'a, PanelMsg> {
    let Coordinate {
        field,
        text: value,
        invalid,
    } = shown;
    let error = look.error;
    let input = text_input(signex_i18n::UserText::new(String::new()), value)
        .on_input(move |typed| {
            PanelMsg::PrePlacementPosition(PrePlacementPositionMsg::FieldTyped(field, typed))
        })
        .on_submit(PanelMsg::ConfirmPrePlacement)
        .size(TS.body)
        .font(mono())
        .padding([SP.xs, SP.xs])
        .style(move |theme: &Theme, status| {
            let mut style = form_look::text_input_style(theme, status);
            if invalid {
                style.border.color = error;
            }
            style
        })
        .width(Length::Fill);
    row![
        text(axis).size(TS.caption).color(look.r.ink4),
        input,
        text(unit_label(i18n, unit))
            .size(TS.caption)
            .color(look.r.ink4),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .boxed()
}

/// An X / Y pair of editable coordinates in the panel's `unit`.
pub(super) fn coordinate_pair<'a>(
    look: Look,
    i18n: &I18n,
    unit: Unit,
    (x, y): (Coordinate, Coordinate),
) -> Element<'a, PanelMsg> {
    row![
        coordinate(look, i18n, unit, t!(i18n, ids::PROPERTIES_AXIS_X), x),
        coordinate(look, i18n, unit, t!(i18n, ids::PROPERTIES_AXIS_Y), y),
    ]
    .spacing(SP.s)
    .width(Length::Fill)
    .boxed()
}

/// A derived, read-only length: sunken, the panel's unit on the right.
pub(super) fn read_only_length<'a>(
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
    unit: Unit,
    value: impl Into<Label<'a>>,
) -> Element<'a, PanelMsg> {
    container(
        row![
            text(value)
                .size(TS.body)
                .font(mono())
                .color(look.r.ink3)
                .width(Length::Fill),
            text(unit_label(i18n, unit))
                .size(TS.caption)
                .color(look.r.ink4),
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.s])
    .width(Length::Fill)
    .style(crate::styles::read_only_field(tokens))
    .boxed()
}

/// Cells of a segmented control, at most `per_row` on a line, each
/// `(label, active, msg)`.
fn segmented_grid<'a>(
    cells: Vec<(Localized, bool, PanelMsg)>,
    per_row: usize,
) -> Element<'a, PanelMsg> {
    let cells: Vec<Element<'a, PanelMsg>> = cells
        .into_iter()
        .map(|(label, active, msg)| {
            button(
                text(label)
                    .size(TS.body)
                    .wrapping(Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([SP.xs, SP.xs])
            .on_press(msg)
            .style(move |theme: &Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered);
                form_look::segment_cell(theme, active, hovered && !active)
            })
            .boxed()
        })
        .collect();
    let grid = signex_widgets::FitRow::new(cells)
        .spacing(SP.xs)
        .fill(true)
        .max_columns(per_row.max(1));
    container(grid)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}

/// Rotation as four steps (0° / 90° / 180° / 270°); a rotation off the
/// quarter turns marks none.
pub(super) fn rotation<'a>(i18n: &I18n, current: f64) -> Element<'a, PanelMsg> {
    let marked = rotation_step(current);
    let cells = ROTATION_STEPS
        .into_iter()
        .map(|deg| {
            (
                t!(i18n, ids::PROPERTIES_ROTATION_STEP, "degrees" => format!("{deg:.0}")),
                marked == Some(deg),
                PanelMsg::SetPrePlacementRotation(deg),
            )
        })
        .collect();
    segmented_grid(cells, ROTATION_STEPS.len())
}

/// The wire Width presets as a 2 x 2 grid, so no name is cut off in a
/// narrow dock.
pub(super) fn wire_width<'a>(i18n: &I18n, current: WireWidthPreset) -> Element<'a, PanelMsg> {
    let cells = WireWidthPreset::ALL
        .into_iter()
        .map(|preset| {
            (
                t!(i18n, super::super::wire_style_rows::preset_label(preset)),
                preset == current,
                PanelMsg::SetPrePlacementWireWidth(preset),
            )
        })
        .collect();
    segmented_grid(cells, 2)
}

/// The 3 x 3 justification grid and the name of the anchor it marks.
/// Picking a cell sets both the horizontal and the vertical anchor.
pub(super) fn justification<'a>(
    look: Look,
    i18n: &I18n,
    (h, v): (HAlign, VAlign),
) -> Element<'a, PanelMsg> {
    let cells = ANCHOR_ROWS.map(|row_v| {
        ANCHOR_COLUMNS.map(|col_h| {
            let picked = PrePlacementPositionMsg::AnchorPicked(col_h, row_v);
            (
                (col_h, row_v) == (h, v),
                PanelMsg::PrePlacementPosition(picked),
            )
        })
    });
    row![
        form_look::anchor_grid(cells),
        text(t!(i18n, anchor_name(h, v)))
            .size(TS.caption)
            .color(look.r.ink3),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The card under Location of a tool with nothing else to set.
pub(super) fn no_options<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PanelMsg> {
    container(crate::panels::empty_state_card(
        tokens,
        t!(i18n, ids::PROPERTIES_PRE_PLACEMENT_NO_OPTIONS),
        t!(i18n, ids::PROPERTIES_PRE_PLACEMENT_NO_OPTIONS_HINT),
        None,
    ))
    .padding([SP.m, SP.m])
    .width(Length::Fill)
    .boxed()
}
