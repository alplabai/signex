//! Refresh the cached distributor prices of a library.
//!
//! The domain half of "Refresh All Pricing": which rows are eligible, how
//! each distributor is walked, and what a failure is called. It does network
//! IO through [`DistributorAdapter`] clients, so callers run it off the UI
//! thread; nothing here touches app state.
//!
//! Rows are visited one distributor at a time and one row at a time. The
//! community clients space their own requests (their `polite_wait`), so
//! sequential calls are what keeps a distributor's rate limit honoured. An
//! auth or rate-limit failure ends that distributor's pass: the rest of its
//! rows are reported with the same reason instead of hammering a refused
//! endpoint.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;

use crate::adapter::{LibraryAdapter, LibraryError};
use crate::component::ComponentRow;
use crate::distributor::{DistributorAdapter, DistributorError, DistributorPart};
use crate::param::ParamMap;
use crate::pricing::{CachedPricing, PricingUpdate, StoreOutcome, distributor_key};

/// Why a distributor client could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientUnavailable {
    /// Signex has no client for this distributor, or it is switched off.
    NotConfigured(String),
    /// A client exists but its credentials are missing.
    AuthMissing(String),
}

/// What went wrong for one row, coarse enough to count and group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    NoSku,
    NoMpn,
    NotConfigured,
    Auth,
    Network,
    RateLimited,
    NotFound,
    Backend,
    InvalidPrice,
    Cancelled,
}

/// One row the refresh could not update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFailure {
    pub table: String,
    pub internal_pn: String,
    /// The listing's distributor, when the failure belongs to one.
    pub distributor: Option<String>,
    pub kind: FailureKind,
    pub detail: String,
}

impl RowFailure {
    /// One Messages-panel line. Never carries a credential: the detail comes
    /// from client errors, which describe the problem, not the secret.
    pub fn message(&self) -> String {
        match &self.distributor {
            Some(d) => format!(
                "{} / {}: {} ({d})",
                self.table, self.internal_pn, self.detail
            ),
            None => format!("{} / {}: {}", self.table, self.internal_pn, self.detail),
        }
    }
}

/// Outcome of fetching; nothing is stored yet.
#[derive(Debug, Clone, Default)]
pub struct PricingRefreshReport {
    /// Rows looked at.
    pub rows_considered: usize,
    /// Prices fetched, ready for [`LibraryAdapter::store_pricing`].
    pub updates: Vec<PricingUpdate>,
    pub failures: Vec<RowFailure>,
    /// The cancel flag stopped the run before every row was visited.
    pub cancelled: bool,
    /// Filled by [`refresh_library_pricing`] once the write-back ran.
    pub store: StoreOutcome,
}

impl PricingRefreshReport {
    /// `"Pricing refresh: 12 updated, 3 failed (5 rows)"`.
    pub fn summary(&self) -> String {
        let stale = self.store.dropped.len();
        let mut s = format!(
            "Pricing refresh: {} updated, {} failed",
            self.store.stored,
            self.failures.len() + stale,
        );
        if stale > 0 {
            s.push_str(&format!(" ({stale} changed while fetching)"));
        }
        s.push_str(&format!(" ({} rows)", self.rows_considered));
        if self.cancelled {
            s.push_str(", cancelled");
        }
        s
    }
}

/// What a client lookup answers: the client, or why there is none.
pub type ClientResult = Result<Arc<dyn DistributorAdapter>, ClientUnavailable>;

struct Target<'a> {
    table: &'a str,
    row: &'a ComponentRow,
    /// The listing's distributor text and trimmed SKU.
    distributor: &'a str,
    sku: String,
}

/// Fetch prices for every eligible row in `rows`.
///
/// A row is eligible per listing: it names a distributor and carries a SKU.
/// A row with no usable listing at all is reported as `NoSku`. `resolve` maps
/// a listing's distributor text to a client (it is asked once per distributor
/// key). `cancel` is checked between rows.
pub fn refresh_pricing_for_rows(
    rows: &[(String, ComponentRow)],
    resolve: &mut dyn FnMut(&str) -> ClientResult,
    cancel: &AtomicBool,
) -> PricingRefreshReport {
    let mut report = PricingRefreshReport {
        rows_considered: rows.len(),
        ..Default::default()
    };
    let mut groups: BTreeMap<String, Vec<Target<'_>>> = BTreeMap::new();
    for (table, row) in rows {
        let mut any = false;
        for l in &row.supply {
            let sku = l.sku.trim();
            let key = distributor_key(&l.distributor);
            if sku.is_empty() || key.is_empty() {
                continue;
            }
            any = true;
            groups.entry(key).or_default().push(Target {
                table,
                row,
                distributor: &l.distributor,
                sku: sku.to_string(),
            });
        }
        if !any {
            report.failures.push(RowFailure {
                table: table.clone(),
                internal_pn: row.internal_pn.as_str().to_string(),
                distributor: None,
                kind: FailureKind::NoSku,
                detail: "no distributor SKU on this row".into(),
            });
        }
    }

    for targets in groups.values() {
        let client = match resolve(targets[0].distributor) {
            Ok(c) => c,
            Err(unavail) => {
                for t in targets {
                    report.failures.push(unavailable_failure(t, &unavail));
                }
                continue;
            }
        };
        // A client that cannot return prices would answer "no price" for
        // every row; say what is true instead.
        if !client.supports_pricing() {
            let detail = format!("{} does not return prices yet", client.name());
            for t in targets {
                report
                    .failures
                    .push(failure(t, FailureKind::NotConfigured, detail.clone()));
            }
            continue;
        }
        let mut halted: Option<(FailureKind, String)> = None;
        for t in targets {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                report.failures.push(failure(
                    t,
                    FailureKind::Cancelled,
                    "refresh cancelled before this row".into(),
                ));
                continue;
            }
            if let Some((kind, detail)) = &halted {
                report.failures.push(failure(t, *kind, detail.clone()));
                continue;
            }
            match fetch_one(client.as_ref(), t) {
                Ok(update) => report.updates.push(update),
                Err((kind, detail)) => {
                    if matches!(kind, FailureKind::Auth | FailureKind::RateLimited) {
                        halted = Some((kind, detail.clone()));
                    }
                    report.failures.push(failure(t, kind, detail));
                }
            }
        }
    }
    report
}

/// Fetch for every row of `adapter`, then write all prices back as one save.
///
/// Reads the rows, runs [`refresh_pricing_for_rows`] (slow, network), and
/// stores through [`LibraryAdapter::store_pricing`], which re-checks each row
/// under the library write lock. Stores nothing, and commits nothing, when no
/// price was fetched.
pub fn refresh_library_pricing(
    adapter: &dyn LibraryAdapter,
    resolve: &mut dyn FnMut(&str) -> ClientResult,
    cancel: &AtomicBool,
) -> Result<PricingRefreshReport, LibraryError> {
    let rows = adapter.iter_rows()?;
    let mut report = refresh_pricing_for_rows(&rows, resolve, cancel);
    if !report.updates.is_empty() {
        let n = report.updates.len();
        report.store = adapter.store_pricing(
            report.updates.clone(),
            &format!("pricing: refresh {n} cached price(s)"),
        )?;
    }
    Ok(report)
}

fn fetch_one(
    client: &dyn DistributorAdapter,
    t: &Target<'_>,
) -> Result<PricingUpdate, (FailureKind, String)> {
    let mpn = t.row.primary_mpn.mpn.trim();
    if mpn.is_empty() {
        return Err((FailureKind::NoMpn, "no MPN on this row".into()));
    }
    let now = Utc::now();
    let part = DistributorPart {
        mpn: mpn.to_string(),
        manufacturer: t.row.primary_mpn.manufacturer.clone(),
        description: String::new(),
        datasheet_url: None,
        footprint_hint: None,
        parameters: ParamMap::new(),
        pricing: None,
        stock: None,
        source: client.source(),
        captured_at: now,
        extra: BTreeMap::from([("sku".to_string(), t.sku.clone())]),
    };
    let snapshot = client
        .refresh_pricing(&part)
        .map_err(|e| classify(&e, client.name()))?;
    let pricing = CachedPricing::from_snapshot(client.name(), &t.sku, &snapshot)
        .map_err(|e| (FailureKind::InvalidPrice, e.to_string()))?;
    Ok(PricingUpdate {
        table: t.table.to_string(),
        row_id: t.row.row_id,
        distributor: client.name().to_string(),
        pricing,
    })
}

fn classify(e: &DistributorError, name: &str) -> (FailureKind, String) {
    match e {
        DistributorError::Auth(m) => (FailureKind::Auth, format!("{name} auth: {m}")),
        DistributorError::Network(m) => (FailureKind::Network, format!("{name} network: {m}")),
        DistributorError::RateLimited {
            retry_after_seconds,
        } => (
            FailureKind::RateLimited,
            format!("{name} rate-limited; retry after {retry_after_seconds}s"),
        ),
        DistributorError::NotFound => (
            FailureKind::NotFound,
            format!("{name} returned no price for this part"),
        ),
        DistributorError::Backend(m) => (FailureKind::Backend, format!("{name} backend: {m}")),
    }
}

fn failure(t: &Target<'_>, kind: FailureKind, detail: String) -> RowFailure {
    RowFailure {
        table: t.table.to_string(),
        internal_pn: t.row.internal_pn.as_str().to_string(),
        distributor: Some(t.distributor.to_string()),
        kind,
        detail,
    }
}

fn unavailable_failure(t: &Target<'_>, u: &ClientUnavailable) -> RowFailure {
    match u {
        ClientUnavailable::NotConfigured(m) => failure(t, FailureKind::NotConfigured, m.clone()),
        ClientUnavailable::AuthMissing(m) => failure(t, FailureKind::Auth, m.clone()),
    }
}
