//! The Start screen's extension slot (#2495): on the Home page, below the
//! project lists, each section an installed extension added, under its
//! heading. With no section added the slot draws nothing at all.

use iced::Element;
use signex_i18n::{I18n, t};
use signex_types::theme::ThemeTokens;

use super::kit::section_header;
use crate::app::Message;
use crate::extension::points::StartScreenSlot;
use crate::extension::{ExtensionHost, ViewContext};

/// The parts the Home page's extension slot adds to the page column: each
/// section's heading, then its body. Empty when no extension added a
/// section.
pub(super) fn home_end<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    extensions: &'a ExtensionHost,
) -> Vec<Element<'a, Message>> {
    let cx = ViewContext::new(tokens, i18n);
    extensions
        .view_start_screen_slot(StartScreenSlot::HomeEnd, cx)
        .into_iter()
        .flat_map(|section| {
            [
                section_header(tokens, t!(i18n, section.title)),
                section.body.map(Message::Extension),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fixture, MOCK_WINDOW};
    use super::*;
    use crate::extension::test_extension::{BODY, LABEL, WELCOME, sample_host};

    #[test]
    fn the_slot_draws_nothing_with_no_extension_installed() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let i18n = I18n::en_us();
        let none = ExtensionHost::default();
        assert!(home_end(&tokens, &i18n, &none).is_empty());
    }

    /// The section shows under its heading after every built-in part of
    /// Home, and a click on it reaches its own update hook.
    #[test]
    fn a_section_ends_home_under_its_heading_and_a_click_reaches_its_update_hook() {
        let mut fx = Fixture::new(&[]);
        let community: Vec<String> = fx
            .texts(MOCK_WINDOW)
            .into_iter()
            .map(|laid_out| laid_out.text)
            .collect();
        let (host, seen) = sample_host();
        fx.extensions = host;
        let texts: Vec<String> = fx
            .texts(MOCK_WINDOW)
            .into_iter()
            .map(|laid_out| laid_out.text)
            .collect();

        let heading = fx.says(LABEL, &[]);
        let body = fx.says(BODY, &[]);
        assert!(!community.contains(&heading) && !community.contains(&body));
        assert_eq!(texts[..community.len()], community[..]);
        assert_eq!(texts[community.len()..], [heading, body.clone()]);

        let sent = fx.click(MOCK_WINDOW, &body);
        let [Message::Extension(message)] = sent.as_slice() else {
            panic!("one extension message: {sent:?}");
        };
        assert_eq!(message.view(), Some(WELCOME));
        let _ = fx.extensions.route(message.clone());
        assert_eq!(seen.lock().expect("seen").section, [1]);
    }
}
