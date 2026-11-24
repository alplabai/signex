//! The floating pad link on the footprint sketch canvas (mockup row 192):
//! "◂ Back to pad N" after Edit in Sketch from a pad, "Jump to pad ▸" while
//! a pad's driving entity is selected. Both show that pad in Footprint
//! mode. Nothing shows on a board sketch or without a pad to go to.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::text::LineHeight;
use iced::widget::{Space, button as button_widget, container};
use iced::{Background, Border, Element, Length, Padding, Pixels, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{PadLinkMsg, PadLinkOffer};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::tokens::{Overlay, composite_over, overlay, resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The link directly above the mode bar, left-aligned with it, so it never
/// covers Bake (mockup row 192); an empty layer when there is nothing to
/// link to.
pub fn overlay_link<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(link) = link_button(editor, tokens, i18n) else {
        return Space::new().boxed();
    };
    container(link)
        .padding(Padding {
            bottom: SP.xl + mode_bar_height() + SP.m,
            ..Padding::new(SP.xl)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Bottom)
        .boxed()
}

/// The link button itself, unplaced; `None` when there is nothing to link
/// to. The mode bar's column (`sketch_exit::footprint_overlay`) can stack
/// it first, so it sits just above the preview card while that is open.
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
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PadLink(
                PadLinkMsg::PadLinkPressed {
                    entity: offer.entity(),
                },
            )),
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
    Some(link.boxed())
}

/// Height of the sketch mode bar (`sketch_exit::mode_bar`) under the link:
/// its buttons or its title line, whichever is taller, inside the bar's
/// padding.
fn mode_bar_height() -> f32 {
    let title = LineHeight::default().to_absolute(Pixels(TS.body)).0;
    crate::styles::ChromeMetrics::DEFAULT.button.max(title) + 2.0 * SP.s
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iced::Rectangle;
    use iced::widget::Stack;
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
        let layers = Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(super::super::sketch_exit::footprint_overlay(
                &editor, &tokens, &i18n,
            ))
            .push(overlay_link(&editor, &tokens, &i18n));
        let width = 480.0;
        let texts = lay_out_texts(layers.boxed(), iced::Size::new(width, 360.0));
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
