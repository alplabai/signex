//! The layer colour legend over a footprint's bake preview (mockup row 190):
//! a swatch and name for each layer the preview draws, in the colours the
//! canvas draws them. Empty while the preview is closed.

use iced::alignment::Horizontal;
use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::parameters_table::view::TOP_CLEARANCE_EM;
use crate::library::messages::LibraryMessage;
use crate::pcb_canvas::layer_style::board_layer_colour;
use crate::styles::Radii;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// A swatch's height, as a part of the caption size.
const SWATCH_HEIGHT: f32 = 0.25;

/// The layers of the legend, in the mockup's order.
const LAYERS: [(MessageId, PcbLayer); 5] = [
    (
        ids::SKETCH_BAKE_LEGEND_TOP,
        PcbLayer::Copper(CopperLayer::Top),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_PASTE,
        PcbLayer::Technical(TechnicalLayer::PasteTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_MASK,
        PcbLayer::Technical(TechnicalLayer::SolderMaskTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_SILK,
        PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_COURTYARD,
        PcbLayer::Technical(TechnicalLayer::CourtyardTop),
    ),
];

fn entry<'a>(
    label: MessageId,
    colour: Color,
    ink: Color,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let swatch = container(
        Space::new()
            .width(TS.caption)
            .height(TS.caption * SWATCH_HEIGHT),
    )
    .style(move |_: &iced::Theme| container::Style {
        background: Some(colour.into()),
        ..container::Style::default()
    });
    row![swatch, text(t!(i18n, label)).size(TS.caption).color(ink)]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}

/// The legend at the canvas's top right, under the editor bar.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    if !editor.state.sketch_exit.preview {
        return Space::new().boxed();
    }
    let ink = theme_ext::text_secondary(tokens);
    let mut entries: Row<Element<'a, LibraryMessage>> =
        Row::new().spacing(SP.m).align_y(Alignment::Center);
    for (label, layer) in LAYERS {
        entries = entries.push(entry(label, board_layer_colour(layer), ink, i18n));
    }
    let fill = theme_ext::to_color(&tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    let card = container(entries)
        .padding([SP.xs, SP.m])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(column![
        Space::new().height(TS.body * TOP_CLEARANCE_EM),
        card
    ])
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::overlay;
    use crate::app::FootprintDocument;
    use crate::layout_probe::lay_out_texts;
    use crate::library::editor::footprint::EditorMode;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn legend_texts(preview: bool) -> Vec<String> {
        let path = std::path::PathBuf::from("legend.snxfpt");
        let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty(
            "legend",
        ));
        let mut editor = FootprintDocument::new(path, file);
        editor.state.mode = EditorMode::Sketch;
        editor.state.sketch_exit.preview = preview;
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let i18n = signex_i18n::I18n::en_us();
        lay_out_texts(
            overlay(&editor, &tokens, &i18n),
            iced::Size::new(1400.0, 900.0),
        )
        .into_iter()
        .map(|t| t.text)
        .collect()
    }

    #[test]
    fn the_open_preview_names_its_layers_in_order() {
        assert_eq!(
            legend_texts(true),
            ["Top", "Paste", "Mask", "Silk", "Courtyard"]
        );
    }

    #[test]
    fn a_closed_preview_shows_no_legend() {
        assert!(legend_texts(false).is_empty());
    }
}
