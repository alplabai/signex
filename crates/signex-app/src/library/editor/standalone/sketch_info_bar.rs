//! The sketch info bar (mockup rows 187 and 179): what is being sketched, on which plane
//! and at which grid step, and the Parameters button that opens the
//! parameters table. On a footprint it is the sketch half of the editor bar,
//! after the Footprint / Sketch switch; on a board it floats alone at the
//! canvas's top right, clear of the DOF chips at the top left.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_sketch::plane::PlaneKind;
use signex_sketch::unit::Quantity;
use signex_types::sketch::SketchHost;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::parameters_table::SketchParametersMsg;
use crate::library::editor::footprint::parameters_table::state::display_value;
use crate::library::editor::footprint::state::EditorMode;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, Radii, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The board bar's distance from the canvas top, in body text heights.
const TOP_CLEARANCE_EM: f32 = 3.5;

/// The plane new sketch geometry goes on, as the caption's selector: the
/// first plane, Top when the sketch has none yet.
fn plane_key(editor: &FootprintDocument) -> &'static str {
    let first = editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|s| s.planes.first());
    match first.map(|p| &p.kind) {
        Some(PlaneKind::BodyTop { .. }) => "body",
        Some(PlaneKind::BoardTop) | None => "top",
    }
}

/// The caption and the Parameters button of `editor`, in a row.
fn caption_and_parameters<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let caption_id = match editor.state.sketch_host() {
        SketchHost::Footprint => ids::SKETCH_INFO_FOOTPRINT,
        SketchHost::Board => ids::SKETCH_INFO_BOARD,
    };
    let (grid, _) = display_value(Quantity::length(editor.state.snap_options.grid_step_mm));
    let caption = t!(
        i18n,
        caption_id,
        "plane" => plane_key(editor),
        "grid" => grid
    );
    let parameters = LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchParameters(
            SketchParametersMsg::TableToggled,
        )),
    };
    let kind = if editor.state.parameters_table.is_some() {
        DialogButtonKind::Accent
    } else {
        DialogButtonKind::Secondary
    };
    row![
        text(caption)
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens)),
        dialog_button(
            t!(i18n, ids::SKETCH_INFO_PARAMETERS),
            kind,
            Some(parameters),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The sketch half of a footprint's editor bar (mockup row 187): the caption and
/// Parameters in Sketch mode, nothing in the other modes.
pub fn editor_bar_items<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    (editor.state.mode == EditorMode::Sketch).then(|| caption_and_parameters(editor, tokens, i18n))
}

/// The bar of a board sketch, floating at the canvas's top right.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let fill = theme_ext::to_color(&tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    let bar = container(caption_and_parameters(editor, tokens, i18n))
        .padding([SP.xs, SP.m])
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(fill.into()),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    container(column![
        Space::new().height(TS.body * TOP_CLEARANCE_EM),
        bar
    ])
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::editor_bar_items;
    use crate::app::FootprintDocument;
    use crate::layout_probe::{LaidOutText, lay_out_texts};
    use crate::library::editor::footprint::pads_active_bar::mode_switcher_overlay;
    use crate::library::editor::footprint::parameters_table::view::TOP_CLEARANCE_EM;
    use crate::library::editor::footprint::state::EditorMode;
    use signex_types::theme::{ThemeId, theme_tokens};

    /// The footprint editor bar of an empty footprint in `mode`, laid out in
    /// a 1400 x 900 canvas, its texts without Fluent's isolation marks.
    fn bar_texts(mode: EditorMode) -> Vec<LaidOutText> {
        let path = std::path::PathBuf::from("bar.snxfpt");
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("bar"));
        let mut editor = FootprintDocument::new(path, file);
        editor.state.mode = mode;
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let i18n = signex_i18n::I18n::en_us();
        let trailing = editor_bar_items(&editor, &tokens, &i18n);
        lay_out_texts(
            mode_switcher_overlay(&editor, &tokens, &i18n, trailing),
            iced::Size::new(1400.0, 900.0),
        )
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect()
    }

    fn find<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
        texts
            .iter()
            .find(|t| t.text == label)
            .unwrap_or_else(|| panic!("no {label:?} in {texts:?}"))
    }

    #[test]
    fn the_bar_reads_footprint_sketch_then_3d_left_to_right() {
        let texts = bar_texts(EditorMode::Normal);
        let footprint = find(&texts, "Footprint").bounds;
        let sketch = find(&texts, "Sketch").bounds;
        let three_d = find(&texts, "3D").bounds;
        assert!(footprint.x < sketch.x && sketch.x < three_d.x);
        assert!(!texts.iter().any(|t| t.text == "Pads"));
    }

    #[test]
    fn the_bar_sits_at_the_top_right_under_the_active_bar_band() {
        let band = crate::active_bar::bar_top_margin()
            + crate::active_bar::bar_height_for(&signex_widgets::active_bar::Metrics::GRAPHITE);
        for mode in [EditorMode::Normal, EditorMode::Sketch, EditorMode::View3d] {
            let texts = bar_texts(mode);
            assert!(!texts.is_empty(), "{mode:?}");
            for t in &texts {
                assert!(t.bounds.y >= band, "{mode:?}: {t:?} reaches the Active Bar");
                assert!(
                    t.bounds.x > 700.0,
                    "{mode:?}: {t:?} is not on the right half"
                );
            }
        }
    }

    #[test]
    fn in_sketch_mode_the_caption_and_parameters_follow_the_3d_toggle() {
        let texts = bar_texts(EditorMode::Sketch);
        let three_d = find(&texts, "3D").bounds;
        let caption = texts
            .iter()
            .find(|t| t.text.starts_with("Footprint sketch"))
            .unwrap_or_else(|| panic!("no caption in {texts:?}"))
            .bounds;
        let parameters = find(&texts, "Parameters").bounds;
        assert!(three_d.x < caption.x && caption.x < parameters.x);
        // The open parameters table card starts lower down.
        let card_top = crate::styles::TypeScale::DEFAULT.body * TOP_CLEARANCE_EM;
        for t in &texts {
            assert!(
                t.bounds.y + t.bounds.height <= card_top,
                "{t:?} reaches the table"
            );
        }
    }

    #[test]
    fn outside_sketch_mode_the_bar_has_no_caption_or_parameters() {
        for mode in [EditorMode::Normal, EditorMode::View3d] {
            let texts = bar_texts(mode);
            assert!(
                !texts.iter().any(|t| t.text == "Parameters"),
                "{mode:?}: {texts:?}"
            );
            assert!(!texts.iter().any(|t| t.text.starts_with("Footprint sketch")));
        }
    }
}
