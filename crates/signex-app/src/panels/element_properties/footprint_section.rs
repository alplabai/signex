//! The FOOTPRINT section of a placed part's Properties: a card with the
//! footprint thumbnail, its name, pitch and pad count, and whether a 3D
//! model is attached. The footprint is read off the UI thread; the card
//! says so while it waits.

use super::super::*;
use super::footprint_card::{FootprintCardStatus, FootprintCardView};
use crate::panels::components_panel::details::footprint_canvas::FootprintPreview;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, TypeScale, ti};
use iced::Widget;
use iced::widget::Row;

const TS: TypeScale = TypeScale::DEFAULT;

/// Collapse key of the Footprint section.
pub(super) const FOOTPRINT_SECTION_KEY: &str = "sel_footprint";
/// Thumbnail box, px.
const THUMB_WIDTH: f32 = 116.0;
const THUMB_HEIGHT: f32 = 88.0;
/// Inner padding of the card, px.
const CARD_PAD: f32 = 10.0;
/// Gap between the thumbnail and the text column, px.
const CARD_GAP: f32 = 12.0;
/// Side margin of the card inside the panel, px.
const CARD_MARGIN_X: f32 = 12.0;

/// The lines of text beside the thumbnail, top to bottom: the name (mono),
/// then the secondary lines.
pub(super) fn card_lines(card: &FootprintCardView) -> Vec<String> {
    let mut lines = vec![card.name.clone()];
    match &card.status {
        FootprintCardStatus::Ready(facts) => {
            lines.push(facts.summary());
            if facts.has_3d_body {
                lines.push("3D body attached".to_string());
            }
        }
        FootprintCardStatus::Loading => lines.push("Reading footprint\u{2026}".to_string()),
        FootprintCardStatus::Failed(why) => {
            lines.push(format!("Footprint could not be read: {why}"))
        }
        FootprintCardStatus::Unlinked => {
            lines.push("Not linked to a library footprint".to_string());
        }
    }
    lines
}

/// Append the Footprint section when the part has a footprint.
pub(super) fn push_footprint_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
) -> Column<Element<'a, PanelMsg>> {
    let Some(card) = ctx.footprint_card.as_ref() else {
        return col;
    };
    col.push(
        ctx.collapsible(
            FOOTPRINT_SECTION_KEY,
            "Footprint",
            &ctx.collapsed_sections,
            move || {
                Column::new()
                    .width(Length::Fill)
                    .push(footprint_card(card, &ctx.tokens))
            },
        )
        .boxed(),
    )
}

fn footprint_card<'a>(card: &'a FootprintCardView, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut lines = card_lines(card).into_iter();
    let mut text_col: Column<Element<'a, PanelMsg>> = Column::new().spacing(3).width(Length::Fill);
    if let Some(name) = lines.next() {
        text_col = text_col.push(
            text(signex_i18n::UserText::new(name))
                .size(TS.dense)
                .font(iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..mono
                })
                .color(ti(tokens.text))
                .boxed(),
        );
    }
    for (i, line) in lines.enumerate() {
        // The summary reads in ink3, any later line (the 3D note) in ink4.
        let ink = if i == 0 { r.ink3 } else { r.ink4 };
        text_col = text_col.push(text(raw(line)).size(TS.caption).color(ink).boxed());
    }
    let mut body: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(CARD_GAP)
        .align_y(iced::Alignment::Center);
    if let Some(thumb) = thumbnail(card, tokens) {
        body = body.push(thumb);
    }
    body = body.push(text_col.boxed());
    container(
        container(body)
            .padding(CARD_PAD)
            .width(Length::Fill)
            .style(crate::styles::card(tokens)),
    )
    .padding([4.0, CARD_MARGIN_X])
    .width(Length::Fill)
    .boxed()
}

/// The footprint drawn on its canvas ground; an empty box while it is read, none
/// for a part without a library footprint.
fn thumbnail<'a>(
    card: &'a FootprintCardView,
    tokens: &ThemeTokens,
) -> Option<Element<'a, PanelMsg>> {
    use crate::library::editor::footprint::graphite_look_for;
    // The ground the preview paints, so the box has no seam around it.
    let ground = graphite_look_for(tokens).bg;
    let inner: Element<'a, PanelMsg> = match &card.status {
        FootprintCardStatus::Ready(facts) => {
            iced::widget::canvas(FootprintPreview::new(&facts.preview).with_theme_tokens(tokens))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed()
        }
        FootprintCardStatus::Loading | FootprintCardStatus::Failed(_) => Space::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        FootprintCardStatus::Unlinked => return None,
    };
    Some(
        container(inner)
            .width(THUMB_WIDTH)
            .height(THUMB_HEIGHT)
            .clip(true)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(ground)),
                border: Border {
                    radius: Radii::GRAPHITE.button.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::FootprintCardFacts;
    use signex_library::primitive::{Footprint, Pad, StepAttachment};
    use std::sync::Arc;

    fn view(status: FootprintCardStatus) -> FootprintCardView {
        FootprintCardView {
            name: "SOT-23-5".into(),
            key: None,
            version: 0,
            status,
        }
    }

    fn ready(step: bool) -> FootprintCardStatus {
        let mut fp = Footprint::empty("SOT-23-5");
        fp.pads = [(-0.95, 1.1), (0.0, 1.1), (0.95, 1.1)]
            .iter()
            .map(|(x, y)| Pad {
                position: [*x, *y],
                ..Pad::default()
            })
            .collect();
        if step {
            fp.step_attachment = Some(StepAttachment {
                content_hash: "ab".into(),
                filename: "SOT-23-5.step".into(),
                offset_xyz: [0.0; 3],
                rotation_xyz: [0.0; 3],
            });
        }
        FootprintCardStatus::Ready(Arc::new(FootprintCardFacts::of(&fp)))
    }

    #[test]
    fn a_read_footprint_lists_name_then_pitch_and_pads() {
        assert_eq!(
            card_lines(&view(ready(false))),
            ["SOT-23-5", "0.95 mm pitch, 3 pads"]
        );
    }

    #[test]
    fn the_3d_line_shows_only_with_an_attached_model() {
        assert_eq!(
            card_lines(&view(ready(true))),
            ["SOT-23-5", "0.95 mm pitch, 3 pads", "3D body attached"]
        );
    }

    #[test]
    fn a_pending_failed_or_unlinked_card_says_so() {
        assert_eq!(
            card_lines(&view(FootprintCardStatus::Loading))[1],
            "Reading footprint\u{2026}"
        );
        assert_eq!(
            card_lines(&view(FootprintCardStatus::Failed("gone".into())))[1],
            "Footprint could not be read: gone"
        );
        assert_eq!(
            card_lines(&view(FootprintCardStatus::Unlinked))[1],
            "Not linked to a library footprint"
        );
    }
}
