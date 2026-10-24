//! Off-thread half of the place-from-library flow (#364): open the library,
//! read the picked row and its `.snxsym`, convert to a `LibSymbol`.
//!
//! Pure IO + conversion with no app state, so it can run in
//! `spawn_blocking`; the `update`-side half is
//! `app/handlers/library_place.rs`.

use std::path::{Path, PathBuf};

use signex_library::RowId;
use uuid::Uuid;

use super::messages::ResolvedPlacement;

/// Read `row_id` of `table` from the `.snxlib` at `library_path` and
/// resolve its symbol. `lib_id` is `"<library uuid>:<symbol uuid>"`.
///
/// A row may bind a symbol living in another library (the primitive
/// picker lists symbols from every open library); `open_libraries` maps
/// each open library's id to its path so the worker can open that one
/// too. The library id stamped on the placement is the row's own.
pub fn resolve_placement(
    library_path: &Path,
    table: &str,
    row_id: RowId,
    open_libraries: &[(Uuid, PathBuf)],
) -> Result<ResolvedPlacement, String> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    let adapter = LocalGitAdapter::open(library_path)
        .map_err(|e| format!("Could not open {}: {e}", library_path.display()))?;
    let library_id = adapter.library_id();
    let row = adapter
        .read_row(table, row_id)
        .map_err(|e| format!("Could not read row {row_id} of table {table}: {e}"))?;
    let symbol = if row.symbol_ref.library_id == library_id {
        adapter.get_symbol(row.symbol_ref.uuid)
    } else {
        let owner = open_libraries
            .iter()
            .find(|(id, _)| *id == row.symbol_ref.library_id)
            .map(|(_, path)| path)
            .ok_or_else(|| {
                format!(
                    "The symbol of {} lives in library {}, which is not open. Open it and try again.",
                    row.internal_pn, row.symbol_ref.library_id
                )
            })?;
        LocalGitAdapter::open(owner)
            .map_err(|e| format!("Could not open {}: {e}", owner.display()))?
            .get_symbol(row.symbol_ref.uuid)
    }
    .map_err(|e| format!("Could not read the symbol of {}: {e}", row.internal_pn))?;

    let lib_id = format!("{}:{}", row.symbol_ref.library_id, row.symbol_ref.uuid);
    let is_power = is_power_port(&symbol);
    let value = if is_power {
        power_port_net_name(&symbol)
    } else if row.primary_mpn.mpn.is_empty() {
        symbol.comment.clone()
    } else {
        row.primary_mpn.mpn.clone()
    };
    Ok(ResolvedPlacement {
        lib_symbol: symbol.to_lib_symbol(lib_id.clone()),
        lib_id,
        library_id,
        row_id: row.row_id,
        library_version: row.version.clone(),
        internal_pn: row.internal_pn.to_string(),
        value,
        is_power,
        prefix: designator_prefix(&symbol.designator),
    })
}

/// A power port: designator starts with `#`, or its only pin is a
/// `PinDirection::Power` pin (the stock `GND` / `VCC` symbols).
fn is_power_port(symbol: &signex_library::primitive::Symbol) -> bool {
    use signex_library::primitive::PinDirection;
    symbol.designator.trim_start().starts_with('#')
        || matches!(symbol.pins.as_slice(), [pin] if pin.electrical == PinDirection::Power)
}

/// The global net name a power port stamps: its comment, or the pin name
/// when the comment is empty or the `*` placeholder.
fn power_port_net_name(symbol: &signex_library::primitive::Symbol) -> String {
    let comment = symbol.comment.trim();
    if !comment.is_empty() && comment != "*" {
        return comment.to_owned();
    }
    symbol
        .pins
        .first()
        .map(|p| p.name.trim().to_owned())
        .unwrap_or_default()
}

/// Reference prefix from a library designator: `"U?"` -> `"U"`, empty -> `"U"`.
/// A leading `#` is kept (`"#PWR?"` -> `"#PWR"`), the convention for
/// power and virtual parts.
fn designator_prefix(designator: &str) -> String {
    let trimmed = designator.trim();
    let hash = if trimmed.starts_with('#') { "#" } else { "" };
    let letters: String = trimmed
        .trim_start_matches('#')
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let prefix = format!("{hash}{letters}");
    if letters.is_empty() {
        "U".into()
    } else {
        prefix.to_ascii_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::designator_prefix;

    #[test]
    fn prefix_keeps_a_leading_hash_and_defaults_to_u() {
        assert_eq!(designator_prefix("U?"), "U");
        assert_eq!(designator_prefix("#PWR?"), "#PWR");
        assert_eq!(designator_prefix("r1"), "R");
        assert_eq!(designator_prefix("?"), "U");
        assert_eq!(designator_prefix("#?"), "U");
    }
}
