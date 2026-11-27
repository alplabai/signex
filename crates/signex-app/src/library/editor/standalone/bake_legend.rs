//! The layer colour legend over a footprint's bake preview (mockup row 190):
//! a swatch and name for each layer the preview draws, in the colours the
//! canvas draws them. Only the layers the bake has something on are listed,
//! the bottom ones among them when the bake uses them (Caner 2026-11-27).
//! Empty while
//! the preview is closed or draws none of them.

use iced::alignment::Horizontal;
use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::canvas::footprint_bake_layers;
use crate::library::editor::footprint::parameters_table::view::TOP_CLEARANCE_EM;
use crate::library::messages::LibraryMessage;
use crate::pcb_canvas::layer_style::board_layer_colour;
use crate::styles::Radii;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// A swatch's height, as a part of the caption size.
const SWATCH_HEIGHT: f32 = 0.25;

/// The layers the legend can list, in the mockup's order, each bottom
/// layer after its top counterpart.
const LAYERS: [(MessageId, PcbLayer); 10] = [
    (
        ids::SKETCH_BAKE_LEGEND_TOP,
        PcbLayer::Copper(CopperLayer::Top),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_BOTTOM,
        PcbLayer::Copper(CopperLayer::Bottom),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_PASTE,
        PcbLayer::Technical(TechnicalLayer::PasteTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_BOTTOM_PASTE,
        PcbLayer::Technical(TechnicalLayer::PasteBottom),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_MASK,
        PcbLayer::Technical(TechnicalLayer::SolderMaskTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_BOTTOM_MASK,
        PcbLayer::Technical(TechnicalLayer::SolderMaskBottom),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_SILK,
        PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_BOTTOM_SILK,
        PcbLayer::Technical(TechnicalLayer::SilkscreenBottom),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_COURTYARD,
        PcbLayer::Technical(TechnicalLayer::CourtyardTop),
    ),
    (
        ids::SKETCH_BAKE_LEGEND_BOTTOM_COURTYARD,
        PcbLayer::Technical(TechnicalLayer::CourtyardBottom),
    ),
];

/// The legend's entries for a preview that draws on `drawn`, in order.
fn entries(drawn: &[PcbLayer]) -> impl Iterator<Item = (MessageId, PcbLayer)> + '_ {
    LAYERS
        .into_iter()
        .filter(move |(_, layer)| drawn.contains(layer))
}

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
    let drawn = footprint_bake_layers(&editor.state, editor.primitive());
    if entries(&drawn).next().is_none() {
        return Space::new().boxed();
    }
    let ink = theme_ext::text_secondary(tokens);
    let mut row: Row<Element<'a, LibraryMessage>> =
        Row::new().spacing(SP.m).align_y(Alignment::Center);
    for (label, layer) in entries(&drawn) {
        row = row.push(entry(label, board_layer_colour(layer), ink, i18n));
    }
    let fill = theme_ext::to_color(&tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    let card = container(row)
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
    use super::{entries, overlay};
    use crate::app::FootprintDocument;
    use crate::layout_probe::lay_out_texts;
    use crate::library::editor::footprint::EditorMode;
    use crate::library::editor::footprint::state::{CourtyardRect, EditorPad};
    use signex_library::primitive::footprint::LayerId;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
    use signex_types::theme::{ThemeId, theme_tokens};

    fn editor() -> FootprintDocument {
        let path = std::path::PathBuf::from("legend.snxfpt");
        let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty(
            "legend",
        ));
        let mut editor = FootprintDocument::new(path, file);
        editor.state.mode = EditorMode::Sketch;
        editor
    }

    fn pad_on(number: &str, layer: &str) -> EditorPad {
        let mut pad = EditorPad::new_default(number.to_string(), (0.0, 0.0));
        pad.layers = vec![LayerId::new(layer)];
        pad
    }

    fn legend_texts(editor: &mut FootprintDocument, preview: bool) -> Vec<String> {
        editor.state.sketch_exit.preview = preview;
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let i18n = signex_i18n::I18n::en_us();
        lay_out_texts(
            overlay(editor, &tokens, &i18n),
            iced::Size::new(1400.0, 900.0),
        )
        .into_iter()
        .map(|t| t.text)
        .collect()
    }

    /// Caner 2026-11-27: only the layers the preview draws on are listed.
    #[test]
    fn the_open_preview_names_only_the_layers_it_draws() {
        let mut editor = editor();
        assert!(legend_texts(&mut editor, true).is_empty(), "nothing baked");
        editor.state.pads = vec![pad_on("1", "F.Cu")];
        assert_eq!(legend_texts(&mut editor, true), ["Top"]);
        editor.state.courtyard_mm = Some(CourtyardRect {
            min_x: -1.0,
            min_y: -1.0,
            max_x: 1.0,
            max_y: 1.0,
        });
        assert_eq!(legend_texts(&mut editor, true), ["Top", "Courtyard"]);
    }

    /// Bottom is listed when the preview draws a bottom pad, beside Top.
    #[test]
    fn bottom_is_listed_when_the_bake_uses_it() {
        let mut editor = editor();
        editor.state.layer_visibility.show_all();
        editor.state.pads = vec![pad_on("1", "F.Cu"), pad_on("2", "B.Cu")];
        assert_eq!(legend_texts(&mut editor, true), ["Top", "Bottom"]);
        editor.state.pads = vec![pad_on("2", "B.Cu")];
        assert_eq!(legend_texts(&mut editor, true), ["Bottom"]);
    }

    /// The baked regions name their layers too, in the mockup's order
    /// whatever order they come in.
    #[test]
    fn region_layers_are_listed_in_the_mockup_order() {
        use TechnicalLayer as T;
        let drawn = [
            PcbLayer::Technical(T::SilkscreenTop),
            PcbLayer::Technical(T::SolderMaskTop),
            PcbLayer::Technical(T::PasteTop),
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Technical(T::SolderMaskTop),
        ];
        let ids: Vec<_> = entries(&drawn).map(|(_, layer)| layer).collect();
        assert_eq!(
            ids,
            [
                PcbLayer::Copper(CopperLayer::Top),
                PcbLayer::Technical(T::PasteTop),
                PcbLayer::Technical(T::SolderMaskTop),
                PcbLayer::Technical(T::SilkscreenTop),
            ]
        );
    }

    /// Caner 2026-11-27: each bottom layer has its own entry, after its
    /// top counterpart, listed only when drawn.
    #[test]
    fn bottom_layers_follow_their_top_counterparts_when_drawn() {
        use TechnicalLayer as T;
        let i18n = signex_i18n::I18n::en_us();
        let names = |drawn: &[PcbLayer]| -> Vec<String> {
            entries(drawn)
                .map(|(id, _)| i18n.message_plain(id, &[]))
                .collect()
        };
        let drawn = [
            PcbLayer::Technical(T::CourtyardBottom),
            PcbLayer::Technical(T::SilkscreenBottom),
            PcbLayer::Technical(T::SolderMaskBottom),
            PcbLayer::Technical(T::PasteBottom),
            PcbLayer::Technical(T::CourtyardTop),
            PcbLayer::Technical(T::SilkscreenTop),
            PcbLayer::Technical(T::SolderMaskTop),
            PcbLayer::Technical(T::PasteTop),
            PcbLayer::Copper(CopperLayer::Bottom),
            PcbLayer::Copper(CopperLayer::Top),
        ];
        assert_eq!(
            names(&drawn),
            [
                "Top",
                "Bottom",
                "Paste",
                "Bottom paste",
                "Mask",
                "Bottom mask",
                "Silk",
                "Bottom silk",
                "Courtyard",
                "Bottom courtyard",
            ]
        );
        let only_bottom = [
            PcbLayer::Technical(T::PasteBottom),
            PcbLayer::Technical(T::SilkscreenBottom),
        ];
        assert_eq!(names(&only_bottom), ["Bottom paste", "Bottom silk"]);
    }

    /// Back silk the preview draws (its layer shown) is listed as Bottom
    /// silk; hidden, it is not drawn and not listed.
    #[test]
    fn drawn_back_silk_is_listed_as_bottom_silk() {
        use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
        let mut editor = editor();
        editor.primitive_mut().silk_b.push(FpGraphic {
            kind: FpGraphicKind::Line {
                from: [0.0, 0.0],
                to: [1.0, 0.0],
            },
            stroke_width: 0.15,
            filled: false,
            sketch_entity: None,
        });
        assert!(
            legend_texts(&mut editor, true).is_empty(),
            "hidden by default"
        );
        editor.state.layer_visibility.show_all();
        assert_eq!(legend_texts(&mut editor, true), ["Bottom silk"]);
    }

    #[test]
    fn a_closed_preview_shows_no_legend() {
        let mut editor = editor();
        editor.state.pads = vec![pad_on("1", "F.Cu")];
        assert!(legend_texts(&mut editor, false).is_empty());
    }
}
