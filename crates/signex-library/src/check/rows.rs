//! Part rows: read as the app reads them, validated as a save validates
//! them, unique ids and part numbers, and every reference resolved.

use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use super::primitives::LibraryIndex;
use super::{Other, Problem};
use crate::adapter::LibraryAdapter;
use crate::adapters::local_git::LocalGitAdapter;
use crate::component::ComponentRow;
use crate::primitive::PrimitiveRef;

/// What a reference points at.
#[derive(Clone, Copy)]
enum Kind {
    Symbol,
    Footprint,
    Sim,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Symbol => "symbol",
            Kind::Footprint => "footprint",
            Kind::Sim => "sim model",
        }
    }

    fn missing_code(self) -> &'static str {
        match self {
            Kind::Symbol => "ref.missing_symbol",
            Kind::Footprint => "ref.missing_footprint",
            Kind::Sim => "ref.missing_sim",
        }
    }

    fn holds(self, index: &LibraryIndex, uuid: Uuid) -> bool {
        match self {
            Kind::Symbol => index.symbols.contains(&uuid),
            Kind::Footprint => index.footprints.contains(&uuid),
            Kind::Sim => index.sims.contains(&uuid),
        }
    }
}

pub(super) fn check_rows(
    adapter: &LocalGitAdapter,
    file: &str,
    own: &LibraryIndex,
    others: &BTreeMap<Uuid, Other>,
    out: &mut Vec<Problem>,
) {
    let tables = match adapter.list_tables() {
        Ok(tables) => tables,
        Err(e) => {
            out.push(Problem::error(
                "table.unreadable",
                file,
                None,
                e.to_string(),
            ));
            return;
        }
    };
    let mut row_ids: HashMap<Uuid, String> = HashMap::new();
    let mut pns: HashMap<String, Uuid> = HashMap::new();
    for table in tables {
        let rows = match adapter.read_table(&table) {
            Ok(rows) => rows,
            Err(e) => {
                out.push(Problem::error(
                    "table.unreadable",
                    file,
                    Some(table.clone()),
                    format!("table {table:?}: {e}"),
                ));
                continue;
            }
        };
        for row in &rows {
            let item = Some(row.row_id.to_string());
            if let Some(first) = row_ids.insert(row.row_id, table.clone()) {
                out.push(Problem::error(
                    "row.duplicate_row_id",
                    file,
                    item.clone(),
                    format!(
                        "row id {} is in table {first:?} and in {table:?}",
                        row.row_id
                    ),
                ));
            }
            let pn = row.internal_pn.as_str().trim();
            if !pn.is_empty()
                && let Some(other) = pns.insert(pn.to_string(), row.row_id)
            {
                out.push(Problem::error(
                    "row.duplicate_internal_pn",
                    file,
                    item.clone(),
                    format!("internal_pn {pn:?} is also held by row {other}"),
                ));
            }
            check_row(row, &table, file, own, others, out);
        }
    }
}

fn check_row(
    row: &ComponentRow,
    table: &str,
    file: &str,
    own: &LibraryIndex,
    others: &BTreeMap<Uuid, Other>,
    out: &mut Vec<Problem>,
) {
    let item = Some(row.row_id.to_string());
    let label = format!("row {} ({table:?})", row.internal_pn);
    if let Err(e) = crate::validate::validate_row_for_write(row) {
        out.push(Problem::error(
            "row.invalid",
            file,
            item.clone(),
            format!("a save refuses it: {e}"),
        ));
    }
    match crate::hash::hash_row_content(row) {
        Ok(hash) if hash == row.content_hash => {}
        Ok(_) => out.push(Problem::warning(
            "row.content_hash",
            file,
            item.clone(),
            format!("{label}: content_hash is not the hash of the row's content; change detection is defeated"),
        )),
        // Non-finite parameters: already reported as `row.invalid`.
        Err(_) => {}
    }
    let mut refs: Vec<(Kind, &PrimitiveRef)> = vec![(Kind::Symbol, &row.symbol_ref)];
    refs.extend(
        row.alternate_symbols
            .iter()
            .map(|a| (Kind::Symbol, &a.symbol_ref)),
    );
    refs.extend(row.footprint_ref.iter().map(|r| (Kind::Footprint, r)));
    refs.extend(
        row.alternate_footprints
            .iter()
            .map(|b| (Kind::Footprint, &b.footprint)),
    );
    refs.extend(row.sim_ref.iter().map(|r| (Kind::Sim, r)));
    for (kind, r) in refs {
        resolve(kind, r, &label, file, &item, own, others, out);
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    kind: Kind,
    r: &PrimitiveRef,
    label: &str,
    file: &str,
    item: &Option<String>,
    own: &LibraryIndex,
    others: &BTreeMap<Uuid, Other>,
    out: &mut Vec<Problem>,
) {
    let index = if r.library_id == own.library_id {
        own
    } else if let Some(other) = others.get(&r.library_id) {
        if !other.required {
            out.push(Problem::warning(
                "ref.not_required",
                file,
                item.clone(),
                format!(
                    "{label}: {} {} is in library {}, which the manifest does not list in `requires`",
                    kind.name(),
                    r.uuid,
                    r.library_id
                ),
            ));
        }
        let Some(index) = &other.index else {
            return;
        };
        index
    } else {
        out.push(Problem::error(
            "ref.unknown_library",
            file,
            item.clone(),
            format!(
                "{label}: {} {} is in library {}, which is neither this library nor one given with --with",
                kind.name(),
                r.uuid,
                r.library_id
            ),
        ));
        return;
    };
    if !kind.holds(index, r.uuid) {
        out.push(Problem::error(
            kind.missing_code(),
            file,
            item.clone(),
            format!(
                "{label}: {} {} is not in library {}",
                kind.name(),
                r.uuid,
                r.library_id
            ),
        ));
    }
}
