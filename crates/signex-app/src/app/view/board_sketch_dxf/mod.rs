//! The Import DXF card of a board sketch (#2794, mockup row 179c), at the
//! canvas's bottom right over a preview of the drawing: file, units, scale
//! and a target per DXF layer, then the profile check, then what landed. It
//! takes the canvas's presses, so the sketch cannot change under it.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, column, container, opaque, pick_list, row, scrollable, stack};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::pcb::dxf::DxfUnits;
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::sketch::{SketchHost, SketchRole};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{text, text_input, theme_ext};

use crate::app::state::{
    DXF_LAYER_TARGETS, DxfLayerTarget, SketchDxfDialog, SketchDxfMsg, SketchDxfProblem,
    SketchDxfStep,
};
use crate::app::{DxfImportMsg, Message, PcbMsg};
use crate::library::editor::footprint::parameters_table::view::field_style;
use crate::library::editor::footprint::sketch_mode::roles::role_label;
use crate::panels::form_look;
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

mod check;
mod preview;
#[cfg(test)]
mod tests;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The card's width, its label column and the tallest its layer list grows,
/// in body text heights.
const CARD_EM: f32 = 24.0;
const LABEL_EM: f32 = 8.0;
const LAYERS_MAX_EM: f32 = 14.0;

static UNIT_CHOICES: [DxfUnits; 4] = DxfUnits::ALL;

fn send(event: SketchDxfMsg) -> Message {
    Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::SketchDialog(event)))
}

/// The dialog over the sketch canvas, with the drawing previewed beside it.
pub(super) fn overlay<'a>(
    dialog: &'a SketchDxfDialog,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let card = match dialog.step {
        SketchDxfStep::Mapping => mapping_card(dialog, tokens, i18n),
        SketchDxfStep::Checking => check::check_card(dialog, tokens, i18n),
        SketchDxfStep::Imported(result) => check::result_card(result, tokens, i18n),
    };
    let card_width = TS.body * CARD_EM;
    let card = container(card)
        .padding(SP.l)
        .width(Length::Fixed(card_width))
        .style(crate::styles::modal_card(tokens));
    let placed = container(card)
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Bottom);
    let preview = preview::preview(dialog, card_width + 2.0 * SP.xl, i18n);
    opaque(stack![preview, placed]).capture_scroll(true).boxed()
}

/// Units, scale and a target per layer, then Next: check profiles.
fn mapping_card<'a>(
    dialog: &'a SketchDxfDialog,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    column![
        heading(
            t!(i18n, ids::BOARD_SKETCH_DXF_TITLE),
            theme_ext::text_primary(tokens)
        ),
        field(
            t!(i18n, ids::BOARD_SKETCH_DXF_FILE),
            text(UserText::new(dialog.file.as_str()))
                .size(TS.body)
                .color(theme_ext::text_primary(tokens))
                .boxed(),
            tokens,
        ),
        field(
            t!(i18n, ids::BOARD_SKETCH_DXF_UNITS),
            units_picker(dialog, i18n),
            tokens,
        ),
        field(
            t!(i18n, ids::BOARD_SKETCH_DXF_SCALE),
            scale_input(dialog, tokens, i18n),
            tokens,
        ),
        layer_rows(dialog, tokens, i18n),
        join_row(dialog, tokens, i18n),
        problem_line(dialog, tokens, i18n),
        buttons(dialog, tokens, i18n),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The card's heading.
fn heading<'a>(label: Localized, color: iced::Color) -> Element<'a, Message> {
    text(label)
        .size(TS.title)
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        })
        .color(color)
        .boxed()
}

fn field<'a>(
    label: Localized,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        text(label)
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens))
            .width(Length::Fixed(TS.body * LABEL_EM)),
        container(control).width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

fn units_label(units: DxfUnits, i18n: &I18n) -> String {
    let id = match units {
        DxfUnits::FromFile => ids::BOARD_SKETCH_DXF_UNITS_FILE,
        DxfUnits::Millimetre => ids::BOARD_SKETCH_DXF_UNITS_MM,
        DxfUnits::Mil => ids::BOARD_SKETCH_DXF_UNITS_MIL,
        DxfUnits::Inch => ids::BOARD_SKETCH_DXF_UNITS_INCH,
    };
    t!(i18n, id).into_string()
}

fn units_picker<'a>(dialog: &SketchDxfDialog, i18n: &I18n) -> Element<'a, Message> {
    let labels: Vec<(DxfUnits, String)> = UNIT_CHOICES
        .iter()
        .map(|&units| (units, units_label(units, i18n)))
        .collect();
    let label_of = move |units: &DxfUnits| {
        labels
            .iter()
            .find(|(listed, _)| listed == units)
            .map(|(_, label)| label.clone())
            .unwrap_or_default()
    };
    pick_list(Some(dialog.units), UNIT_CHOICES.as_slice(), label_of)
        .graphite()
        .on_select(|units| send(SketchDxfMsg::UnitsPicked(units)))
        .padding(SP.xs)
        .width(Length::Fill)
        .boxed()
}

fn scale_input<'a>(
    dialog: &SketchDxfDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let bad = dialog.check == Err(SketchDxfProblem::Scale);
    text_input(
        t!(i18n, ids::BOARD_SKETCH_DXF_SCALE_HINT),
        dialog.scale.clone(),
    )
    .size(TS.dense)
    .padding(SP.xs)
    .style(field_style(tokens, bad))
    .on_input(|scale| send(SketchDxfMsg::ScaleEdited(scale)))
    .boxed()
}

fn target_label(target: DxfLayerTarget, i18n: &I18n) -> String {
    match target {
        None => t!(i18n, ids::BOARD_SKETCH_DXF_SKIP).into_string(),
        Some(SketchRole::Unassigned) => t!(i18n, ids::BOARD_SKETCH_DXF_SKETCH_ONLY).into_string(),
        Some(role) => role_label(role, SketchHost::Board, i18n),
    }
}

fn layer_rows<'a>(
    dialog: &'a SketchDxfDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let mut list: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    for (index, layer) in dialog.rows.iter().enumerate() {
        let labels: Vec<(DxfLayerTarget, String)> = DXF_LAYER_TARGETS
            .iter()
            .map(|&target| (target, target_label(target, i18n)))
            .collect();
        let label_of = move |target: &DxfLayerTarget| {
            labels
                .iter()
                .find(|(listed, _)| listed == target)
                .map(|(_, label)| label.clone())
                .unwrap_or_default()
        };
        let picked = dialog.targets.get(index).copied().flatten();
        let picker = pick_list(Some(picked), DXF_LAYER_TARGETS.as_slice(), label_of)
            .graphite()
            .on_select(move |target| send(SketchDxfMsg::TargetPicked { row: index, target }))
            .padding(SP.xs)
            .width(Length::Fill);
        list = list.push(
            row![
                text(UserText::new(layer.name.as_str()))
                    .size(TS.caption)
                    .color(theme_ext::text_secondary(tokens))
                    .width(Length::Fixed(TS.body * LABEL_EM)),
                picker,
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    scrollable(list)
        .height(Length::Fit.max(TS.body * LAYERS_MAX_EM))
        .boxed()
}

fn join_row<'a>(
    dialog: &SketchDxfDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    row![
        signex_widgets::checkbox(dialog.join)
            .on_toggle(|join| send(SketchDxfMsg::JoinToggled(join)))
            .size(TS.body)
            .spacing(0)
            .style(form_look::checkbox_style),
        text(t!(i18n, ids::BOARD_SKETCH_DXF_JOIN))
            .size(TS.caption)
            .color(theme_ext::text_primary(tokens)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// Why the choices cannot import; nothing when they can, since the check
/// step names what the drawing holds.
fn problem_line<'a>(
    dialog: &SketchDxfDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let Err(problem) = dialog.check else {
        return iced::widget::Space::new().boxed();
    };
    let id = match problem {
        SketchDxfProblem::Scale => ids::BOARD_SKETCH_DXF_BAD_SCALE,
        SketchDxfProblem::Range => ids::BOARD_SKETCH_DXF_OUT_OF_RANGE,
        SketchDxfProblem::NothingPicked => ids::BOARD_SKETCH_DXF_NOTHING,
    };
    text(t!(i18n, id))
        .size(TS.caption)
        .color(theme_ext::error_color(tokens))
        .boxed()
}

fn buttons<'a>(
    dialog: &SketchDxfDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let next = dialog
        .check
        .is_ok()
        .then(|| send(SketchDxfMsg::CheckRequested));
    row![
        dialog_button(
            t!(i18n, ids::BOARD_SKETCH_DXF_NEXT),
            DialogButtonKind::Primary,
            next,
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::BOARD_SKETCH_DXF_CANCEL),
            DialogButtonKind::Secondary,
            Some(send(SketchDxfMsg::Cancelled)),
            tokens,
        ),
    ]
    .spacing(SP.s)
    .boxed()
}
