//! The property illustration of the footprint Properties panel and the
//! hover link between its callouts and the field rows (epic #2523).
//! The hover is model state (`FootprintEditorState::properties_hover`);
//! these helpers only draw it and publish its events.

use iced::widget::{container, mouse_area};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::property_illustration::{
    FieldId, Illustration, IllustrationHover, IllustrationMsg, property_illustration,
};

use super::{SP, TS};
use crate::panels::{FpPropertiesMsg, PanelMsg};
use crate::styles::tokens::resolve;

/// A hover event of the illustration or a field row, as a panel message.
pub(super) fn hover_msg(msg: IllustrationMsg) -> PanelMsg {
    PanelMsg::FpProperties(FpPropertiesMsg::Illustration(msg))
}

/// `illustration` at the panel width and its default height, on the panel
/// ground, inset like the field rows.
pub(super) fn view_illustration<'a>(
    illustration: Illustration,
    hover: &IllustrationHover,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    container(property_illustration(illustration, hover, tokens, TS.caption).map(hover_msg))
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// `content` linked to `field`: entering it highlights the callouts of
/// `field`, and a hovered callout of `field` tints it with the soft accent.
pub(super) fn linked_field<'a>(
    content: Element<'a, PanelMsg>,
    field: FieldId,
    hover: &IllustrationHover,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let tint = hover
        .is_field_highlighted(&field)
        .then(|| resolve(tokens).accent_soft);
    let body = container(content)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: tint.map(Background::Color),
            ..container::Style::default()
        })
        .boxed();
    mouse_area(body)
        .on_enter(hover_msg(IllustrationMsg::FieldHovered(field.clone())))
        .on_exit(hover_msg(IllustrationMsg::FieldLeft(field)))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both sides of the link reach the footprint Properties handler.
    #[test]
    fn hover_events_are_footprint_properties_messages() {
        let field = FieldId::new("pad-size-x");
        let msg = hover_msg(IllustrationMsg::FieldHovered(field.clone()));
        assert!(matches!(
            msg,
            PanelMsg::FpProperties(FpPropertiesMsg::Illustration(IllustrationMsg::FieldHovered(f)))
                if f == field
        ));
    }
}
