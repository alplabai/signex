//! What a distributor refresh fetched for a row's manufacturer alternates
//! (#3046): units in stock and the maker's lifecycle, per distributor and
//! alternate MPN.
//!
//! A cache, like the cached prices (`pricing.rs`): kept in the `.snxlib`
//! sidecar `[tables.<name>.alternate_supply.<row_id>.<distributor>.<mpn>]`
//! (`docs/LIBRARY_FORMAT.md` 2.6) and fetched again by Refresh All Pricing.
//! The librarian's own values live on the alternate
//! ([`ManufacturerPart::manual_stock`], [`ManufacturerPart::manual_lifecycle`]);
//! a refresh never writes them, and [`AlternateSupply`] says which value is
//! in effect.

use std::collections::BTreeMap;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::component::ComponentRow;
use crate::manufacturer::{ManufacturerLifecycle, ManufacturerPart};
use crate::pricing::{distributor_key, same_part_number};

/// What one distributor reported for one alternate MPN.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchedSupply {
    /// Units in stock, when the distributor reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<u32>,
    /// The maker's lifecycle, when the distributor reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ManufacturerLifecycle>,
    /// When the distributor was asked (UTC).
    pub fetched_at: DateTime<Utc>,
}

impl FetchedSupply {
    /// RFC 3339 form written to the `.snxlib`; round-trips exactly.
    pub(crate) fn fetched_at_text(&self) -> String {
        self.fetched_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    }

    /// Whether the record says anything: a stock or a lifecycle.
    pub fn is_empty(&self) -> bool {
        self.stock.is_none() && self.lifecycle.is_none()
    }
}

/// The fetched supply of one row's alternates: distributor name, then the
/// alternate's MPN.
pub type RowAlternateSupply = BTreeMap<String, BTreeMap<String, FetchedSupply>>;

/// One fetched record to write back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternateSupplyUpdate {
    pub table: String,
    pub row_id: Uuid,
    /// Key in the row's [`RowAlternateSupply`]; the client's own name.
    pub distributor: String,
    /// The alternate's MPN, as the row lists it.
    pub mpn: String,
    pub fetched: FetchedSupply,
}

/// Whether a record of `distributor` for `mpn` still describes `row`: the
/// row lists that distributor and an alternate with that MPN.
pub fn record_is_current(row: &ComponentRow, distributor: &str, mpn: &str) -> bool {
    let wanted = distributor_key(distributor);
    !mpn.trim().is_empty()
        && row
            .supply
            .iter()
            .any(|l| distributor_key(&l.distributor) == wanted)
        && row.alternates.iter().any(|a| same_part_number(&a.mpn, mpn))
}

/// Drop the records of `cached` that no longer describe `row`; returns
/// whether anything was removed.
pub fn prune_row_alternate_supply(row: &ComponentRow, cached: &mut RowAlternateSupply) -> bool {
    let before: usize = cached.values().map(BTreeMap::len).sum();
    for (distributor, per_mpn) in cached.iter_mut() {
        per_mpn.retain(|mpn, _| record_is_current(row, distributor, mpn));
    }
    cached.retain(|_, per_mpn| !per_mpn.is_empty());
    let after: usize = cached.values().map(BTreeMap::len).sum();
    after != before
}

/// A fetched value, the distributor that reported it and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched<T> {
    pub value: T,
    pub distributor: String,
    pub fetched_at: DateTime<Utc>,
}

/// Where the value in effect comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupplySource {
    /// Entered by hand in the editor.
    Manual,
    /// Fetched from `distributor` at `fetched_at`.
    Fetched {
        distributor: String,
        fetched_at: DateTime<Utc>,
    },
}

/// One alternate's stock and lifecycle: the hand-entered values and the
/// fetched ones side by side. A hand-entered value is in effect when set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlternateSupply {
    pub manual_stock: Option<u32>,
    pub manual_lifecycle: Option<ManufacturerLifecycle>,
    /// The largest stock any distributor reported (the newest on a tie).
    pub fetched_stock: Option<Fetched<u32>>,
    /// The lifecycle of the newest record that reports one.
    pub fetched_lifecycle: Option<Fetched<ManufacturerLifecycle>>,
}

impl AlternateSupply {
    /// The supply of `alternate`, from its own values and its row's fetched
    /// records (`fetched`, `None` when the row has none).
    pub fn of(alternate: &ManufacturerPart, fetched: Option<&RowAlternateSupply>) -> Self {
        let records = || {
            fetched
                .into_iter()
                .flatten()
                .flat_map(|(distributor, per_mpn)| {
                    per_mpn
                        .iter()
                        .filter(move |(mpn, _)| same_part_number(mpn, &alternate.mpn))
                        .map(move |(_, record)| (distributor, record))
                })
        };
        let fetched_stock = records()
            .filter_map(|(d, r)| r.stock.map(|stock| (stock, d, r.fetched_at)))
            .max_by_key(|(stock, _, at)| (*stock, *at))
            .map(|(value, distributor, fetched_at)| Fetched {
                value,
                distributor: distributor.clone(),
                fetched_at,
            });
        let fetched_lifecycle = records()
            .filter_map(|(d, r)| r.lifecycle.map(|lifecycle| (lifecycle, d, r.fetched_at)))
            .max_by_key(|(_, _, at)| *at)
            .map(|(value, distributor, fetched_at)| Fetched {
                value,
                distributor: distributor.clone(),
                fetched_at,
            });
        Self {
            manual_stock: alternate.manual_stock,
            manual_lifecycle: alternate.manual_lifecycle,
            fetched_stock,
            fetched_lifecycle,
        }
    }

    /// The stock in effect.
    pub fn stock(&self) -> Option<u32> {
        self.manual_stock
            .or_else(|| self.fetched_stock.as_ref().map(|f| f.value))
    }

    /// The lifecycle in effect.
    pub fn lifecycle(&self) -> Option<ManufacturerLifecycle> {
        self.manual_lifecycle
            .or_else(|| self.fetched_lifecycle.as_ref().map(|f| f.value))
    }

    /// Where [`Self::stock`] comes from; `None` when there is none.
    pub fn stock_source(&self) -> Option<SupplySource> {
        source(self.manual_stock.is_some(), self.fetched_stock.as_ref())
    }

    /// Where [`Self::lifecycle`] comes from; `None` when there is none.
    pub fn lifecycle_source(&self) -> Option<SupplySource> {
        source(
            self.manual_lifecycle.is_some(),
            self.fetched_lifecycle.as_ref(),
        )
    }
}

fn source<T>(manual: bool, fetched: Option<&Fetched<T>>) -> Option<SupplySource> {
    if manual {
        return Some(SupplySource::Manual);
    }
    fetched.map(|f| SupplySource::Fetched {
        distributor: f.distributor.clone(),
        fetched_at: f.fetched_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manufacturer::{AlternateStatus, DistributorListing};
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, day, 10, 0, 0).unwrap()
    }

    fn record(
        stock: Option<u32>,
        lifecycle: Option<ManufacturerLifecycle>,
        day: u32,
    ) -> FetchedSupply {
        FetchedSupply {
            stock,
            lifecycle,
            fetched_at: at(day),
        }
    }

    fn row() -> ComponentRow {
        use crate::component::{DatasheetRef, PlmReserved};
        use crate::identity::{ComponentClass, InternalPn};
        use crate::lifecycle::LifecycleState;
        use crate::param::ParamMap;
        use crate::primitive::PrimitiveRef;
        ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::nil(),
            internal_pn: InternalPn::new("R"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Example Corp", "PRIMARY-1"),
            alternates: vec![ManufacturerPart {
                status: AlternateStatus::Approved,
                ..ManufacturerPart::draft("Sample Inc", "ALT-1")
            }],
            supply: vec![
                DistributorListing::new("DigiKey", "DK-1"),
                DistributorListing::new("Mouser", ""),
            ],
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: at(1),
            updated: at(1),
            content_hash: [0; 32],
            server_revision: None,
        }
    }

    fn cache(entries: &[(&str, &str, FetchedSupply)]) -> RowAlternateSupply {
        let mut out = RowAlternateSupply::new();
        for (distributor, mpn, rec) in entries {
            out.entry(distributor.to_string())
                .or_default()
                .insert(mpn.to_string(), rec.clone());
        }
        out
    }

    #[test]
    fn fetched_values_take_the_largest_stock_and_the_newest_lifecycle() {
        use ManufacturerLifecycle::*;
        let cached = cache(&[
            ("DigiKey", "ALT-1", record(Some(500), Some(Active), 1)),
            ("Mouser", "alt-1", record(Some(1200), Some(Nrnd), 2)),
            ("Mouser", "OTHER", record(Some(9999), Some(Eol), 3)),
        ]);
        let supply = AlternateSupply::of(&row().alternates[0], Some(&cached));
        assert_eq!(supply.stock(), Some(1200));
        assert_eq!(supply.lifecycle(), Some(Nrnd));
        assert_eq!(
            supply.stock_source(),
            Some(SupplySource::Fetched {
                distributor: "Mouser".into(),
                fetched_at: at(2)
            })
        );
    }

    #[test]
    fn a_manual_value_wins_and_the_fetched_one_is_kept_beside_it() {
        let mut alternate = row().alternates[0].clone();
        alternate.manual_stock = Some(10);
        let cached = cache(&[(
            "DigiKey",
            "ALT-1",
            record(Some(500), Some(ManufacturerLifecycle::Active), 1),
        )]);
        let supply = AlternateSupply::of(&alternate, Some(&cached));
        assert_eq!(supply.stock(), Some(10));
        assert_eq!(supply.stock_source(), Some(SupplySource::Manual));
        assert_eq!(supply.fetched_stock.as_ref().map(|f| f.value), Some(500));
        // The lifecycle has no manual value: the fetched one is in effect.
        assert!(matches!(
            supply.lifecycle_source(),
            Some(SupplySource::Fetched { .. })
        ));
    }

    #[test]
    fn nothing_known_is_none() {
        let supply = AlternateSupply::of(&row().alternates[0], None);
        assert_eq!(supply, AlternateSupply::default());
        assert!(supply.stock_source().is_none() && supply.lifecycle_source().is_none());
    }

    #[test]
    fn prune_keeps_only_listed_distributors_and_alternates() {
        let row = row();
        let mut cached = cache(&[
            ("DigiKey", "ALT-1", record(Some(1), None, 1)),
            ("Mouser", "ALT-1", record(Some(2), None, 1)),
            ("LCSC", "ALT-1", record(Some(3), None, 1)),
            ("DigiKey", "GONE", record(Some(4), None, 1)),
        ]);
        assert!(prune_row_alternate_supply(&row, &mut cached));
        assert_eq!(
            cached.keys().map(String::as_str).collect::<Vec<_>>(),
            ["DigiKey", "Mouser"]
        );
        assert_eq!(cached["DigiKey"].len(), 1);
        assert!(!prune_row_alternate_supply(&row, &mut cached));
        assert!(!record_is_current(&row, "DigiKey", " "));
    }
}
