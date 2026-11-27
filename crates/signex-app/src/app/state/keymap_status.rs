//! The status line of Preferences ▸ Keyboard shortcuts (#3050).

use signex_i18n::{I18n, MessageId, Translatable};

/// The result of the last profile action, load, save or edit on the
/// Keyboard shortcuts page: a catalog message with its arguments, rendered
/// in the interface language when the page is drawn. Empty shows nothing.
/// Boxed, so an empty status costs one pointer in `UiState`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeymapStatus(Option<Box<Translatable>>);

impl KeymapStatus {
    /// Show `message`.
    pub fn set(&mut self, message: Translatable) {
        self.0 = Some(Box::new(message));
    }

    /// Show nothing.
    pub fn clear(&mut self) {
        self.0 = None;
    }

    /// Whether nothing is shown.
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// The catalog id of the message shown.
    pub fn id(&self) -> Option<MessageId> {
        self.0.as_deref().and_then(Translatable::id)
    }

    /// The text in `i18n`'s language; empty when nothing is shown.
    pub fn render(&self, i18n: &I18n) -> String {
        self.0
            .as_deref()
            .map(|message| message.render(i18n))
            .unwrap_or_default()
    }

    /// The text in en-US, as a log line or a test reads it.
    pub fn en_us(&self) -> String {
        self.render(I18n::en_us_ref())
    }

    /// Whether the en-US text contains `needle`.
    pub fn contains(&self, needle: &str) -> bool {
        self.en_us().contains(needle)
    }

    /// Whether the en-US text starts with `prefix`.
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.en_us().starts_with(prefix)
    }
}

impl std::fmt::Display for KeymapStatus {
    /// The en-US text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.en_us())
    }
}
