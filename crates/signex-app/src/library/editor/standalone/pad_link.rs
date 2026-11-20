//! The floating pad link on the footprint sketch canvas (#192, m192):
//! "◂ Back to pad N" after Edit in Sketch from a pad, "Jump to pad ▸" while
//! a pad's driving entity is selected. Both show that pad in Footprint
//! mode. Nothing shows on a board sketch or without a pad to go to.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, button as button_widget, container};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{PadLinkMsg, PadLinkOffer};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::tokens::{Overlay, composite_over, overlay, resolve};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The link at the canvas's bottom centre, between the mode bar and the
/// zoom pill; an empty layer when there is nothing to link to.
pub fn overlay_link<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(offer) = editor.state.pad_link_offer() else {
        return Space::new().boxed();
    };
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
                },
                ..button_widget::Style::default()
            }
        });
    container(link)
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Bottom)
        .boxed()
}
