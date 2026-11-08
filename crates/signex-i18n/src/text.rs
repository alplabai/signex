//! Locale-aware case and ordering of text, for the screen only.

use std::cmp::Ordering;

use icu_casemap::CaseMapper;
use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorBorrowed};
use icu_locale_core::LanguageIdentifier;

use crate::i18n::I18n;

impl I18n {
    fn is_turkic(&self) -> bool {
        matches!(self.format_locale().id.language.as_str(), "tr" | "az")
    }

    /// `text` case-folded for matching: `İ` and `I` become `i` and `ı` under
    /// `tr`/`az`, `ß` becomes `ss`, and under `en` it is plain lowercase.
    /// A search folds BOTH the query and the haystack with this one fold,
    /// then compares: `fold(query)` is a substring of `fold(haystack)`.
    /// The result is for comparing, never for showing.
    pub fn fold_for_search(&self, text: &str) -> String {
        let mapper = CaseMapper::new();
        if self.is_turkic() {
            mapper.fold_turkic_string(text).into_owned()
        } else {
            mapper.fold_string(text).into_owned()
        }
    }

    /// `text` in capitals for display: `i` becomes `İ` under `tr`/`az`.
    /// Never fold or compare with it; use [`Self::fold_for_search`].
    pub fn upper_for_display(&self, text: &str) -> String {
        let lang: &LanguageIdentifier = &self.format_locale().id;
        CaseMapper::new()
            .uppercase_to_string(text, lang)
            .into_owned()
    }

    /// The order of `a` and `b` in the UI language's alphabet (`ç` after `c`
    /// under `tr`). Digits sort as characters (`"10"` before `"9"`); there is
    /// no numeric collation.
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
                Collator::try_new(self.format_locale().into(), CollatorOptions::default()).ok()
            })
            .as_ref()
    }
}
