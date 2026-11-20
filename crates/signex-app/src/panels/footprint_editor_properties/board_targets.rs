//! The Board section rows of the targets of decision 2 (Caner, 2026-11-20),
//! for the selected entity (mockup row 179, "Bake as"): a Slot's size, read
//! only; a Pour's copper layer; a Mechanical layer drawing's layer; a
//! Component anchor's component. A pour's net, fill and priority stay in
//! the Pour sub-form of the Role section.

use iced::widget::{Column, container, pick_list, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::MechanicalLayer;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, text};

use super::super::rows::value_row;
use crate::panels::{
    BoardSketchAttrMsg, BoardTargetProps, ComponentChoice, LayerChoice, PROPERTY_ROW_PAD_X,
    PanelMsg, PanelPalette,
};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The label and value parts of a row, as `rows::pick_row` splits them.
const LABEL_PORTION: u16 = 4;
const VALUE_PORTION: u16 = 5;

/// The rows of `target` for entity `id`; nothing for an entity without one
/// of these roles.
pub(super) fn view_board_targets<'a>(
    col: Column<Element<'a, PanelMsg>>,
    id: SketchEntityId,
    target: &'a BoardTargetProps,
    palette: PanelPalette,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    use BoardSketchAttrMsg as M;
    match target {
        BoardTargetProps::None => col,
        BoardTargetProps::Slot { size } => {
            let value = match size {
                Some((length, width)) => t!(
                    i18n, ids::BOARD_SKETCH_SLOT_SIZE,
                    "length" => format!("{length:.2}"), "width" => format!("{width:.2}")
                ),
                None => t!(i18n, ids::BOARD_SKETCH_SLOT_NOT_YET),
            };
            col.push(value_row(
                &t!(i18n, ids::BOARD_SKETCH_SLOT).into_string(),
                &value.into_string(),
                palette.muted,
                palette.input_bg,
            ))
        }
        BoardTargetProps::Pour { layer, layers } => {
            let selected = layers.iter().find(|choice| choice.layer == *layer).cloned();
            let picker = pick_list(selected, layers.as_slice(), LayerChoice::to_string)
                .graphite()
                .on_select(move |choice: LayerChoice| {
                    PanelMsg::BoardSketch(M::PourLayerPicked {
                        id,
                        layer: choice.layer,
                    })
                })
                .padding(SP.s)
                .width(Length::FillPortion(VALUE_PORTION))
                .boxed();
            col.push(picker_row(
                t!(i18n, ids::BOARD_SKETCH_POUR_LAYER),
                picker,
                palette,
            ))
        }
        BoardTargetProps::Mechanical { layer, layers } => {
            let labels: Vec<(MechanicalLayer, String)> = layers
                .iter()
                .map(|&choice| (choice, mechanical_label(choice, i18n)))
                .collect();
            let label_of = move |choice: &MechanicalLayer| {
                labels
                    .iter()
                    .find(|(listed, _)| listed == choice)
                    .map(|(_, label)| label.clone())
                    .unwrap_or_default()
            };
            let picker = pick_list(Some(*layer), layers.as_slice(), label_of)
                .graphite()
                .on_select(move |layer| {
                    PanelMsg::BoardSketch(M::MechanicalLayerPicked { id, layer })
                })
                .padding(SP.s)
                .width(Length::FillPortion(VALUE_PORTION))
                .boxed();
            col.push(picker_row(
                t!(i18n, ids::BOARD_SKETCH_MECHANICAL_LAYER),
                picker,
                palette,
            ))
        }
        BoardTargetProps::Anchor { picked, components } => {
            let selected = picked.and_then(|footprint| {
                components
                    .iter()
                    .find(|choice| choice.footprint == footprint)
                    .cloned()
            });
            let picker = pick_list(selected, components.as_slice(), ComponentChoice::to_string)
                .graphite()
                .placeholder_label(t!(i18n, ids::BOARD_SKETCH_ANCHOR_PICK))
                .on_select(move |choice: ComponentChoice| {
                    PanelMsg::BoardSketch(M::AnchorComponentPicked {
                        id,
                        footprint: choice.footprint,
                    })
                })
                .padding(SP.s)
                .width(Length::FillPortion(VALUE_PORTION))
                .boxed();
            col.push(picker_row(
                t!(i18n, ids::BOARD_SKETCH_ANCHOR_COMPONENT),
                picker,
                palette,
            ))
        }
    }
}

/// "Mechanical 3", in the UI language.
fn mechanical_label(layer: MechanicalLayer, i18n: &I18n) -> String {
    t!(i18n, ids::BOARD_SKETCH_MECHANICAL_N, "n" => layer.index().to_string()).into_string()
}

/// A label and a picker, split as the panel's other pick rows are.
fn picker_row<'a>(
    label: Localized,
    picker: Element<'a, PanelMsg>,
    palette: PanelPalette,
) -> Element<'a, PanelMsg> {
    container(
        row![
            container(text(label).size(TS.body).color(palette.muted))
                .width(Length::FillPortion(LABEL_PORTION))
                .clip(true),
            picker,
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}
