//! The Properties form look of an enable-header subsection (epic #2523,
//! BB3). The widget is [`signex_widgets::enable_section::EnableSection`];
//! this module fills its style from the theme tokens and the form controls.
#![allow(
    dead_code,
    reason = "a building block of epic #2523; the Properties panels adopt it in later units"
)]

use signex_types::theme::ThemeTokens;
use signex_widgets::enable_section::EnableSectionStyle;

use super::form_look;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The form look of an enable section on `tokens`: the header label in the
/// secondary ink, the form checkbox, the disabled ink for a read-only ticked
/// header, and the panel colour for the veil over switched-off fields.
pub(crate) fn enable_section_style(tokens: &ThemeTokens) -> EnableSectionStyle<'static> {
    EnableSectionStyle {
        header_ink: ti(tokens.text_secondary),
        disabled_ink: resolve(tokens).ink4,
        panel: ti(tokens.panel_bg),
        font: crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT),
        checkbox: Box::new(form_look::checkbox_style),
        tip: Box::new(crate::styles::context_menu(tokens)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn the_header_reads_in_the_secondary_ink_and_greys_with_ink4() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let style = enable_section_style(&t);
            assert_eq!(style.header_ink, ti(t.text_secondary), "{id:?}");
            assert_eq!(style.disabled_ink, resolve(&t).ink4, "{id:?}");
            assert_eq!(style.panel, ti(t.panel_bg), "{id:?}");
        }
    }
}
