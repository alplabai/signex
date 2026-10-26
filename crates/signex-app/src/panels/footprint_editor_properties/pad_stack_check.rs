//! Properties-panel "Pad stack check" section (#1073): the one-time offer to
//! restore paste and thermal relief on pre-#1052 sketch pads, and the
//! standing warning listing SMD pads that have no paste opening.

use iced::widget::{Column, button, container, row, text};
use iced::{Background, Color, Element, Length, Theme};

use super::super::{CollapsedSections, FootprintEditorPanelContext, PanelMsg, PanelPalette};
use super::{fp_is_collapsed, props_section_header};

/// Pads named in the warning before it truncates to "+N more".
const LISTED_PADS: usize = 12;

pub(super) fn view_pad_stack_check<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    if fp.legacy_pad_stack_offer.is_none() && fp.paste_disabled_smd_pads.is_empty() {
        return col;
    }
    let PanelPalette {
        primary, border, ..
    } = palette;
    let warn = Color::from_rgba(1.00, 0.75, 0.30, 1.00);
    col = col.push(props_section_header(
        "Pad stack check",
        "fp_pad_stack_check",
        collapsed_sections,
        primary,
        border,
    ));
    if fp_is_collapsed("fp_pad_stack_check", collapsed_sections) {
        return col;
    }
    if let Some(n) = fp.legacy_pad_stack_offer {
        col = col.push(
            container(
                text(format!(
                    "All {n} sketch pad(s) have paste disabled and direct connect, the \
                     stack written by builds before #1052. They produce no paste stencil \
                     openings. Enable paste and thermal relief on them? Undo reverts it; \
                     nothing is saved until you save."
                ))
                .size(10)
                .color(warn),
            )
            .padding([4, 8])
            .width(Length::Fill),
        );
        col = col.push(
            container(
                row![
                    action_button(
                        format!("Enable paste and thermal relief on {n} pad(s)"),
                        PanelMsg::FpEditorRestorePadStackDefaults,
                    ),
                    action_button(
                        "Keep as is".to_string(),
                        PanelMsg::FpEditorDismissPadStackOffer
                    ),
                ]
                .spacing(6),
            )
            .padding([2, 8])
            .width(Length::Fill),
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
        col = col.push(
            container(
                text(format!(
                    "Warning: {total} SMD pad(s) have paste disabled on their copper side \
                     and get no stencil opening: {listed}"
                ))
                .size(10)
                .color(warn),
            )
            .padding([4, 8])
            .width(Length::Fill),
        );
    }
    col
}

fn action_button<'a>(label: String, msg: PanelMsg) -> Element<'a, PanelMsg> {
    button(text(label).size(10))
        .padding([2, 8])
        .on_press(msg)
        .style(move |_t: &Theme, _| iced::widget::button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 0.75, 0.30, 0.12))),
            text_color: Color::WHITE,
            ..iced::widget::button::Style::default()
        })
        .into()
}
