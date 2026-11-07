//! The locale in force: its bundle, the `en-US` fallback and the language
//! list for Preferences.

use std::collections::HashSet;
use std::fmt;
use std::sync::{LazyLock, Mutex, PoisonError};

use crate::bundle::{self, Bundle};
use crate::catalog::{self, EN_US};
use crate::ids;
use crate::locale::{self, LanguagePref};
use crate::types::{Arg, Localized, MessageId};

/// One row of the Language list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageOption {
    pub pref: LanguagePref,
    /// "System default", or the language's own name.
    pub label: String,
}

impl fmt::Display for LanguageOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The active locale. Built once per language change and shared by `Arc`;
/// replaced, never edited.
pub struct I18n {
    pref: LanguagePref,
    dropped: Option<String>,
    locale: &'static str,
    primary: Bundle,
    fallback: Bundle,
    options: Vec<LanguageOption>,
    // Ids already reported as falling back, so the log gets each once.
    reported: Mutex<HashSet<&'static str>>,
}

impl fmt::Debug for I18n {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("I18n")
            .field("pref", &self.pref)
            .field("locale", &self.locale)
            .finish()
    }
}

static EN_US_SHARED: LazyLock<std::sync::Arc<I18n>> =
    LazyLock::new(|| std::sync::Arc::new(I18n::new(&LanguagePref::Locale(EN_US.to_owned()))));

impl I18n {
    /// The locale `pref` resolves to. An explicit locale this build does not
    /// have becomes `System` and is kept in [`Self::dropped_locale`] for the
    /// caller to report.
    pub fn new(pref: &LanguagePref) -> Self {
        let (pref, dropped) = match pref {
            LanguagePref::Locale(tag) if !locale::is_available(tag) => {
                (LanguagePref::System, Some(tag.clone()))
            }
            other => (other.clone(), None),
        };
        let requested = locale::requested(&pref);
        let tag = locale::negotiate(&requested);
        if let Some(first) = requested.first().filter(|l| l.to_string() != tag) {
            log::debug!("No {first} messages; using {tag}");
        }
        let primary = bundle::build(tag);
        let fallback = bundle::build(EN_US);
        let options = language_options(&primary, &fallback);
        Self {
            pref,
            dropped,
            locale: tag,
            primary,
            fallback,
            options,
            reported: Mutex::default(),
        }
    }

    /// A shared `en-US` instance, for contexts built without app state.
    pub fn en_us() -> std::sync::Arc<Self> {
        std::sync::Arc::clone(&EN_US_SHARED)
    }

    /// The setting this instance stands for.
    pub fn pref(&self) -> &LanguagePref {
        &self.pref
    }

    /// The BCP 47 tag of the locale in force.
    pub fn locale(&self) -> &'static str {
        self.locale
    }

    /// A saved language this build no longer has.
    pub fn dropped_locale(&self) -> Option<&str> {
        self.dropped.as_deref()
    }

    /// The Language list: System default first, then each locale by code.
    pub fn language_options(&self) -> &[LanguageOption] {
        &self.options
    }

    /// The list row of `pref`.
    pub fn option_for(&self, pref: &LanguagePref) -> Option<LanguageOption> {
        self.options.iter().find(|o| &o.pref == pref).cloned()
    }

    /// Render `id`. A message the locale lacks falls back to `en-US`; one
    /// `en-US` lacks too shows its id and is logged.
    pub fn message(&self, id: MessageId, args: &[(&str, Arg)]) -> Localized {
        if let Some(text) = bundle::render(&self.primary, id.as_str(), args) {
            return Localized::new(text);
        }
        if self.locale != EN_US {
            self.report_fallback_once(id);
            if let Some(text) = bundle::render(&self.fallback, id.as_str(), args) {
                return Localized::new(text);
            }
        }
        log::warn!("Message {} is missing from the catalog", id.as_str());
        Localized::new(id.as_str().to_owned())
    }

    fn report_fallback_once(&self, id: MessageId) {
        let mut reported = self.reported.lock().unwrap_or_else(PoisonError::into_inner);
        if reported.insert(id.as_str()) {
            log::debug!("No {} message {}; using en-US", self.locale, id.as_str());
        }
    }
}

fn language_options(current: &Bundle, fallback: &Bundle) -> Vec<LanguageOption> {
    let system_id = ids::LANGUAGE_SYSTEM_DEFAULT.as_str();
    let system = bundle::render(current, system_id, &[])
        .or_else(|| bundle::render(fallback, system_id, &[]))
        .unwrap_or_else(|| system_id.to_owned());
    let named = catalog::listed().into_iter().map(|tag| LanguageOption {
        pref: LanguagePref::Locale(tag.to_owned()),
        label: bundle::render(
            fallback,
            &format!("language-name-{}", tag.to_lowercase()),
            &[],
        )
        .unwrap_or_else(|| tag.to_owned()),
    });
    std::iter::once(LanguageOption {
        pref: LanguagePref::System,
        label: system,
    })
    .chain(named)
    .collect()
}
