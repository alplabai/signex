//! The Language setting and RFC 4647 lookup negotiation.

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use unic_langid::LanguageIdentifier;

use crate::catalog::{AVAILABLE, EN_US};

/// What the user chose in Preferences. An absent saved value is `System`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguagePref {
    /// Follow the operating system's language list.
    System,
    /// A BCP 47 tag such as `en-US`.
    Locale(String),
}

impl LanguagePref {
    /// The saved tag; `None` is `System`.
    pub fn from_saved(tag: Option<&str>) -> Self {
        tag.map_or(Self::System, |tag| Self::Locale(tag.to_owned()))
    }

    /// The tag to save; `None` (no key) is `System`.
    pub fn saved_tag(&self) -> Option<&str> {
        match self {
            Self::System => None,
            Self::Locale(tag) => Some(tag),
        }
    }
}

/// The best available locale for `requested`, most wanted first: `tr-TR`
/// tries `tr-TR`, then `tr`, and falls back to `en-US`.
pub(crate) fn negotiate(requested: &[LanguageIdentifier]) -> &'static str {
    let available: Vec<LanguageIdentifier> =
        AVAILABLE.iter().filter_map(|t| t.parse().ok()).collect();
    let default: LanguageIdentifier = EN_US.parse().expect("en-US is a valid tag");
    let picked = negotiate_languages(
        requested,
        &available,
        Some(&default),
        NegotiationStrategy::Lookup,
    );
    let tag = picked.first().map(|lang| lang.to_string());
    AVAILABLE
        .into_iter()
        .find(|known| Some(*known) == tag.as_deref())
        .unwrap_or(EN_US)
}

/// The tags `pref` asks for, most wanted first. Tags that do not parse are
/// skipped.
pub(crate) fn requested(pref: &LanguagePref) -> Vec<LanguageIdentifier> {
    match pref {
        LanguagePref::System => sys_locale::get_locales()
            .filter_map(|tag| tag.parse().ok())
            .collect(),
        LanguagePref::Locale(tag) => tag.parse().into_iter().collect(),
    }
}

/// Whether `tag` is a locale this build has, spelled exactly as listed.
pub(crate) fn is_available(tag: &str) -> bool {
    AVAILABLE.contains(&tag)
}
