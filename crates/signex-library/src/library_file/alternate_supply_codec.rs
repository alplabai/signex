//! Read and write the `[tables.<name>.alternate_supply...]` sidecar: what a
//! distributor refresh fetched for each row's alternates (#3046,
//! `docs/LIBRARY_FORMAT.md` 2.6).

use std::collections::BTreeMap;

use super::codec::{toml_key, toml_string};
use crate::alternate_supply::{FetchedSupply, RowAlternateSupply};

/// The sidecar as a table. A value that is not a table (a hand edit) is
/// dropped with a warning, never fatal: it is fetched again.
pub(super) fn alternate_supply_table(table: &str, raw: Option<toml::Value>) -> toml::Table {
    match raw {
        None => toml::Table::new(),
        Some(toml::Value::Table(t)) => t,
        Some(_) => {
            warn_dropped(
                table,
                "",
                "",
                "",
                "the alternate_supply section is not a table",
            );
            toml::Table::new()
        }
    }
}

/// Read a table's fetched alternate supply. The records hold no floats, so
/// nothing here fails the load: a damaged row, distributor or record is
/// dropped with one warning naming it, and the next refresh fetches it again.
pub(super) fn read_alternate_supply(
    table: &str,
    raw: toml::Table,
) -> BTreeMap<String, RowAlternateSupply> {
    let mut out: BTreeMap<String, RowAlternateSupply> = BTreeMap::new();
    for (row_id, per_distributor) in raw {
        let toml::Value::Table(per_distributor) = per_distributor else {
            warn_dropped(table, &row_id, "", "", "row entry is not a table");
            continue;
        };
        for (distributor, per_mpn) in per_distributor {
            let toml::Value::Table(per_mpn) = per_mpn else {
                warn_dropped(
                    table,
                    &row_id,
                    &distributor,
                    "",
                    "distributor entry is not a table",
                );
                continue;
            };
            for (mpn, value) in per_mpn {
                let record: FetchedSupply = match value.try_into() {
                    Ok(record) => record,
                    Err(e) => {
                        warn_dropped(table, &row_id, &distributor, &mpn, &e.to_string());
                        continue;
                    }
                };
                out.entry(row_id.clone())
                    .or_default()
                    .entry(distributor.clone())
                    .or_default()
                    .insert(mpn, record);
            }
        }
    }
    out
}

/// Append one `[tables.<name>.alternate_supply.<row_id>.<distributor>.<mpn>]`
/// sub-table per record, in byte order of each key; `stock` and `lifecycle`
/// only when known.
pub(super) fn write_alternate_supply(
    out: &mut String,
    table: &str,
    supply: &BTreeMap<String, RowAlternateSupply>,
) {
    for (row_id, per_distributor) in supply {
        for (distributor, per_mpn) in per_distributor {
            for (mpn, record) in per_mpn {
                out.push('\n');
                out.push_str(&format!(
                    "[tables.{}.alternate_supply.{}.{}.{}]\n",
                    toml_key(table),
                    toml_key(row_id),
                    toml_key(distributor),
                    toml_key(mpn)
                ));
                if let Some(stock) = record.stock {
                    out.push_str(&format!("stock = {stock}\n"));
                }
                if let Some(lifecycle) = record.lifecycle {
                    out.push_str(&format!("lifecycle = {}\n", toml_string(lifecycle.token())));
                }
                out.push_str(&format!(
                    "fetched_at = {}\n",
                    toml_string(&record.fetched_at_text())
                ));
            }
        }
    }
}

fn warn_dropped(table: &str, row_id: &str, distributor: &str, mpn: &str, reason: &str) {
    tracing::warn!(
        target: "signex::library",
        table = %table,
        row = %row_id,
        distributor = %distributor,
        part = %mpn,
        reason = %reason,
        "dropped an unusable alternate stock record; it will be fetched again by Refresh All Pricing"
    );
}
