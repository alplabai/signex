//! Refresh All Pricing: the pieces that sit between the menu row and
//! `signex_library::refresh_library_pricing`.
//!
//! Nothing here runs in `update`. [`run_job`] blocks on the network and the
//! library write lock, so the dispatcher hands it to `spawn_blocking`;
//! [`report_lines`] turns the outcome into the Messages-panel text.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signex_library::distributors::digikey::DigiKeyAuth;
use signex_library::distributors::{DigiKeyAdapter, JlcpcbAdapter, LcscAdapter, MouserAdapter};
use signex_library::{
    ClientResult, ClientUnavailable, DistributorAdapter, DistributorSource, LocalGitAdapter,
    PricingRefreshReport, distributor_key, refresh_library_pricing,
};

use crate::library::settings::digikey_oauth;

/// Redirect URI handed to `DigiKeyAuth` for the refresh-token grant. The
/// grant never redirects; the value only has to be a valid URL.
const DIGIKEY_REFRESH_REDIRECT: &str = "http://localhost";

/// The refresh being run: one at a time, so a distributor's rate limit is
/// shared by exactly one walker.
#[derive(Debug, Clone)]
pub struct PricingRefreshRun {
    pub library_path: PathBuf,
    /// Set to stop the walk before its next row.
    pub cancel: Arc<AtomicBool>,
}

/// The client for the distributor a listing names, or why there is none.
///
/// `enabled` is the Distributor APIs list; a distributor that is not on it is
/// off. Credentials come from the OS keyring (Mouser, DigiKey refresh token)
/// and the environment (DigiKey client id and secret); they are read by the
/// client when it sends a request and never reach a log line.
pub fn resolve_client(
    listing_name: &str,
    enabled: &[DistributorSource],
) -> Result<Arc<dyn DistributorAdapter>, ClientUnavailable> {
    let (source, label) = match distributor_key(listing_name).as_str() {
        "digikey" => (DistributorSource::DigiKey, "DigiKey"),
        "mouser" => (DistributorSource::Mouser, "Mouser"),
        "lcsc" => (DistributorSource::Lcsc, "LCSC"),
        "jlcpcb" => (DistributorSource::Jlcpcb, "JLCPCB"),
        _ => {
            return Err(ClientUnavailable::NotConfigured(format!(
                "no pricing client for distributor {listing_name:?}"
            )));
        }
    };
    if !enabled.contains(&source) {
        return Err(ClientUnavailable::NotConfigured(format!(
            "{label} is not in the distributor order (Distributor APIs settings)"
        )));
    }
    match source {
        DistributorSource::DigiKey => {
            let (id, secret) = digikey_oauth::read_env_credentials();
            if id.is_empty() {
                return Err(ClientUnavailable::AuthMissing(format!(
                    "DigiKey client id missing; set {}",
                    digikey_oauth::ENV_CLIENT_ID
                )));
            }
            let auth = DigiKeyAuth::new(id, secret, DIGIKEY_REFRESH_REDIRECT)
                .map_err(|e| ClientUnavailable::AuthMissing(format!("DigiKey: {e}")))?;
            Ok(Arc::new(DigiKeyAdapter::new(auth, None)))
        }
        DistributorSource::Mouser => MouserAdapter::from_keyring(None)
            .map(|a| Arc::new(a) as Arc<dyn DistributorAdapter>)
            .map_err(|e| ClientUnavailable::AuthMissing(format!("Mouser: {e}"))),
        DistributorSource::Lcsc => Ok(Arc::new(LcscAdapter::new(None))),
        DistributorSource::Jlcpcb => Ok(Arc::new(JlcpcbAdapter::new(None))),
        _ => unreachable!("resolve_client maps only the four sources above"),
    }
}

/// Fetch and store prices for the library at `library_path` with the real
/// clients. Opens its own adapter: the write lock and the reload-on-stale
/// path make that safe next to the mounted one.
pub fn run_job(
    library_path: &Path,
    enabled: &[DistributorSource],
    cancel: &AtomicBool,
) -> Result<PricingRefreshReport, String> {
    run_job_with(
        library_path,
        &mut |name| resolve_client(name, enabled),
        cancel,
    )
}

/// [`run_job`] with the client lookup supplied.
pub fn run_job_with(
    library_path: &Path,
    resolve: &mut dyn FnMut(&str) -> ClientResult,
    cancel: &AtomicBool,
) -> Result<PricingRefreshReport, String> {
    let adapter = LocalGitAdapter::open(library_path).map_err(|e| e.to_string())?;
    refresh_library_pricing(&adapter, resolve, cancel).map_err(|e| e.to_string())
}

/// How loud a Messages-panel line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineLevel {
    Info,
    Warning,
}

/// Failed rows named one per line, then "and N more". A library of
/// thousands of SKU-less rows must not flood the panel.
pub const MAX_ROW_LINES: usize = 100;

/// The Messages-panel lines for a finished refresh: one warning per failed
/// row (capped at [`MAX_ROW_LINES`]), then the summary.
pub fn report_lines(library_name: &str, report: &PricingRefreshReport) -> Vec<(LineLevel, String)> {
    let mut lines = Vec::new();
    for failure in report.failures.iter().take(MAX_ROW_LINES) {
        lines.push((
            LineLevel::Warning,
            format!("Pricing {library_name}: {}", failure.message()),
        ));
    }
    let hidden = report.failures.len().saturating_sub(MAX_ROW_LINES);
    if hidden > 0 {
        lines.push((
            LineLevel::Warning,
            format!("Pricing {library_name}: and {hidden} more failed rows"),
        ));
    }
    for dropped in &report.store.dropped {
        lines.push((
            LineLevel::Warning,
            format!(
                "Pricing {library_name}: {} / {} not stored: {}",
                dropped.table, dropped.distributor, dropped.reason
            ),
        ));
    }
    let level = if report.failures.is_empty() && report.store.dropped.is_empty() {
        LineLevel::Info
    } else {
        LineLevel::Warning
    };
    lines.push((level, format!("{library_name}: {}", report.summary())));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use signex_library::adapter::LibraryAdapter;
    use signex_library::adapters::local_git::LibraryInitOptions;
    use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
    use signex_library::distributor::{
        DistributorError, DistributorPart, PriceBreak, PricingSnapshot,
    };
    use signex_library::identity::{ComponentClass, InternalPn};
    use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
    use signex_library::lifecycle::LifecycleState;
    use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
    use signex_library::manufacturer::{DistributorListing, ManufacturerPart};
    use signex_library::param::ParamMap;
    use signex_library::primitive::PrimitiveRef;
    use signex_library::{FailureKind, RowFailure};
    use uuid::Uuid;

    struct Fixed(&'static str, f64);

    impl DistributorAdapter for Fixed {
        fn name(&self) -> &'static str {
            self.0
        }
        fn source(&self) -> DistributorSource {
            DistributorSource::Mouser
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
        fn refresh_pricing(
            &self,
            part: &DistributorPart,
        ) -> Result<PricingSnapshot, DistributorError> {
            if part.mpn == "BAD" {
                return Err(DistributorError::Network("timed out".into()));
            }
            Ok(PricingSnapshot {
                captured_at: chrono::Utc::now(),
                currency: "EUR".into(),
                stock: Some(7),
                part_number: None,
                by_distributor: BTreeMap::from([(
                    self.0.to_string(),
                    vec![PriceBreak {
                        qty: 1,
                        unit_price: self.1,
                    }],
                )]),
            })
        }
    }

    fn row(pn: &str, lib: Uuid, listings: &[(&str, &str)]) -> ComponentRow {
        let now = chrono::Utc::now();
        ComponentRow {
            alternate_symbols: Vec::new(),
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
            server_revision: None,
        }
    }

    #[test]
    fn unknown_and_disabled_distributors_are_not_configured() {
        let enabled = [DistributorSource::Lcsc];
        for (name, needle) in [
            ("Farnell", "no pricing client"),
            ("Mouser", "not in the distributor order"),
        ] {
            match resolve_client(name, &enabled) {
                Err(ClientUnavailable::NotConfigured(m)) => assert!(m.contains(needle), "{m}"),
                Err(other) => panic!("{name}: {other:?}"),
                Ok(_) => panic!("{name}: expected a refusal"),
            }
        }
        assert!(
            resolve_client("lcsc", &enabled).is_ok(),
            "enabled keyless client"
        );
        assert!(
            resolve_client("L.C.S.C", &enabled).is_ok(),
            "name is normalised"
        );
    }

    #[test]
    fn job_stores_prices_through_the_library_and_reports_failures() {
        signex_library::git_identity::test_identity::install();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("J").join("J.snxlib");
        let id = Uuid::now_v7();
        let adapter = signex_library::LocalGitAdapter::init(
            &file,
            SnxlibManifest {
                format: FORMAT_TOKEN.into(),
                library_id: id,
                library: LibrarySection {
                    name: "J".into(),
                    description: None,
                },
                mode: LibraryMode::default(),
                workflow: WorkflowConfig::default(),
                users: UsersConfig::default(),
                classes: Vec::new(),
            },
            LibraryInitOptions::default(),
        )
        .unwrap();
        let ok = row("GOOD", id, &[("Mouser", "M-1")]);
        let ok_id = ok.row_id;
        adapter.insert_row("R", ok, "seed").unwrap();
        adapter
            .insert_row("R", row("BAD", id, &[("Mouser", "M-2")]), "seed")
            .unwrap();
        adapter
            .insert_row("R", row("NONE", id, &[]), "seed")
            .unwrap();

        let report = run_job_with(
            &file,
            &mut |_| Ok(Arc::new(Fixed("Mouser", 0.25)) as Arc<dyn DistributorAdapter>),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(report.store.stored, 1);
        assert_eq!(report.failures.len(), 2);

        let stored = adapter.table_pricing("R").unwrap();
        let cached = &stored[&ok_id]["Mouser"]["M-1"];
        assert_eq!((cached.currency.as_str(), cached.stock), ("EUR", Some(7)));
        assert_eq!(cached.headline_break().unwrap().unit_price, 0.25);
    }

    #[test]
    fn report_lines_name_each_failure_then_the_summary() {
        let report = PricingRefreshReport {
            rows_considered: 3,
            failures: vec![RowFailure {
                table: "R".into(),
                internal_pn: "R-2".into(),
                distributor: Some("Mouser".into()),
                kind: FailureKind::Network,
                detail: "Mouser network: timed out".into(),
            }],
            ..Default::default()
        };
        let lines = report_lines("Parts", &report);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, LineLevel::Warning);
        assert_eq!(
            lines[0].1,
            "Pricing Parts: R / R-2: Mouser network: timed out (Mouser)"
        );
        assert_eq!(
            lines[1].1,
            "Parts: Pricing refresh: 0 updated, 1 failed (3 rows)"
        );
    }

    #[test]
    fn report_lines_cap_the_per_row_flood() {
        let failures = (0..MAX_ROW_LINES + 7)
            .map(|i| RowFailure {
                table: "R".into(),
                internal_pn: format!("R-{i}"),
                distributor: None,
                kind: FailureKind::NoSku,
                detail: "no distributor SKU on this row".into(),
            })
            .collect();
        let report = PricingRefreshReport {
            failures,
            ..Default::default()
        };
        let lines = report_lines("Parts", &report);
        assert_eq!(
            lines.len(),
            MAX_ROW_LINES + 2,
            "rows + 'and N more' + summary"
        );
        assert!(lines[MAX_ROW_LINES].1.ends_with("and 7 more failed rows"));
    }

    #[test]
    fn a_clean_run_reports_at_info() {
        let lines = report_lines("Parts", &PricingRefreshReport::default());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, LineLevel::Info);
    }
}
