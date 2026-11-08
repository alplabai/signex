//! The locale in force: its bundle, the `en-US` fallback and the language
//! list for Preferences.

use std::collections::HashSet;
use std::fmt;
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use icu_collator::CollatorBorrowed;
use icu_locale_core::Locale;

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
/// replaced, never edited. `pref` is what the user chose, kept even when this
/// build has no such locale: only the rendering falls back.
pub struct I18n {
    pref: LanguagePref,
    dropped: Option<String>,
    locale: &'static str,
    primary: Bundle,
    fallback: Bundle,
    options: Vec<LanguageOption>,
    // What was already logged, so a message rendered every frame is
    // reported once.
    reported: Mutex<HashSet<(Problem, &'static str)>>,
    // Where numbers, dates, lists, case and order follow: the language asked
    // for, which stays itself when the messages fall back to en-US.
    format_locale: Locale,
    pub(crate) bad_zones: Mutex<HashSet<String>>,
    pub(crate) collator: OnceLock<Option<CollatorBorrowed<'static>>>,
    #[cfg(test)]
    emitted: std::sync::atomic::AtomicUsize,
}

/// The kinds of catalog trouble worth one log line each.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Problem {
    FellBack,
    Missing,
    BadFormat,
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
    /// The locale `pref` resolves to. An explicit non-English request that
    /// fell back to en-US, or a tag that does not parse, is named by
    /// [`Self::dropped_locale`] for the caller to report. An English variant
    /// (`en`, `en-GB`) renders en-US unreported, and a case or script variant
    /// of an available language (`en-xa`, `en-Latn-XA`) is not reported
    /// either. `pref` stays as chosen, so saving Preferences does not erase it.
    pub fn new(pref: &LanguagePref) -> Self {
        let requested = locale::requested(pref);
        let tag = locale::negotiate(&requested);
        let dropped = match pref {
            LanguagePref::Locale(saved) if locale::is_dropped(&requested, tag) => {
                Some(saved.clone())
            }
            _ => None,
        };
        if let Some(first) = requested.first().filter(|l| l.to_string() != tag) {
            log::debug!("No {first} messages; using {tag}");
        }
        let format_locale = requested
            .first()
            .and_then(|l| l.to_string().parse().ok())
            .unwrap_or(icu_locale_core::locale!("en-US"));
        let primary = bundle::build(tag);
        let fallback = bundle::build(EN_US);
        let options = language_options(&primary, &fallback);
        Self {
            pref: pref.clone(),
            dropped,
            locale: tag,
            primary,
            fallback,
            options,
            reported: Mutex::default(),
            format_locale,
            bad_zones: Mutex::default(),
            collator: OnceLock::new(),
            #[cfg(test)]
            emitted: std::sync::atomic::AtomicUsize::new(0),
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

    /// The locale numbers, dates, lists, case and order follow: the first
    /// language requested (the OS list under System default), even when this
    /// build has no messages in it; `en-US` when nothing parses.
    pub fn format_locale(&self) -> &Locale {
        &self.format_locale
    }

    /// A saved language this build no longer has.
    pub fn dropped_locale(&self) -> Option<&str> {
        self.dropped.as_deref()
    }

    /// The Language list: System default first, then each locale by code.
    pub fn language_options(&self) -> &[LanguageOption] {
        &self.options
    }

    /// The list row of `pref`. A saved tag the list does not show (the
    /// pseudolocale in a release build, a language this build lacks) still
    /// gets a row, so the picker shows the setting rather than a blank.
    pub fn option_for(&self, pref: &LanguagePref) -> LanguageOption {
        let listed = self.options.iter().find(|o| &o.pref == pref);
        listed.cloned().unwrap_or_else(|| LanguageOption {
            pref: pref.clone(),
            label: pref.saved_tag().map_or_else(String::new, |tag| {
                bundle::render(&self.fallback, &language_name_id(tag), &[])
                    .map_or_else(|| tag.to_owned(), |r| r.text)
            }),
        })
    }

    /// Render `id`. A message the locale lacks falls back to `en-US`; one
    /// `en-US` lacks too shows its id and is logged.
    pub fn message(&self, id: MessageId, args: &[(&str, Arg)]) -> Localized {
        if let Some(done) = bundle::render(&self.primary, id.as_str(), args) {
            return self.finish(id, done);
        }
        if self.locale != EN_US {
            self.report_once(Problem::FellBack, id, || {
                log::debug!("No {} message {}; using en-US", self.locale, id.as_str());
            });
            if let Some(done) = bundle::render(&self.fallback, id.as_str(), args) {
                return self.finish(id, done);
            }
        }
        self.report_once(Problem::Missing, id, || {
            log::warn!("Message {} is missing from the catalog", id.as_str());
        });
        Localized::new(id.as_str().to_owned())
    }

    fn finish(&self, id: MessageId, done: bundle::Rendered) -> Localized {
        if let Some(problems) = &done.problems {
            self.report_once(Problem::BadFormat, id, || {
                log::warn!("Message {} did not format cleanly: {problems}", id.as_str());
            });
        }
        Localized::new(done.text)
    }

    /// Run `log` the first time `problem` is seen for `id`; a view calls
    /// `message` every frame and the Messages panel does not dedupe.
    fn report_once(&self, problem: Problem, id: MessageId, log: impl FnOnce()) {
        let mut reported = self.reported.lock().unwrap_or_else(PoisonError::into_inner);
        if reported.insert((problem, id.as_str())) {
            log();
            #[cfg(test)]
            self.emitted
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// How many log lines this instance has emitted (tests).
    #[cfg(test)]
    pub(crate) fn emitted_for_test(&self) -> usize {
        self.emitted.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many ids fell back or are missing; a format problem (a message
    /// rendered without its arguments) is not a lookup problem (tests).
    #[cfg(test)]
    pub(crate) fn lookup_problems_for_test(&self) -> usize {
        let reported = self.reported.lock().unwrap_or_else(PoisonError::into_inner);
        reported
            .iter()
            .filter(|(problem, _)| *problem != Problem::BadFormat)
            .count()
    }

    /// An en-US instance over explicit sources (tests).
    #[cfg(test)]
    pub(crate) fn en_us_from_sources_for_test(sources: &[&str]) -> Self {
        let mut i18n = Self::new(&LanguagePref::Locale(EN_US.to_owned()));
        i18n.primary = bundle::build_from(EN_US, sources);
        i18n.fallback = bundle::build_from(EN_US, sources);
        i18n
    }
}

fn language_name_id(tag: &str) -> String {
    format!("language-name-{}", tag.to_lowercase())
}

fn language_options(current: &Bundle, fallback: &Bundle) -> Vec<LanguageOption> {
    let system_id = ids::LANGUAGE_SYSTEM_DEFAULT.as_str();
    let system = bundle::render(current, system_id, &[])
        .or_else(|| bundle::render(fallback, system_id, &[]))
        .map_or_else(|| system_id.to_owned(), |r| r.text);
    let named = catalog::listed().into_iter().map(|tag| LanguageOption {
        pref: LanguagePref::Locale(tag.to_owned()),
        label: bundle::render(fallback, &language_name_id(tag), &[])
            .map_or_else(|| tag.to_owned(), |r| r.text),
    });
    std::iter::once(LanguageOption {
        pref: LanguagePref::System,
        label: system,
    })
    .chain(named)
    .collect()
}
