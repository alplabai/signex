//! #152 — Refresh All Pricing stores prices in the library and the open
//! library shows them.
//!
//! The background walk runs off `update`; this drives its two ends with a
//! fake distributor client and a really mounted library: the job writes the
//! cached price through the adapter, and the finished message reloads the
//! open library's caches and reports to the Messages panel.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signex_app::app::{Message, Signex};
use signex_app::diagnostics;
use signex_app::library::messages::LibraryMessage;
use signex_app::library::pricing::run_job_with;
use signex_library::ComponentRow;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{DatasheetRef, PlmReserved};
use signex_library::distributor::{
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource, PriceBreak,
    PricingSnapshot,
};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::{DistributorListing, ManufacturerPart};
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

struct Lcsc;

impl DistributorAdapter for Lcsc {
    fn name(&self) -> &'static str {
        "LCSC"
    }
    fn source(&self) -> DistributorSource {
        DistributorSource::Lcsc
    }
    fn lookup_by_url(&self, _: &url::Url) -> Result<Option<DistributorPart>, DistributorError> {
        Ok(None)
    }
    fn lookup_by_mpn(&self, _: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        Ok(Vec::new())
    }
    fn supports_pricing(&self) -> bool {
        true
    }
    fn refresh_pricing(&self, _: &DistributorPart) -> Result<PricingSnapshot, DistributorError> {
        Ok(PricingSnapshot {
            captured_at: chrono::Utc::now(),
            currency: "USD".into(),
            stock: Some(1200),
            part_number: None,
            by_distributor: BTreeMap::from([(
                "LCSC".to_string(),
                vec![PriceBreak {
                    qty: 1,
                    unit_price: 0.0123,
                }],
            )]),
        })
    }
}

fn init_library(dir: &std::path::Path) -> (PathBuf, Uuid) {
    let snxlib = dir.join("priced.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        extra: Default::default(),
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "priced".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(&snxlib, manifest, LibraryInitOptions::default()).expect("init library");
    (snxlib, library_id)
}

fn row(library_id: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R-10K"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", "RC0603FR-0710KL"),
        alternates: Vec::new(),
        supply: vec![DistributorListing::new("LCSC", "C25804")],
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
        server_revision: None,
    }
}

#[test]
fn refreshed_prices_are_saved_in_the_library_and_shown_by_the_open_library() {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::Builder::new()
        .prefix("signex-152-pricing-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, library_id) = init_library(tmp.path());
    let seeded = row(library_id);
    let row_id = seeded.row_id;
    LocalGitAdapter::open(&snxlib)
        .expect("open")
        .insert_row("Resistors", seeded, "seed")
        .expect("seed row");

    let (mut app, _boot) = Signex::new();
    app.library.open_libraries.clear();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    assert!(
        app.library
            .library_at(&snxlib)
            .expect("open")
            .pricing
            .is_empty(),
        "no price before the refresh"
    );

    // The blocking half, as `spawn_blocking` would run it.
    let report = run_job_with(
        &snxlib,
        &mut |_| Ok(Arc::new(Lcsc) as Arc<dyn DistributorAdapter>),
        &AtomicBool::new(false),
    )
    .expect("job runs");
    assert_eq!(report.store.stored, 1);

    // The message half, back on the UI thread.
    let before = diagnostics::newest_entry_id();
    let _ = app.update(Message::Library(LibraryMessage::PricingRefreshFinished {
        library_path: snxlib.clone(),
        outcome: Arc::new(Ok(report)),
    }));

    let cached = &app.library.library_at(&snxlib).expect("open").pricing[&row_id]["LCSC"]["C25804"];
    assert_eq!(cached.sku, "C25804");
    assert_eq!(cached.stock, Some(1200));
    assert_eq!(cached.headline_break().unwrap().unit_price, 0.0123);
    assert!(
        diagnostics::recent_entries()
            .iter()
            .filter(|e| before.is_none_or(|id| e.id > id))
            .any(|e| e
                .message
                .contains("priced: Pricing refresh: 1 updated, 0 failed (1 rows)")),
        "the summary reaches the Messages panel"
    );

    // And it is in the file: a fresh mount sees it with no refresh.
    let reopened = LocalGitAdapter::open(&snxlib).expect("reopen");
    assert!(
        reopened
            .table_pricing("Resistors")
            .unwrap()
            .contains_key(&row_id)
    );
}
