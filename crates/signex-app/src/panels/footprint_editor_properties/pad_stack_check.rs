//! The "Pad stack check" card (#1073, look of #125): the one-time offer to
//! restore paste and thermal relief on pre-#1052 sketch pads, and the
//! standing warning listing SMD pads that have no paste opening. An inline
//! card on the theme's warning tone; its actions are the shipped Restore
//! and Keep as is.

use iced::widget::{Column, column, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_widgets::{button, raw, text};

use super::super::{CollapsedSections, FootprintEditorPanelContext, PanelMsg, PanelPalette};
use crate::styles::{Radii, Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Pads named in the warning before it truncates to "+N more".
const LISTED_PADS: usize = 12;

/// Alpha of the card's warning-tone fill.
const CARD_FILL_ALPHA: f32 = 0.15;

pub(super) fn view_pad_stack_check<'a>(
    col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    _palette: PanelPalette,
    _collapsed_sections: &'a CollapsedSections,
) -> Column<Element<'a, PanelMsg>> {
    if fp.legacy_pad_stack_offer.is_none() && fp.paste_disabled_smd_pads.is_empty() {
        return col;
    }
    let bold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let mut body: Column<Element<'a, PanelMsg>> =
        column![text(raw("Pad stack check")).size(TS.caption).font(bold)]
            .spacing(SP.xs)
            .width(Length::Fill);
    if let Some(n) = fp.legacy_pad_stack_offer {
        body = body.push(
            text(raw(format!(
                "All {n} sketch pad(s) have paste disabled and direct connect, the \
                 stack written by builds before #1052. They produce no paste stencil \
                 openings. Enable paste and thermal relief on them? Undo reverts it; \
                 nothing is saved until you save."
            )))
            .size(TS.caption),
        );
        body = body.push(
            row![
                action_button(
                    format!("Enable paste and thermal relief on {n} pad(s)"),
                    PanelMsg::FpEditorRestorePadStackDefaults,
                    false,
                ),
                action_button(
                    "Keep as is".to_string(),
                    PanelMsg::FpEditorDismissPadStackOffer,
                    true,
                ),
            ]
            .spacing(SP.s),
        );
    }
    if !fp.paste_disabled_smd_pads.is_empty() {
        let total = fp.paste_disabled_smd_pads.len();
        let mut listed = fp
            .paste_disabled_smd_pads
            .iter()
            .take(LISTED_PADS)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if total > LISTED_PADS {
            listed.push_str(&format!(", +{} more", total - LISTED_PADS));
        }
        body = body.push(
            text(raw(format!(
                "Warning: {total} SMD pad(s) have paste disabled on their copper side \
                 and get no stencil opening: {listed}"
            )))
            .size(TS.caption),
        );
    }
    let card = container(body)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let warn = warning_ink(theme);
            container::Style {
                background: Some(Background::Color(Color {
                    a: CARD_FILL_ALPHA,
                    ..warn
                })),
                text_color: Some(warn),
                border: Border {
                    radius: Radii::GRAPHITE.button.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        });
    col.push(
        container(card)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
    )
}

/// The theme's warning tone, which the Graphite themes seed from their
/// warning token.
fn warning_ink(theme: &Theme) -> Color {
    theme.palette().warning.base.color
}

/// A small card action: outlined, or quiet (no outline) for "Keep as is".
fn action_button<'a>(label: String, msg: PanelMsg, quiet: bool) -> Element<'a, PanelMsg> {
    button(text(raw(label)).size(TS.caption))
        .padding([SP.xs, SP.m])
        .on_press(msg)
        .style(move |theme: &Theme, status: button::Status| {
            let mut style = crate::panels::form_look::ghost_button(theme, status);
            if quiet {
                style.border = Border {
                    radius: style.border.radius,
                    ..Border::default()
                };
            }
            style.text_color = warning_ink(theme);
            style
        })
        .boxed()
}
