//! The view hooks an extension draws a dock panel or a Start-screen section
//! with. A GUI adapter: it stays out of `app/core/` (ADR-0006).

use iced::{Element, Task};
use signex_i18n::I18n;
use signex_types::theme::ThemeTokens;

use super::ExtensionMsg;
use crate::app::Message;

/// What a view hook may read while it draws: the theme and the language
/// the rest of the window draws in. Sizes come from the theme and the font
/// size, never fixed pixels; text goes through the `.ftl` catalogs.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct ViewContext<'a> {
    /// The active theme's colours.
    pub tokens: &'a ThemeTokens,
    /// The language the window draws in.
    pub i18n: &'a I18n,
}

impl<'a> ViewContext<'a> {
    pub(crate) fn new(tokens: &'a ThemeTokens, i18n: &'a I18n) -> Self {
        Self { tokens, i18n }
    }
}

/// One surface an extension draws: a dock panel or a Start-screen section.
///
/// The object is the surface's state. It lives in the application model,
/// and after install only the update loop calls into it: [`Self::view`] is
/// pure, [`Self::update`] is its one mutation site. A message the view sends
/// is an [`ExtensionMsg`] made with [`ExtensionMsg::for_view`] and this
/// surface's key; the application hands it to this surface's `update`.
pub trait ExtensionView: Send + 'static {
    /// Draw the surface. Pure: no IO and no mutation.
    fn view<'a>(&'a self, cx: ViewContext<'a>) -> Element<'a, ExtensionMsg>;

    /// Called with each message addressed to this surface. Long or blocking
    /// work goes in the returned `Task`. Does nothing by default.
    fn update(&mut self, _message: ExtensionMsg) -> Task<Message> {
        Task::none()
    }
}
