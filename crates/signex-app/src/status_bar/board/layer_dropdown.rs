//! The layer dropdown of the status bar's Layer pill, under the Dropdown
//! layer switcher: the visible layers by group, each with its swatch, the
//! active one checked. A row makes that layer active and closes the list.

use iced::widget::{Column, Widget, container, scrollable};
use iced::{Element, Length};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::graphite::{SP, TS};
use super::counters::layer_swatch;
use super::kit::{PX, caption, card, card_width, check, list_row};
use super::{BoardPopoverEvent, BoardPopoverKind, LayerChoice};
use crate::styles::ti;

/// Lines (headings and rows) the list shows before it scrolls.
const LIST_LINES: usize = 12;

/// A heading of the dropdown, in the order they are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerGroup {
    Copper,
    Silkscreen,
    Mask,
    Paste,
    Assembly,
    Mechanical,
    Documentation,
}

impl LayerGroup {
    const ALL: [Self; 7] = [
        Self::Copper,
        Self::Silkscreen,
        Self::Mask,
        Self::Paste,
        Self::Assembly,
        Self::Mechanical,
        Self::Documentation,
    ];

    /// The group `layer` is listed under.
    pub(crate) fn of(layer: PcbLayer) -> Self {
        use TechnicalLayer as T;
        match layer {
            PcbLayer::Copper(_) => Self::Copper,
            PcbLayer::Mechanical(_) => Self::Mechanical,
            PcbLayer::Technical(technical) => match technical {
                T::SilkscreenTop | T::SilkscreenBottom => Self::Silkscreen,
                T::SolderMaskTop | T::SolderMaskBottom => Self::Mask,
                T::PasteTop | T::PasteBottom => Self::Paste,
                T::AdhesiveTop
                | T::AdhesiveBottom
                | T::FabTop
                | T::FabBottom
                | T::AssemblyTop
                | T::AssemblyBottom
                | T::CourtyardTop
                | T::CourtyardBottom
                | T::Body3dTop
                | T::Body3dBottom
                | T::FiducialTop
                | T::FiducialBottom
                | T::DesignatorTop
                | T::DesignatorBottom => Self::Assembly,
                T::EdgeCuts
                | T::Margin
                | T::KeepoutTop
                | T::KeepoutBottom
                | T::Dimensions
                | T::DrillDrawing => Self::Mechanical,
                T::Notes | T::Eco1 | T::Eco2 => Self::Documentation,
            },
        }
    }

    fn title(self) -> MessageId {
        match self {
            Self::Copper => ids::BOARD_STATUS_LAYER_GROUP_COPPER,
            Self::Silkscreen => ids::BOARD_STATUS_LAYER_GROUP_SILKSCREEN,
            Self::Mask => ids::BOARD_STATUS_LAYER_GROUP_MASK,
            Self::Paste => ids::BOARD_STATUS_LAYER_GROUP_PASTE,
            Self::Assembly => ids::BOARD_STATUS_LAYER_GROUP_ASSEMBLY,
            Self::Mechanical => ids::BOARD_STATUS_LAYER_GROUP_MECHANICAL,
            Self::Documentation => ids::BOARD_STATUS_LAYER_GROUP_DOCUMENTATION,
        }
    }
}

/// One heading of the dropdown and its layers.
#[derive(Debug, Clone)]
pub(crate) struct LayerDropdownGroup {
    pub group: LayerGroup,
    pub rows: Vec<LayerChoice>,
}

/// `rows` under their headings, each keeping its order; an empty group is
/// left out.
pub(crate) fn group_layers(rows: Vec<LayerChoice>) -> Vec<LayerDropdownGroup> {
    LayerGroup::ALL
        .into_iter()
        .filter_map(|group| {
            let rows: Vec<LayerChoice> = rows
                .iter()
                .filter(|row| LayerGroup::of(row.layer) == group)
                .cloned()
                .collect();
            (!rows.is_empty()).then_some(LayerDropdownGroup { group, rows })
        })
        .collect()
}

pub(super) fn view<'a>(
    groups: &[LayerDropdownGroup],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let mut lines: Vec<Element<'a, BoardPopoverEvent>> = Vec::new();
    for group in groups {
        lines.push(
            container(caption(t!(i18n, group.group.title()), tokens))
                .padding([SP.xs, 6.0 * PX])
                .boxed(),
        );
        lines.extend(group.rows.iter().map(|row| layer_row(row, tokens)));
    }
    let height = if lines.len() > LIST_LINES {
        Length::Fixed(LIST_LINES as f32 * crate::status_bar::popup_row_height())
    } else {
        Length::Shrink
    };
    let list = scrollable(Column::with_children(lines).width(Length::Fill))
        .height(height)
        .direction(crate::styles::thin_vertical())
        .boxed();
    card(card_width(BoardPopoverKind::Layer), vec![list], tokens)
}

fn layer_row<'a>(row: &LayerChoice, tokens: &ThemeTokens) -> Element<'a, BoardPopoverEvent> {
    let name = text(row.name.clone())
        .size(TS.body)
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None);
    list_row(
        Some(layer_swatch(row.colour, tokens)),
        name.boxed(),
        Some(check(row.active, tokens)),
        Some(BoardPopoverEvent::LayerPicked(row.layer)),
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::CopperLayer;

    fn choice(layer: PcbLayer) -> LayerChoice {
        LayerChoice {
            layer,
            name: t!(I18n::en_us(), ids::BOARD_STATUS_LAYER_GROUP_COPPER),
            colour: iced::Color::WHITE,
            active: false,
        }
    }

    /// Rows land under their headings in the heading order, each group
    /// keeping its rows' order, and an empty group is left out.
    #[test]
    fn the_layers_are_listed_by_group() {
        let top = PcbLayer::Copper(CopperLayer::Top);
        let bottom = PcbLayer::Copper(CopperLayer::Bottom);
        let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
        let mask = PcbLayer::Technical(TechnicalLayer::SolderMaskBottom);
        let paste = PcbLayer::Technical(TechnicalLayer::PasteTop);
        let rows = [paste, top, mask, silk, bottom].map(choice).to_vec();
        let groups: Vec<(LayerGroup, Vec<PcbLayer>)> = group_layers(rows)
            .into_iter()
            .map(|g| (g.group, g.rows.iter().map(|r| r.layer).collect()))
            .collect();
        assert_eq!(
            groups,
            [
                (LayerGroup::Copper, vec![top, bottom]),
                (LayerGroup::Silkscreen, vec![silk]),
                (LayerGroup::Mask, vec![mask]),
                (LayerGroup::Paste, vec![paste]),
            ]
        );
    }
}
