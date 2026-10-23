//! Off-thread half of the place-from-library flow (#364): open the library,
//! read the picked row and its `.snxsym`, convert to a `LibSymbol`.
//!
//! Pure IO + conversion with no app state, so it can run in
//! `spawn_blocking`; the `update`-side half is
//! `app/handlers/library_place.rs`.

use std::path::Path;

use signex_library::RowId;

use super::messages::ResolvedPlacement;

/// Read `row_id` of `table` from the `.snxlib` at `library_path` and
/// resolve its symbol. `lib_id` is `"<library uuid>:<symbol uuid>"`.
pub fn resolve_placement(
    library_path: &Path,
    table: &str,
    row_id: RowId,
) -> Result<ResolvedPlacement, String> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    let adapter = LocalGitAdapter::open(library_path)
        .map_err(|e| format!("Could not open {}: {e}", library_path.display()))?;
    let library_id = adapter.library_id();
    let row = adapter
        .read_row(table, row_id)
        .map_err(|e| format!("Could not read row {row_id} of table {table}: {e}"))?;
    if row.symbol_ref.library_id != library_id {
        return Err(format!(
            "{} uses a symbol from another library; placing that is not supported yet.",
            row.internal_pn
        ));
    }
    let symbol = adapter
        .get_symbol(row.symbol_ref.uuid)
        .map_err(|e| format!("Could not read the symbol of {}: {e}", row.internal_pn))?;

    let lib_id = format!("{}:{}", row.symbol_ref.library_id, row.symbol_ref.uuid);
    let value = if row.primary_mpn.mpn.is_empty() {
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
        prefix: designator_prefix(&symbol.designator),
    })
}

/// Reference prefix from a library designator: `"U?"` -> `"U"`, empty -> `"U"`.
fn designator_prefix(designator: &str) -> String {
    let prefix: String = designator
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if prefix.is_empty() {
        "U".into()
    } else {
        prefix.to_ascii_uppercase()
    }
}
