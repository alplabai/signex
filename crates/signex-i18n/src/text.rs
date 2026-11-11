//! Locale-aware case and ordering of text, for the screen only.

use std::cmp::Ordering;

use icu_casemap::CaseMapper;
use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorBorrowed};
use icu_locale_core::LanguageIdentifier;

use crate::i18n::I18n;

/// `text` folded under the rules of the language in `tag`.
pub(crate) fn fold_in(tag: &str, text: &str) -> String {
    let mapper = CaseMapper::new();
    if matches!(tag.split('-').next(), Some("tr" | "az")) {
        mapper.fold_turkic_string(text).into_owned()
    } else {
        mapper.fold_string(text).into_owned()
    }
}

/// `c` simple-folded (always one char) under the rules of the language in `tag`.
/// Outside `tr`/`az`, `İ` still folds to `i` so a plain `i` query keeps
/// finding it (docs/DECISIONS.md, #2063).
pub(crate) fn fold_char_in(tag: &str, c: char) -> char {
    let mapper = CaseMapper::new();
    if matches!(tag.split('-').next(), Some("tr" | "az")) {
        mapper.simple_fold_turkic(c)
    } else if c == '\u{130}' {
        'i'
    } else {
        mapper.simple_fold(c)
    }
}

/// `text` in capitals under the rules of the language in `tag`.
pub(crate) fn upper_in(tag: &str, text: &str) -> String {
    let lang = tag
        .parse::<LanguageIdentifier>()
        .unwrap_or(LanguageIdentifier::UNKNOWN);
    CaseMapper::new()
        .uppercase_to_string(text, &lang)
        .into_owned()
}

impl I18n {
    /// `text` case-folded for matching: `İ` and `I` become `i` and `ı` under
    /// `tr`/`az`, `ß` becomes `ss`, and under `en` it is plain lowercase.
    /// A search folds BOTH the query and the haystack with this one fold,
    /// then compares: `fold(query)` is a substring of `fold(haystack)`.
    /// The result is for comparing, never for showing.
    ///
    /// A full fold can change the length (`ß` becomes `ss`, `İ` becomes
    /// `i̇` under `en`), so a match position in the folded string does not
    /// map back to the original: do not use it to highlight. Under `en`,
    /// `istanbul` does not match `İstanbul`, which is correct Unicode
    /// folding (`İ` folds to `i` plus a combining dot).
    ///
    /// Case operations act on text, so they follow the MESSAGE locale
    /// ([`Self::locale`]), the language of the catalog text: under a `tr-TR`
    /// preference with no `tr` catalog the labels are English and fold as
    /// English. The language of user content is unknown, so user content is
    /// folded the same way.
    pub fn fold_for_search(&self, text: &str) -> String {
        fold_in(self.locale(), text)
    }

    /// `text` case-folded by the FORMAT locale's rules, as a key for
    /// [`Self::compare_for_display`], whose collator follows the same
    /// locale: under `tr-TR`, `IŞIK` folds to `ışık` and sorts in the `ı`
    /// group. For ordering only, never for matching or showing.
    pub fn fold_for_sort(&self, text: &str) -> String {
        fold_in(self.format_locale().id.language.as_str(), text)
    }

    /// `c` simple-folded to exactly one char, for matching and highlighting
    /// per char (`İ` and `I` become `i` and `ı` under `tr`/`az`). Unlike
    /// [`Self::fold_for_search`] it never changes the length (`ß` stays
    /// `ß`), so a match position maps back to the original text. Under every
    /// other language `İ` still folds to `i`, so a plain `i` finds it.
    /// Follows the message locale like [`Self::fold_for_search`].
    pub fn fold_char_for_search(&self, c: char) -> char {
        fold_char_in(self.locale(), c)
    }

    /// `text` in capitals for display: `i` becomes `İ` when the message
    /// language is `tr`/`az`. Follows the message locale like
    /// [`Self::fold_for_search`]. Never fold or compare with it.
    pub fn upper_for_display(&self, text: &str) -> String {
        upper_in(self.locale(), text)
    }

    /// The order of `a` and `b` in the formatting locale's alphabet (`ç`
    /// after `c` under `tr`). Collation, like numbers, dates and lists,
    /// follows [`Self::format_locale`]: it orders values, not catalog text.
    /// Digits sort as characters (`"10"` before `"9"`); there is no numeric
    /// collation.
    ///
    /// For on-screen lists only. Never a file, export, BOM, netlist or
    /// clipboard order: those must not change with the language.
    pub fn compare_for_display(&self, a: &str, b: &str) -> Ordering {
        match self.collator() {
            Some(collator) => collator.compare(a, b),
            None => a.cmp(b),
        }
    }

    /// Sort `items` by `key` in display order (stable); the same on-screen
    /// only rule as [`Self::compare_for_display`].
    pub fn sort_for_display<T>(&self, items: &mut [T], key: impl Fn(&T) -> &str) {
        items.sort_by(|a, b| self.compare_for_display(key(a), key(b)));
    }

    fn collator(&self) -> Option<&CollatorBorrowed<'static>> {
        self.collator
            .get_or_init(|| {
                Collator::try_new(self.format_locale().into(), CollatorOptions::default())
                    .inspect_err(|error| {
                        self.warn_format_once("collator", || {
                            log::warn!(
                                "No collation for {}: {error}; sorting by code point",
                                self.format_locale()
                            );
                        });
                    })
                    .ok()
            })
            .as_ref()
    }
}
