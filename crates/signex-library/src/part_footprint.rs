//! A component row's footprint, ready for the board (#663, #1312): which
//! binding the part uses, the library footprint it names, and the pin map to
//! wire it with.

use signex_types::part::{FootprintResolveError, FootprintSourceRef, PinPadMap, ResolvedFootprint};
use signex_types::schematic::{RowStamp, Symbol};
use uuid::Uuid;

use crate::board_footprint::board_footprint;
use crate::component::{ComponentRow, PinPadOverride};
use crate::primitive::PrimitiveRef;
use crate::primitive::footprint::Footprint;

/// The footprint binding `stamp` records; `None` for the row's default (or a
/// binding that is not a `<library uuid>:<footprint uuid>` key).
pub fn stamped_footprint(stamp: &RowStamp) -> Option<PrimitiveRef> {
    PrimitiveRef::parse_key(stamp.footprint_ref.as_deref()?)
}

/// The alternate footprint the part whose unit is `symbol` is built with,
/// from its valid stamp; `None` for the row's default.
pub fn chosen_footprint(symbol: &Symbol) -> Option<PrimitiveRef> {
    stamped_footprint(symbol.valid_stamp()?)
}

/// The board footprint of the part whose first unit is `symbol`: its library
/// row (`find_row`, by library and row id) with the binding it was built
/// with. A part with no row, or whose row gives no footprint, uses the
/// library footprint its own footprint names as `<library uuid>:<footprint
/// uuid>`, which Choose footprint sets (#2371).
pub fn resolve_symbol_footprint<'r>(
    symbol: &Symbol,
    find_row: impl FnOnce(Uuid, Uuid) -> Option<&'r ComponentRow>,
    lookup: impl Fn(&PrimitiveRef, &str) -> Option<Footprint>,
) -> Result<ResolvedFootprint, FootprintResolveError> {
    let from_row = match (symbol.library_id, symbol.row_id) {
        (Some(library), Some(row)) => find_row(library, row)
            .ok_or(FootprintResolveError::RowNotFound { library, row })
            .and_then(|row| {
                resolve_part_footprint(row, chosen_footprint(symbol).as_ref(), &lookup)
            }),
        _ => Err(FootprintResolveError::NoFootprint),
    };
    match (from_row, PrimitiveRef::parse_key(symbol.footprint.trim())) {
        (Err(_), Some(named)) => resolve_named_footprint(&named, &lookup),
        (resolved, _) => resolved,
    }
}

/// Resolve the footprint of a part built from `row`.
///
/// `chosen` is the alternate the part was placed with; `None` (or the row's
/// default itself) is the default footprint. `lookup` reads a library
/// footprint by address and pinned version (empty = latest). The row's pin
/// map applies to every binding; an alternate's own overrides take priority
/// over it pin by pin.
pub fn resolve_part_footprint(
    row: &ComponentRow,
    chosen: Option<&PrimitiveRef>,
    lookup: impl Fn(&PrimitiveRef, &str) -> Option<Footprint>,
) -> Result<ResolvedFootprint, FootprintResolveError> {
    let default = row
        .footprint_ref
        .ok_or(FootprintResolveError::NoFootprint)?;
    let address = chosen.copied().unwrap_or(default);
    let binding = row
        .footprint_binding(&address)
        .ok_or(FootprintResolveError::NotOffered {
            library: address.library_id,
            footprint: address.uuid,
        })?;
    let mut pin_map = pin_map(&row.pin_map_overrides);
    if address != default {
        for o in &binding.pin_map_overrides {
            pin_map.set(o.symbol_pin_number.clone(), o.footprint_pad_number.clone());
        }
    }
    let library =
        lookup(&address, &binding.version).ok_or_else(|| FootprintResolveError::NotFound {
            library: address.library_id,
            footprint: address.uuid,
            version: binding.version.clone(),
        })?;
    if !binding.version.is_empty() && library.version != binding.version {
        return Err(FootprintResolveError::VersionMismatch {
            wanted: binding.version,
            found: library.version,
        });
    }
    ready_for_board(&address, &library, pin_map)
}

/// Resolve a library footprint a part names itself, with no row behind it
/// (Update PCB's Choose footprint, #2371): the latest version, pin n on pad n.
pub fn resolve_named_footprint(
    address: &PrimitiveRef,
    lookup: impl Fn(&PrimitiveRef, &str) -> Option<Footprint>,
) -> Result<ResolvedFootprint, FootprintResolveError> {
    let library = lookup(address, "").ok_or_else(|| FootprintResolveError::NotFound {
        library: address.library_id,
        footprint: address.uuid,
        version: String::new(),
    })?;
    ready_for_board(address, &library, PinPadMap::default())
}

/// The library footprint `library` at `address` in its board form, stamped
/// with its source.
fn ready_for_board(
    address: &PrimitiveRef,
    library: &Footprint,
    pin_map: PinPadMap,
) -> Result<ResolvedFootprint, FootprintResolveError> {
    let source = FootprintSourceRef {
        library: address.library_id,
        footprint: address.uuid,
        version: library.version.clone(),
    };
    // The board saves this identity; one it could not read back (an empty
    // version) would make the board unopenable, so it is refused here.
    if FootprintSourceRef::parse(&source.to_string()).as_ref() != Some(&source) {
        return Err(FootprintResolveError::Unconvertible {
            what: "library footprint identity",
            value: source.to_string(),
        });
    }
    let mut footprint = board_footprint(library)?;
    footprint.library_ref = Some(source.clone());
    Ok(ResolvedFootprint {
        source,
        footprint,
        pin_map,
    })
}

fn pin_map(overrides: &[PinPadOverride]) -> PinPadMap {
    PinPadMap::from_pairs(
        overrides
            .iter()
            .map(|o| (o.symbol_pin_number.clone(), o.footprint_pad_number.clone())),
    )
}

#[cfg(test)]
#[path = "part_footprint_tests.rs"]
mod tests;
