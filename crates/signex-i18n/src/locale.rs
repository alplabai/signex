//! The Language setting and RFC 4647 lookup negotiation.

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use icu_list::provider::{ListAndV1, ListFormatterPatterns};
use icu_locale_core::{DataLocale, Locale};
use icu_provider::{DataIdentifierBorrowed, DataProvider, DataRequest};
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
pub(crate) fn requested(tags: &[String]) -> Vec<LanguageIdentifier> {
    tags.iter().filter_map(|tag| tag.parse().ok()).collect()
}

/// The raw tags `pref` asks for, most wanted first, extensions intact.
pub(crate) fn requested_tags(pref: &LanguagePref) -> Vec<String> {
    match pref {
        LanguagePref::System => sys_locale::get_locales().collect(),
        LanguagePref::Locale(tag) => vec![tag.clone()],
    }
}

/// The locale numbers, dates and lists follow: the first requested tag with
/// its `-u-` extensions (`th-TH-u-ca-buddhist`), or `message_tag` when it
/// does not parse or ICU has no data for its language.
pub(crate) fn format_locale(tags: &[String], message_tag: &str) -> Locale {
    tags.first()
        .and_then(|tag| tag.parse::<Locale>().ok())
        .filter(has_icu_data)
        .or_else(|| message_tag.parse().ok())
        .unwrap_or(icu_locale_core::locale!("en-US"))
}

/// Whether the compiled ICU data holds `locale`'s language. Compiled data
/// falls back inside `load` and reports the locale it found; `und` means
/// only the root answered. The probe is the list patterns, which every
/// shipped language has: decimal and date data are stored only where they
/// differ from the root, so `en` and `th` would look unknown there. It is
/// the provider's own lookup, so it cannot disagree with the formatters.
fn has_icu_data(locale: &Locale) -> bool {
    let asked = DataLocale::from(locale);
    let request = DataRequest {
        id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
            ListFormatterPatterns::WIDE,
            &asked,
        ),
        ..Default::default()
    };
    DataProvider::<ListAndV1>::load(&icu_list::provider::Baked, request).is_ok_and(|found| {
        // No resolved locale means the asked one matched exactly.
        !found.metadata.locale.unwrap_or(asked).is_unknown()
    })
}

/// Whether a request fell back to `en-US` with no catalog in its language:
/// `tr-TR` did, `en-GB` did not (it is English), and a tag that does not
/// parse counts as lost. Decided from the negotiation result, so case and
/// script variants of an available locale are not lost.
pub(crate) fn is_dropped(requested: &[LanguageIdentifier], resolved: &str) -> bool {
    resolved == EN_US && requested.first().is_none_or(|first| first.language != "en")
}
