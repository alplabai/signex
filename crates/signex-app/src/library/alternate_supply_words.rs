//! The words for a manufacturer alternate's stock and lifecycle (#3046),
//! shared by the Alternate part picker and the editor's Supply tab: the
//! lifecycle label and its tone, and the line that says which value is in
//! effect and when a fetched one was taken. Pure.

use chrono::{DateTime, Utc};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_library::{AlternateSupply, ManufacturerLifecycle, SupplySource};

use crate::styles::PillTone;

/// The label and tone of a maker's lifecycle; `None` reads "Unknown".
pub fn lifecycle_words(lifecycle: Option<ManufacturerLifecycle>) -> (MessageId, PillTone) {
    match lifecycle {
        Some(ManufacturerLifecycle::Active) => (ids::LIBRARY_MFR_LIFECYCLE_ACTIVE, PillTone::Ok),
        Some(ManufacturerLifecycle::Nrnd) => (ids::LIBRARY_MFR_LIFECYCLE_NRND, PillTone::Warn),
        Some(ManufacturerLifecycle::Eol) => (ids::LIBRARY_MFR_LIFECYCLE_EOL, PillTone::Err),
        _ => (ids::LIBRARY_MFR_LIFECYCLE_UNKNOWN, PillTone::Neutral),
    }
}

/// A fetch time as the supply lines show it: `2026-11-26 09:40 UTC`.
pub fn when(at: DateTime<Utc>) -> String {
    at.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// Which of an alternate's two values a line is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyValue {
    Stock,
    Lifecycle,
}

/// Where `which` of `supply` comes from: entered by hand (with the fetched
/// value it hides, if any), fetched from a distributor at a time, or not
/// known yet.
pub fn source_line(supply: &AlternateSupply, which: SupplyValue, i18n: &I18n) -> Localized {
    let (source, hidden) = match which {
        SupplyValue::Stock => (
            supply.stock_source(),
            supply.manual_stock.and(
                supply
                    .fetched_stock
                    .as_ref()
                    .map(|f| (f.value.to_string(), f.distributor.clone(), f.fetched_at)),
            ),
        ),
        SupplyValue::Lifecycle => (
            supply.lifecycle_source(),
            supply
                .manual_lifecycle
                .and(supply.fetched_lifecycle.as_ref().map(|f| {
                    let label = t!(i18n, lifecycle_words(Some(f.value)).0).into_string();
                    (label, f.distributor.clone(), f.fetched_at)
                })),
        ),
    };
    match (source, hidden) {
        (None, _) => t!(i18n, ids::LIBRARY_SUPPLY_NONE),
        (Some(SupplySource::Manual), Some((value, distributor, at))) => t!(
            i18n,
            ids::LIBRARY_SUPPLY_MANUAL_OVER_FETCHED,
            "value" => value,
            "distributor" => distributor,
            "when" => when(at)
        ),
        (Some(SupplySource::Manual), None) => t!(i18n, ids::LIBRARY_SUPPLY_MANUAL),
        (
            Some(SupplySource::Fetched {
                distributor,
                fetched_at,
            }),
            _,
        ) => t!(
            i18n,
            ids::LIBRARY_SUPPLY_FETCHED,
            "distributor" => distributor,
            "when" => when(fetched_at)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use signex_library::Fetched;

    fn english(line: Localized) -> String {
        line.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    fn fetched<T>(value: T) -> Option<Fetched<T>> {
        Some(Fetched {
            value,
            distributor: "Mouser".into(),
            fetched_at: Utc.with_ymd_and_hms(2026, 11, 26, 9, 40, 0).unwrap(),
        })
    }

    #[test]
    fn the_line_says_which_value_is_in_effect_and_when_it_was_fetched() {
        let i18n = I18n::en_us();
        let none = AlternateSupply::default();
        assert_eq!(
            english(source_line(&none, SupplyValue::Stock, &i18n)),
            "No value yet: Refresh All Pricing asks the distributors the row lists"
        );
        let from_mouser = AlternateSupply {
            fetched_stock: fetched(86_400),
            ..AlternateSupply::default()
        };
        assert_eq!(
            english(source_line(&from_mouser, SupplyValue::Stock, &i18n)),
            "From Mouser, 2026-11-26 09:40 UTC"
        );
        let by_hand = AlternateSupply {
            manual_lifecycle: Some(ManufacturerLifecycle::Eol),
            fetched_lifecycle: fetched(ManufacturerLifecycle::Active),
            ..AlternateSupply::default()
        };
        assert_eq!(
            english(source_line(&by_hand, SupplyValue::Lifecycle, &i18n)),
            "Entered by hand in the library; Mouser reported Active on 2026-11-26 09:40 UTC"
        );
        let only_by_hand = AlternateSupply {
            manual_stock: Some(5),
            ..AlternateSupply::default()
        };
        assert_eq!(
            english(source_line(&only_by_hand, SupplyValue::Stock, &i18n)),
            "Entered by hand in the library"
        );
    }

    #[test]
    fn lifecycle_tones_follow_the_mockup() {
        use ManufacturerLifecycle::*;
        assert_eq!(lifecycle_words(Some(Active)).1, PillTone::Ok);
        assert_eq!(lifecycle_words(Some(Nrnd)).1, PillTone::Warn);
        assert_eq!(lifecycle_words(Some(Eol)).1, PillTone::Err);
        assert_eq!(lifecycle_words(None).1, PillTone::Neutral);
    }
}
