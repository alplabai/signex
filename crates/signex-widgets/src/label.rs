//! The only text the house widgets accept (#2056).

use std::borrow::Cow;

use iced::advanced::text::IntoFragment;
use signex_i18n::{Localized, UserText};

/// Text for a house widget. It is built from a catalog message
/// ([`Localized`]), from text the user wrote ([`UserText`]), or, for text not
/// yet migrated to the catalog, from [`raw`].
#[derive(Debug, Clone)]
pub struct Label<'a>(Cow<'a, str>);

impl From<Localized> for Label<'_> {
    fn from(text: Localized) -> Self {
        Self(Cow::Owned(text.into_string()))
    }
}

impl From<UserText> for Label<'_> {
    fn from(text: UserText) -> Self {
        Self(Cow::Owned(text.as_str().to_owned()))
    }
}

impl<'a> From<&'a UserText> for Label<'a> {
    fn from(text: &'a UserText) -> Self {
        Self(Cow::Borrowed(text.as_str()))
    }
}

impl<'a> IntoFragment<'a> for Label<'a> {
    fn into_fragment(self) -> Cow<'a, str> {
        self.0
    }
}

impl Label<'_> {
    /// The text, for measuring it (a button sized to its label).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_string(self) -> String {
        self.0.into_owned()
    }
}

/// The escape hatch for text that is neither a catalog message nor user
/// content: a literal or a `format!` the area migrations (#2057-#2062) have
/// not reached yet. `scripts/check_raw_labels.sh` counts the call sites and
/// the count may only go down, so never add one to new code: use `t!` or
/// [`UserText`].
pub fn raw<'a>(text: impl IntoFragment<'a>) -> Label<'a> {
    Label(text.into_fragment())
}
