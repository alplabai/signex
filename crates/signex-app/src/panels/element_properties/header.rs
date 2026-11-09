//! The header row of the single-selection Properties: the kind of the
//! selected object. A placed part reads "Component" with its description
//! beside it; every other object shows the engine's type name.

use super::super::*;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The title a placed part's header shows.
pub(super) const COMPONENT_TITLE: &str = "Component";

/// Title and the muted line beside it. A placed part is a "Component" with
/// its description; anything else keeps `elem_type` alone.
pub(super) fn header_text<'s>(
    elem_type: &'s str,
    is_placed_part: bool,
    description: &'s str,
) -> (&'s str, Option<&'s str>) {
    if is_placed_part {
        let line = description.trim();
        (COMPONENT_TITLE, (!line.is_empty()).then_some(line))
    } else {
        (elem_type, None)
    }
}

/// The header row. No "1 of N" counter: this view shows one object, and a
/// count is shown only where the panel steps through a real list.
pub(super) fn selection_header<'a>(
    ctx: &PanelContext,
    elem_type: &str,
    is_placed_part: bool,
    description: &str,
    primary: Color,
) -> Element<'a, PanelMsg> {
    let (title, line) = header_text(elem_type, is_placed_part, description);
    let ink3 = resolve(&ctx.tokens).ink3;
    let mut header = iced::widget::Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .push(
            text(title.to_owned())
                .size(TS.body)
                .color(primary)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::DEFAULT
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    if let Some(line) = line {
        header = header.push(
            text(line.to_owned())
                .size(TS.dense)
                .color(ink3)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
        );
    }
    container(header)
        .padding([10.0, 12.0])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placed_part_is_a_component_with_its_description() {
        assert_eq!(
            header_text("Symbol", true, "Resistor (IEC rectangle). "),
            ("Component", Some("Resistor (IEC rectangle)."))
        );
    }

    #[test]
    fn a_part_without_a_description_shows_only_the_title() {
        assert_eq!(header_text("Symbol", true, "  "), ("Component", None));
    }

    #[test]
    fn other_objects_keep_the_type_name() {
        assert_eq!(header_text("Wire", false, ""), ("Wire", None));
    }
}
