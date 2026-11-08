//! The Language setting and RFC 4647 lookup negotiation.

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use icu_datetime::provider::semantic_skeletons::DatetimePatternsDateGregorianV1;
use icu_decimal::provider::DecimalSymbolsV1;
use icu_list::provider::{ListAndV1, ListFormatterPatterns};
use icu_locale_core::{DataLocale, Locale};
use icu_provider::{
    DataIdentifierBorrowed, DataMarker, DataMarkerAttributes, DataProvider, DataRequest,
};
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

/// The locale numbers, dates and lists follow: the first requested tag that
/// parses and has ICU data, with its `-u-` extensions
/// (`th-TH-u-ca-buddhist`), or `message_tag` when none does. Under System
/// default that walks the whole OS language list.
pub(crate) fn format_locale(tags: &[String], message_tag: &str) -> Locale {
    tags.iter()
        .filter_map(|tag| tag.parse::<Locale>().ok())
        .find(has_icu_data)
        .or_else(|| message_tag.parse().ok())
        .unwrap_or(icu_locale_core::locale!("en-US"))
}

/// Whether the compiled ICU data holds `locale`'s language. Compiled data
/// falls back inside `load` and reports the locale it found; `und` means
/// only the root answered. A language whose data equals the root's is stored
/// as `und` for that marker, so one probe is not enough: the language counts
/// as known when its list patterns, Gregorian date patterns or decimal
/// symbols resolve to anything but `und`. It is the provider's own lookup,
/// so it cannot disagree with the formatters.
fn has_icu_data(locale: &Locale) -> bool {
    let asked = DataLocale::from(locale);
    let lists = resolves::<ListAndV1>(
        &icu_list::provider::Baked,
        ListFormatterPatterns::WIDE,
        &asked,
    );
    let dates = resolves::<DatetimePatternsDateGregorianV1>(
        &icu_datetime::provider::Baked,
        DataMarkerAttributes::from_str_or_panic("ym0d"),
        &asked,
    );
    let numbers = resolves::<DecimalSymbolsV1>(
        &icu_decimal::provider::Baked,
        DataMarkerAttributes::empty(),
        &asked,
    );
    lists || dates || numbers
}

/// Whether `provider` answers `asked` for marker `M` with something other
/// than the root.
fn resolves<M: DataMarker>(
    provider: &impl DataProvider<M>,
    attributes: &DataMarkerAttributes,
    asked: &DataLocale,
) -> bool {
    let request = DataRequest {
        id: DataIdentifierBorrowed::for_marker_attributes_and_locale(attributes, asked),
        ..Default::default()
    };
    provider.load(request).is_ok_and(|found| {
        // No resolved locale means the asked one matched exactly.
        !found.metadata.locale.unwrap_or(*asked).is_unknown()
    })
}

/// Whether a request fell back to `en-US` with no catalog in its language:
/// `tr-TR` did, `en-GB` did not (it is English), and a tag that does not
/// parse counts as lost. Decided from the negotiation result, so case and
/// script variants of an available locale are not lost.
pub(crate) fn is_dropped(requested: &[LanguageIdentifier], resolved: &str) -> bool {
    resolved == EN_US && requested.first().is_none_or(|first| first.language != "en")
}
