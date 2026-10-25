//! Refresh All Pricing: fetch with a fake distributor client, store through
//! the library adapter as one commit, report each failed row.

#![cfg(feature = "local-git")]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::distributor::{
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource, PriceBreak,
    PricingSnapshot,
};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::{DistributorListing, ManufacturerPart};
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library::{
    CachedPricing, ClientUnavailable, FailureKind, PricingUpdate, refresh_library_pricing,
};
use uuid::Uuid;

/// Answers by MPN: a price, or the scripted error.
struct FakeClient {
    name: &'static str,
    priced: bool,
    by_mpn: BTreeMap<&'static str, Result<f64, DistributorErrorKind>>,
    calls: std::sync::Mutex<Vec<String>>,
}

#[derive(Clone, Copy)]
enum DistributorErrorKind {
    Network,
    RateLimited,
}

impl DistributorAdapter for FakeClient {
    fn name(&self) -> &'static str {
        self.name
    }
    fn source(&self) -> DistributorSource {
        DistributorSource::DigiKey
    }
    fn lookup_by_url(&self, _: &url::Url) -> Result<Option<DistributorPart>, DistributorError> {
        Ok(None)
    }
    fn lookup_by_mpn(&self, _: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        Ok(Vec::new())
    }
    fn supports_pricing(&self) -> bool {
        self.priced
    }
    fn refresh_pricing(&self, part: &DistributorPart) -> Result<PricingSnapshot, DistributorError> {
        self.calls.lock().unwrap().push(part.mpn.clone());
        match self.by_mpn.get(part.mpn.as_str()) {
            Some(Ok(price)) => Ok(PricingSnapshot {
                captured_at: chrono::Utc::now(),
                currency: "USD".into(),
                stock: Some(42),
                by_distributor: BTreeMap::from([(
                    self.name.to_string(),
                    vec![
                        PriceBreak {
                            qty: 1,
                            unit_price: *price,
                        },
                        PriceBreak {
                            qty: 100,
                            unit_price: price / 2.0,
                        },
                    ],
                )]),
            }),
            Some(Err(DistributorErrorKind::Network)) => {
                Err(DistributorError::Network("connection reset".into()))
            }
            Some(Err(DistributorErrorKind::RateLimited)) => Err(DistributorError::RateLimited {
                retry_after_seconds: 30,
            }),
            None => Err(DistributorError::NotFound),
        }
    }
}

fn fake(
    name: &'static str,
    answers: &[(&'static str, Result<f64, DistributorErrorKind>)],
) -> Arc<FakeClient> {
    Arc::new(FakeClient {
        name,
        priced: true,
        by_mpn: answers.iter().cloned().collect(),
        calls: Default::default(),
    })
}

fn library(dir: &Path) -> (LocalGitAdapter, Uuid) {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Px").join("Px.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Px".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .unwrap();
    (adapter, library_id)
}

fn row(pn: &str, lib: Uuid, listings: &[(&str, &str)]) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: listings
            .iter()
            .map(|(d, s)| DistributorListing::new(*d, *s))
            .collect(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

fn commit_count(adapter: &LocalGitAdapter) -> usize {
    let repo = git2::Repository::open(adapter.root()).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.count()
}

fn seeded() -> (
    tempfile::TempDir,
    LocalGitAdapter,
    BTreeMap<&'static str, Uuid>,
) {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = library(dir.path());
    let rows = [
        row("R-OK", lib, &[("DigiKey", "DK-1")]),
        row("R-OK2", lib, &[("DigiKey", "DK-3")]),
        row("R-NET", lib, &[("DigiKey", "DK-2")]),
        row("R-NOSKU", lib, &[]),
        row("R-LCSC", lib, &[("LCSC", "C1")]),
        row("R-MOUSER", lib, &[("Mouser", "M1")]),
    ];
    let mut ids = BTreeMap::new();
    for r in rows {
        ids.insert(
            match r.internal_pn.as_str() {
                "R-OK" => "ok",
                "R-NET" => "net",
                "R-OK2" => "ok2",
                "R-NOSKU" => "nosku",
                "R-LCSC" => "lcsc",
                _ => "mouser",
            },
            r.row_id,
        );
        adapter.insert_row("R", r, "seed").unwrap();
    }
    (dir, adapter, ids)
}

#[test]
fn refresh_stores_successes_in_one_commit_and_names_every_failure() {
    let (_dir, adapter, ids) = seeded();
    let digikey = fake(
        "DigiKey",
        &[
            ("R-OK", Ok(0.1)),
            ("R-OK2", Ok(0.2)),
            ("R-NET", Err(DistributorErrorKind::Network)),
        ],
    );
    let before = commit_count(&adapter);

    let mut resolve = |name: &str| -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> {
        match name {
            "DigiKey" => Ok(digikey.clone()),
            "LCSC" => Err(ClientUnavailable::NotConfigured(
                "LCSC is not enabled".into(),
            )),
            _ => Err(ClientUnavailable::AuthMissing(
                "no Mouser API key in keyring".into(),
            )),
        }
    };
    let report = refresh_library_pricing(&adapter, &mut resolve, &AtomicBool::new(false)).unwrap();

    assert_eq!(report.store.stored, 2);
    assert_eq!(
        commit_count(&adapter),
        before + 1,
        "ONE commit for all rows"
    );
    let kinds: BTreeMap<String, FailureKind> = report
        .failures
        .iter()
        .map(|f| (f.internal_pn.clone(), f.kind))
        .collect();
    assert_eq!(
        kinds,
        BTreeMap::from([
            ("R-NET".to_string(), FailureKind::Network),
            ("R-NOSKU".to_string(), FailureKind::NoSku),
            ("R-LCSC".to_string(), FailureKind::NotConfigured),
            ("R-MOUSER".to_string(), FailureKind::Auth),
        ])
    );
    assert_eq!(
        report.summary(),
        "Pricing refresh: 2 updated, 4 failed (6 rows)"
    );

    let stored = adapter.table_pricing("R").unwrap();
    assert_eq!(stored.len(), 2, "only the successful rows have a price");
    let cached = &stored[&ids["ok"]]["DigiKey"];
    assert_eq!(cached.sku, "DK-1");
    assert_eq!(cached.currency, "USD");
    assert_eq!(cached.stock, Some(42));
    assert_eq!(cached.headline_break().unwrap().unit_price, 0.1);
}

#[test]
fn failed_refresh_keeps_the_previous_cached_price() {
    let (_dir, adapter, ids) = seeded();
    let good = fake("DigiKey", &[("R-NET", Ok(0.5))]);
    let mut resolve =
        |_: &str| -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> { Ok(good.clone()) };
    refresh_library_pricing(&adapter, &mut resolve, &AtomicBool::new(false)).unwrap();
    assert!(
        adapter
            .table_pricing("R")
            .unwrap()
            .contains_key(&ids["net"])
    );

    let broken = fake("DigiKey", &[("R-NET", Err(DistributorErrorKind::Network))]);
    let mut resolve =
        |_: &str| -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> { Ok(broken.clone()) };
    let report = refresh_library_pricing(&adapter, &mut resolve, &AtomicBool::new(false)).unwrap();
    assert_eq!(report.store.stored, 0);
    let kept = adapter.table_pricing("R").unwrap();
    assert_eq!(
        kept[&ids["net"]]["DigiKey"]
            .headline_break()
            .unwrap()
            .unit_price,
        0.5
    );
}

#[test]
fn rate_limit_ends_that_distributors_pass() {
    let dir = tempfile::tempdir().unwrap();
    let (adapter, lib) = library(dir.path());
    for pn in ["R-A", "R-B", "R-C"] {
        adapter
            .insert_row("R", row(pn, lib, &[("DigiKey", pn)]), "seed")
            .unwrap();
    }
    let client = fake(
        "DigiKey",
        &[("R-A", Err(DistributorErrorKind::RateLimited))],
    );
    let mut resolve =
        |_: &str| -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> { Ok(client.clone()) };
    let report = refresh_library_pricing(&adapter, &mut resolve, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        client.calls.lock().unwrap().len(),
        1,
        "no calls after the limit"
    );
    assert_eq!(report.failures.len(), 3);
    assert!(
        report
            .failures
            .iter()
            .all(|f| f.kind == FailureKind::RateLimited)
    );
}

#[test]
fn cancelled_refresh_fetches_nothing_and_says_so() {
    let (_dir, adapter, _ids) = seeded();
    let client = fake("DigiKey", &[("R-OK", Ok(0.1))]);
    let mut resolve =
        |_: &str| -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> { Ok(client.clone()) };
    let report = refresh_library_pricing(&adapter, &mut resolve, &AtomicBool::new(true)).unwrap();
    assert!(report.cancelled);
    assert!(client.calls.lock().unwrap().is_empty());
    assert!(report.summary().ends_with(", cancelled"));
    assert!(adapter.table_pricing("R").unwrap().is_empty());
}

#[test]
fn price_for_a_sku_changed_during_the_fetch_is_dropped() {
    let (_dir, adapter, ids) = seeded();
    // The fetch was for DK-1; meanwhile the user re-pointed the row at DK-9.
    let mut edited = adapter.read_row("R", RowId::from_uuid(ids["ok"])).unwrap();
    edited.supply = vec![DistributorListing::new("DigiKey", "DK-9")];
    adapter.update_row("R", edited, "edit").unwrap();

    let stale = PricingUpdate {
        table: "R".into(),
        row_id: ids["ok"],
        distributor: "DigiKey".into(),
        pricing: CachedPricing {
            sku: "DK-1".into(),
            currency: "USD".into(),
            stock: None,
            breaks: vec![PriceBreak {
                qty: 1,
                unit_price: 0.1,
            }],
            fetched_at: chrono::Utc::now(),
        },
    };
    let before = commit_count(&adapter);
    let outcome = adapter.store_pricing(vec![stale], "pricing").unwrap();
    assert_eq!(outcome.stored, 0);
    assert_eq!(outcome.dropped.len(), 1);
    assert!(adapter.table_pricing("R").unwrap().is_empty());
    assert_eq!(
        commit_count(&adapter),
        before,
        "nothing stored, nothing committed"
    );
}

#[test]
fn deleting_a_row_drops_its_cached_price() {
    let (_dir, adapter, ids) = seeded();
    let client = fake("DigiKey", &[("R-OK", Ok(0.1))]);
    let mut resolve =
        |_: &str| -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> { Ok(client.clone()) };
    refresh_library_pricing(&adapter, &mut resolve, &AtomicBool::new(false)).unwrap();
    assert_eq!(adapter.table_pricing("R").unwrap().len(), 1);
    adapter
        .delete_row("R", RowId::from_uuid(ids["ok"]), "delete")
        .unwrap();
    assert!(adapter.table_pricing("R").unwrap().is_empty());
}
