//! The floating pad links on the footprint canvas (mockup row 192). On the
//! sketch canvas: "◂ Back to pad N" after Edit in Sketch from a pad, "Jump
//! to pad ▸" while a pad's driving entity is selected; both show that pad
//! in Footprint mode. On the Footprint canvas: "Edit in Sketch ▸" while a
//! selected pad has sketch geometry behind it. Nothing shows on a board
//! sketch or without a pad to go to.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, button as button_widget, container};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{PadLinkMsg, PadLinkOffer};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::tokens::{Overlay, composite_over, overlay, resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The link button; `None` when there is nothing to link to. The mode bar's
/// column (`sketch_exit::footprint_overlay`) stacks it first, so it sits just
/// above the preview card while that is open and above the mode bar
/// otherwise, left-aligned, never covering Bake (mockup row 192).
pub fn link_button<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let offer = editor.state.pad_link_offer()?;
    let label = match offer {
        PadLinkOffer::BackToPad { pad_idx, .. } => {
            let designator = editor
                .state
                .pads
                .get(pad_idx)
                .map(|pad| pad.number.as_str())
                .unwrap_or_default();
            t!(i18n, ids::SKETCH_TOOLS_BACK_TO_PAD, "designator" => designator)
        }
        PadLinkOffer::JumpToPad { .. } => t!(i18n, ids::SKETCH_TOOLS_JUMP_TO_PAD),
    };
    let msg = PadLinkMsg::PadLinkPressed {
        entity: offer.entity(),
    };
    Some(floating_link(editor, label, msg, tokens))
}

/// The Footprint-mode overlay with the floating "Edit in Sketch ▸" link at
/// the canvas's bottom left while a selected pad has sketch geometry behind
/// it (mockup row 192); empty otherwise.
pub fn edit_in_sketch_overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let offer = editor
        .state
        .edit_in_sketch_offer(editor.primitive().sketch.as_ref());
    let Some(pad_idx) = offer else {
        return Space::new().boxed();
    };
    let label = t!(i18n, ids::EDITORS_FP_PAD_EDIT_IN_SKETCH);
    let msg = PadLinkMsg::EditInSketchPressed { pad_idx };
    container(floating_link(editor, label, msg, tokens))
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Bottom)
        .boxed()
}

/// A floating link button rimmed in the selection colour that sends `msg`.
fn floating_link<'a>(
    editor: &'a FootprintDocument,
    label: Localized,
    msg: PadLinkMsg,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ink = theme_ext::text_primary(tokens);
    let rest = resolve(tokens).raised;
    let hover = composite_over(overlay(tokens, Overlay::Hover), rest);
    let pressed = composite_over(overlay(tokens, Overlay::Pressed), rest);
    // The selection colour rims the link, as it does the profile it names.
    let rim = theme_ext::accent(tokens);
    let link = button(text(label).size(TS.label).color(ink))
        .padding([SP.xs, SP.l])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: editor.path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PadLink(msg)),
        })
        .style(move |_: &Theme, status: button_widget::Status| {
            let fill = match status {
                button_widget::Status::Hovered => hover,
                button_widget::Status::Pressed => pressed,
                _ => rest,
            };
            button_widget::Style {
                background: Some(Background::Color(fill)),
                text_color: ink,
                border: Border {
                    radius: crate::styles::Radii::GRAPHITE.small.into(),
                    width: 1.0,
                    color: rim,
                    ..Border::default()
                },
                ..button_widget::Style::default()
            }
        });
    link.boxed()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iced::Rectangle;
    use signex_library::{Footprint, FootprintFile, Pad};
    use signex_sketch::id::SketchEntityId;
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;
    use crate::layout_probe::{find, lay_out_texts};
    use crate::library::editor::footprint::EditorMode;

    /// A footprint in sketch mode with pad 1's driving entity selected, so
    /// the link offers Jump to pad.
    fn jump_offer() -> FootprintDocument {
        let entity = SketchEntityId::new();
        let mut fp = Footprint::empty("link");
        fp.pads.push(Pad {
            number: "1".into(),
            ..Pad::default()
        });
        let mut editor = FootprintDocument::new(
            PathBuf::from("link.snxfpt"),
            FootprintFile::from_footprint(fp),
        );
        editor.state.mode = EditorMode::Sketch;
        if let Some(pad) = editor.state.pads.first_mut() {
            pad.sketch_entity_id = Some(entity);
        }
        editor.state.selected_sketch = Some(entity);
        editor
    }

    /// A footprint in Footprint mode whose pad 1 a sketch point drives,
    /// selected when `selected`.
    fn driven_pad(selected: bool) -> FootprintDocument {
        let mut fp = Footprint::empty("link");
        fp.pads.push(Pad {
            number: "1".into(),
            ..Pad::default()
        });
        fp.sketch = Some(signex_sketch::SketchData::default());
        let mut editor = FootprintDocument::new(
            PathBuf::from("link.snxfpt"),
            FootprintFile::from_footprint(fp),
        );
        editor.state.mode = EditorMode::Normal;
        if let Some(pad) = editor.state.pads.first_mut() {
            pad.sketch_entity_id = Some(SketchEntityId::new());
        }
        editor.state.selected_pad = selected.then_some(0);
        editor
    }

    /// Mockup row 192: a selected pad the sketch drives shows a floating
    /// "Edit in Sketch ▸" on the Footprint canvas, at its bottom left, and
    /// a click on it asks for that pad's sketch.
    #[test]
    fn the_footprint_canvas_offers_edit_in_sketch_for_a_selected_driven_pad() {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let i18n = I18n::en_us();
        let label = t!(i18n, ids::EDITORS_FP_PAD_EDIT_IN_SKETCH).into_string();
        let window = iced::Size::new(480.0, 360.0);

        let unselected = driven_pad(false);
        let texts = lay_out_texts(edit_in_sketch_overlay(&unselected, &tokens, &i18n), window);
        assert!(texts.iter().all(|t| t.text != label), "{texts:?}");

        let editor = driven_pad(true);
        let texts = lay_out_texts(edit_in_sketch_overlay(&editor, &tokens, &i18n), window);
        let link = find(&texts, &label).bounds;
        assert!(link.x < window.width / 2.0, "{link:?} is not at the left");
        assert!(
            link.y > window.height / 2.0,
            "{link:?} is not at the bottom"
        );

        let sent = crate::layout_probe::messages_on_click(
            edit_in_sketch_overlay(&editor, &tokens, &i18n),
            window,
            &label,
        );
        assert!(
            sent.iter().any(|m| matches!(
                m,
                LibraryMessage::PrimitiveEditorEvent {
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PadLink(
                        PadLinkMsg::EditInSketchPressed { pad_idx: 0 }
                    )),
                    ..
                }
            )),
            "a click sends Edit in Sketch for pad 1: {sent:?}"
        );
    }

    fn overlaps(a: Rectangle, b: Rectangle) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    /// Mockup row 192: on a narrow canvas the link sits above the mode bar,
    /// left-aligned with it, and covers neither Bake nor any other part of
    /// the bar.
    #[test]
    fn the_link_sits_above_the_mode_bar_and_never_covers_bake() {
        let editor = jump_offer();
        assert!(matches!(
            editor.state.pad_link_offer(),
            Some(PadLinkOffer::JumpToPad { .. })
        ));
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let i18n = I18n::en_us();
        let overlay = super::super::sketch_exit::footprint_overlay(&editor, &tokens, &i18n);
        let width = 480.0;
        let texts = lay_out_texts(overlay, iced::Size::new(width, 360.0));
        let word = |id: signex_i18n::MessageId| t!(i18n, id).into_string();
        let link = find(&texts, &word(ids::SKETCH_TOOLS_JUMP_TO_PAD)).bounds;
        let bake = find(&texts, &word(ids::EDITORS_SKETCH_BAKE)).bounds;
        let title = find(&texts, &word(ids::EDITORS_SKETCH_MODE_TITLE)).bounds;
        assert!(!overlaps(link, bake), "{link:?} covers Bake at {bake:?}");
        assert!(
            link.y + link.height <= title.y,
            "{link:?} is not above the mode bar title {title:?}"
        );
        assert!(link.x < width / 2.0, "{link:?} is not left-aligned");
        for laid in texts.iter().filter(|t| t.bounds != link) {
            assert!(
                !overlaps(link, laid.bounds),
                "{link:?} covers {:?} at {:?}",
                laid.text,
                laid.bounds
            );
        }
    }
}
