//! The chrome around a rule drawing: the "Preview" header with the
//! target's caption and the Expand button, and the hint under the drawing.

use std::sync::LazyLock;

use iced::widget::{Space, container, row, svg};
use iced::{Alignment, Background, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text, tooltip};

use super::clearance::Object;
use super::{PreviewMsg, PreviewTarget};
use crate::styles::tokens::{Overlay, muted_text, overlay, resolve};
use crate::styles::{ChromeMetrics, Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

static EXPAND: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("expand.svg").as_slice()));

/// What the header names: the rule's caption or the pair.
fn caption(target: &PreviewTarget, i18n: &I18n) -> Label<'static> {
    match target {
        PreviewTarget::Rule { caption, .. } => UserText::new(caption.clone()).into(),
        PreviewTarget::ClassPair { a, b, .. } => {
            t!(i18n, ids::RULES_PREVIEW_PAIR, "a" => a.as_str(), "b" => b.as_str()).into()
        }
        PreviewTarget::ObjectPair { objects, .. } => {
            let [a, b] = objects.each_ref().map(|o| {
                let name: Localized = t!(i18n, Object::parse(o).name());
                name.into_string()
            });
            t!(i18n, ids::RULES_PREVIEW_PAIR, "a" => a.as_str(), "b" => b.as_str()).into()
        }
    }
}

pub(super) fn header<'a>(
    is_expanded: bool,
    target: &PreviewTarget,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PreviewMsg> {
    let muted = muted_text(tokens);
    let faint = resolve(tokens).ink4;
    let heading = text(t!(i18n, ids::RULES_PREVIEW_HEADING))
        .size(TS.section)
        .color(muted);
    let caption = text(caption(target, i18n)).size(TS.caption).color(faint);
    row![
        heading,
        Space::new().width(Length::Fill),
        caption,
        expand_button(is_expanded, i18n, tokens),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

fn expand_button<'a>(
    is_expanded: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, PreviewMsg> {
    let ink = ti(tokens.text_secondary);
    let hover = overlay(tokens, Overlay::Hover);
    let pressed = overlay(tokens, Overlay::Pressed);
    let side = ChromeMetrics::DEFAULT.field;
    let glyph =
        svg(EXPAND.clone())
            .width(TS.title)
            .height(TS.title)
            .style(move |_: &iced::Theme, _| svg::Style {
                color: Some(ink),
                ..Default::default()
            });
    let btn = button(
        container(glyph)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(side)
    .height(side)
    .padding(0)
    .on_press(PreviewMsg::ExpandToggled)
    .style(move |_: &iced::Theme, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(hover)),
            button::Status::Pressed => Some(Background::Color(pressed)),
            _ => None,
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    let tip = if is_expanded {
        ids::RULES_PREVIEW_COLLAPSE
    } else {
        ids::RULES_PREVIEW_EXPAND
    };
    let raised = resolve(tokens).raised;
    let border = ti(tokens.border);
    let hint = container(text(t!(i18n, tip)).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.m])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            raised, border,
        ));
    tooltip(btn, hint, tooltip::Position::Bottom)
        .gap(SP.xs)
        .boxed()
}

/// The hint under the drawing.
pub(super) fn hint<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, PreviewMsg> {
    text(t!(i18n, ids::RULES_PREVIEW_HINT))
        .size(TS.caption)
        .color(resolve(tokens).ink4)
        .boxed()
}
