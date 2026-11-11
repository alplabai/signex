//! Small read-only form controls used by the Annotate modal — the inline
//! checkbox pip, the order-of-processing radio pill, the 2×2 order-preview
//! legend, and the shared bordered-container style. On Graphite each one
//! delegates to `graphite_annotate`; other themes keep the original look.

use super::super::*;
use iced::widget::{container, scrollable};
use iced::{Background, Border, Element, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::super::graphite_annotate as ga;
use crate::app::state::AnnotateOrder;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Scroll-rail style of the modal's lists: a quiet pill thumb (the
/// form-look rail).
pub(super) fn scroll_style(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'static {
    let r = crate::styles::tokens::resolve(theme_tokens);
    let (thumb, thumb_hot) = (r.line_strong, r.ink4);
    move |theme: &Theme, status: scrollable::Status| {
        let mut style = scrollable::default(theme, status);
        let idle = matches!(status, scrollable::Status::Active { .. });
        for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
            rail.background = None;
            rail.border = Border::default();
            rail.scroller.background = Background::Color(if idle { thumb } else { thumb_hot });
            rail.scroller.border = Border {
                radius: 999.0.into(),
                ..Border::default()
            };
        }
        style
    }
}

/// Greyed checkbox pip for the parameter-matching list, which the
/// annotation engine does not read yet.
pub(super) fn disabled_check_pip(
    on: bool,
    theme_tokens: &ThemeTokens,
) -> Element<'static, Message> {
    ga::disabled_checkbox_pip(theme_tokens, on)
}

pub(super) fn bordered_style(
    theme_tokens: &ThemeTokens,
) -> Box<dyn Fn(&Theme) -> container::Style + 'static> {
    Box::new(ga::field_box(theme_tokens))
}

pub(super) fn order_radio<'a>(
    label: &'a str,
    value: AnnotateOrder,
    current: AnnotateOrder,
    theme_tokens: &ThemeTokens,
) -> Element<'a, Message> {
    button(container(text(raw(label.to_string())).size(TS.body)).padding([SP.s, 10.0]))
        .on_press(Message::Annotate(AnnotateMsg::OrderChanged(value)))
        .style(ga::toggle_style(theme_tokens, value == current))
        .boxed()
}

/// The line under the Annotation Summary that says what "Re-prefix to
/// standard" will do, or why it is unavailable (#1315).
pub(super) fn reprefix_note(
    standard: Option<signex_types::designator::DesignatorStandard>,
) -> String {
    match standard {
        None => "Re-prefix to standard is unavailable: this project follows no designator \
                 standard. Pick one in Project Options."
            .to_string(),
        Some(standard) => format!(
            "Re-prefix to standard re-issues the prefix of every placed part under {standard} \
             and renumbers them, across the whole project, as one undo step, after you confirm a \
             list of the changes. Sheets that are not open are loaded unsaved; nothing is \
             written until you save. Locked parts, hand-edited prefixes and parts without a \
             library link are left alone and listed in Messages. PCB references are not \
             updated."
        ),
    }
}
