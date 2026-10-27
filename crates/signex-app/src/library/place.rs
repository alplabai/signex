//! Off-thread half of the place-from-library flow (#364): open the library,
//! read the picked row and its `.snxsym`, convert to a `LibSymbol`.
//!
//! Pure IO + conversion with no app state, so it can run in
//! `spawn_blocking`; the `update`-side half is
//! `app/handlers/library_place.rs`.

use std::path::{Path, PathBuf};

use signex_library::RowId;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

use super::messages::{ResolvedPlacement, ResolvedSymbolChoice};

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
    resolve_placement_of(library_path, table, row_id, None, open_libraries)
}

/// [`resolve_placement`] for the drawing `chosen` names: the row's default
/// symbol when `None`, else one of its alternates (refused when the row no
/// longer offers it).
pub fn resolve_placement_of(
    library_path: &Path,
    table: &str,
    row_id: RowId,
    chosen: Option<PrimitiveRef>,
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
    let reference = chosen.unwrap_or(row.symbol_ref);
    let symbol = read_symbol(&adapter, &row, &reference, open_libraries)?;

    let lib_id = lib_id_of(&reference);
    let is_flag = is_power_flag(&symbol);
    let is_power = is_flag || is_power_port(&symbol);
    let value = if is_flag {
        // A flag marks a net as driven; it must not name it, and an empty
        // value keeps it out of the netlist's by-name joining.
        String::new()
    } else if is_power {
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
        prefix: signex_types::designator::placement_prefix(&symbol.designator),
        class: row.class.as_str().to_owned(),
    })
}

/// `"<library uuid>:<symbol uuid>"` for a symbol reference.
pub fn lib_id_of(reference: &PrimitiveRef) -> String {
    format!("{}:{}", reference.library_id, reference.uuid)
}

/// Read the symbol `reference` names. It may live in another open library
/// (the worker gets an id-to-path map); a library that is not open is
/// reported by name.
fn read_symbol(
    own: &signex_library::adapters::local_git::LocalGitAdapter,
    row: &signex_library::ComponentRow,
    reference: &PrimitiveRef,
    open_libraries: &[(Uuid, PathBuf)],
) -> Result<signex_library::primitive::Symbol, String> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    if !row.binds_symbol(reference) {
        return Err(format!(
            "{} no longer offers symbol {}.",
            row.internal_pn, reference.uuid
        ));
    }
    if reference.library_id == own.library_id() {
        return own
            .get_symbol(reference.uuid)
            .map_err(|e| format!("Could not read the symbol of {}: {e}", row.internal_pn));
    }
    let owner = open_libraries
        .iter()
        .find(|(id, _)| *id == reference.library_id)
        .map(|(_, path)| path)
        .ok_or_else(|| {
            format!(
                "The symbol of {} lives in library {}, which is not open. Open it and try again.",
                row.internal_pn, reference.library_id
            )
        })?;
    LocalGitAdapter::open(owner)
        .map_err(|e| format!("Could not open {}: {e}", owner.display()))?
        .get_symbol(reference.uuid)
        .map_err(|e| format!("Could not read the symbol of {}: {e}", row.internal_pn))
}

/// Read the drawing `reference` of a row and convert it for embedding on a
/// sheet. The switch to an alternate registers this copy next to the
/// placed symbol.
pub fn resolve_symbol_choice(
    library_path: &Path,
    table: &str,
    row_id: RowId,
    reference: PrimitiveRef,
    open_libraries: &[(Uuid, PathBuf)],
) -> Result<ResolvedSymbolChoice, String> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    let adapter = LocalGitAdapter::open(library_path)
        .map_err(|e| format!("Could not open {}: {e}", library_path.display()))?;
    let row = adapter
        .read_row(table, row_id)
        .map_err(|e| format!("Could not read row {row_id} of table {table}: {e}"))?;
    let symbol = read_symbol(&adapter, &row, &reference, open_libraries)?;
    let lib_id = lib_id_of(&reference);
    Ok(ResolvedSymbolChoice {
        lib_symbol: symbol.to_lib_symbol(lib_id.clone()),
        lib_id,
    })
}

/// A power flag: designator prefix `#FLG` (`"#FLG?"`).
fn is_power_flag(symbol: &signex_library::primitive::Symbol) -> bool {
    signex_types::designator::placement_prefix(&symbol.designator) == "#FLG"
}

/// A power port: its only pin is a `PinDirection::Power` pin (the stock
/// `GND` / `VCC` symbols). Flags are handled separately.
fn is_power_port(symbol: &signex_library::primitive::Symbol) -> bool {
    use signex_library::primitive::PinDirection;
    !is_power_flag(symbol)
        && matches!(symbol.pins.as_slice(), [pin] if pin.electrical == PinDirection::Power)
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
