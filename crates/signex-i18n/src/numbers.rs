//! Number formatting through ICU4X `icu_decimal`.
//!
//! Fluent's own number formatting ignores the locale (fluent-rs PR #377), so
//! the bundle's formatter hook routes every number here. `DecimalFormatter`
//! is not `Send`, which a concurrent bundle's memoizer needs; so the
//! memoizer keeps the bundle's locale and the formatter is built per number,
//! cheap with compiled data.

use fluent_bundle::FluentValue;
use fluent_bundle::memoizer::MemoizerKind;
use icu_decimal::DecimalFormatter;
use icu_decimal::input::{Decimal, FloatPrecision};
use icu_locale_core::Locale;
use intl_memoizer::Memoizable;
use unic_langid::LanguageIdentifier;

/// The bundle's language as an ICU locale.
pub(crate) struct BundleLocale(Locale);

impl Memoizable for BundleLocale {
    type Args = ();
    type Error = String;

    fn construct(lang: LanguageIdentifier, _: ()) -> Result<Self, String> {
        lang.to_string()
            .parse()
            .map(Self)
            .map_err(|e| format!("{e}"))
    }
}

/// `FluentBundle::set_formatter` hook: numbers through ICU, everything else
/// left to Fluent (`None`).
pub(crate) fn format_value<M: MemoizerKind>(value: &FluentValue, memoizer: &M) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    memoizer
        .with_try_get_threadsafe::<BundleLocale, _, _>((), |locale| {
            format_decimal(&locale.0, number.value)
        })
        .ok()
        .flatten()
}

/// `value` in `locale`'s digits, grouping and decimal separator; `None`
/// for NaN and infinities.
pub(crate) fn format_decimal(locale: &Locale, value: f64) -> Option<String> {
    let decimal = Decimal::try_from_f64(value, FloatPrecision::RoundTrip).ok()?;
    let formatter = DecimalFormatter::try_new(locale.into(), Default::default()).ok()?;
    Some(formatter.format(&decimal).to_string())
}
