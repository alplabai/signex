//! A component row's footprint, ready for the board (#663, #1312): which
//! binding the part uses, the library footprint it names, and the pin map to
//! wire it with.

use signex_types::part::{FootprintResolveError, FootprintSourceRef, PinPadMap, ResolvedFootprint};

use crate::board_footprint::board_footprint;
use crate::component::{ComponentRow, PinPadOverride};
use crate::primitive::PrimitiveRef;
use crate::primitive::footprint::Footprint;

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
    let mut footprint = board_footprint(&library)?;
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
