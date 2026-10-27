//! Component rows whose primitive bindings point at nothing.
//!
//! A row binds a symbol, optionally a footprint and a sim by
//! [`PrimitiveRef`]. When the primitive file is deleted or the target
//! library is gone, the binding dangles and the row silently renders
//! without its graphics. [`dangling_row_refs`] finds those bindings.
//!
//! The check is pure: the caller says what exists through `resolves`,
//! so the sweep does no IO and is cheap enough to run after every table
//! refresh. A nil UUID is "not bound", never "dangling".

use uuid::Uuid;

use crate::component::ComponentRow;
use crate::primitive::PrimitiveRef;

/// Which binding of a row a [`DanglingRef`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefSlot {
    Symbol,
    Footprint,
    /// An alternate footprint (#1234), not the default.
    AlternateFootprint,
    Sim,
}

impl RefSlot {
    /// Lower-case noun for messages ("symbol", "footprint", "sim").
    pub fn label(self) -> &'static str {
        match self {
            RefSlot::Symbol => "symbol",
            RefSlot::Footprint => "footprint",
            RefSlot::AlternateFootprint => "alternate footprint",
            RefSlot::Sim => "sim",
        }
    }
}

/// One binding that resolves to nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DanglingRef {
    /// Table the row lives in.
    pub table: String,
    pub row_id: Uuid,
    /// The row's internal part number, for the message.
    pub internal_pn: String,
    pub slot: RefSlot,
    /// The missing primitive.
    pub target: PrimitiveRef,
}

/// Every binding of every row that `resolves` rejects, in table-name
/// then row order.
pub fn dangling_row_refs<'a, I, F>(tables: I, resolves: F) -> Vec<DanglingRef>
where
    I: IntoIterator<Item = (&'a str, &'a [ComponentRow])>,
    F: Fn(RefSlot, &PrimitiveRef) -> bool,
{
    let mut tables: Vec<_> = tables.into_iter().collect();
    tables.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = Vec::new();
    for (table, rows) in tables {
        for row in rows {
            let alternates = row
                .alternate_symbols
                .iter()
                .map(|a| (RefSlot::Symbol, Some(a.symbol_ref)));
            let bindings = [
                (RefSlot::Symbol, Some(row.symbol_ref)),
                (RefSlot::Footprint, row.footprint_ref),
                (RefSlot::Sim, row.sim_ref),
            ]
            .into_iter()
            .chain(alternates)
            .chain(
                row.alternate_footprints
                    .iter()
                    .map(|alt| (RefSlot::AlternateFootprint, Some(alt.footprint))),
            );
            for (slot, target) in bindings {
                let Some(target) = target else { continue };
                if target.uuid.is_nil() || resolves(slot, &target) {
                    continue;
                }
                out.push(DanglingRef {
                    table: table.to_string(),
                    row_id: row.row_id,
                    internal_pn: row.internal_pn.as_str().to_string(),
                    slot,
                    target,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alternate_symbol::AlternateSymbol;

    /// A footprint that exists only as an alternate is reported when it is
    /// gone, under its own slot; the default and live alternates are not
    /// (#1234).
    #[test]
    fn a_missing_alternate_footprint_is_reported_per_alternate() {
        use crate::footprint_binding::FootprintBinding;
        let mut row = crate::component::tests::fixture_row();
        let lib = row.symbol_ref.library_id;
        let live = PrimitiveRef::new(lib, Uuid::from_u128(2));
        let live_alt = PrimitiveRef::new(lib, Uuid::from_u128(5));
        let gone = PrimitiveRef::new(lib, Uuid::from_u128(3));
        let also_gone = PrimitiveRef::new(lib, Uuid::from_u128(4));
        row.footprint_ref = Some(live);
        row.alternate_footprints = vec![
            FootprintBinding::new(gone),
            FootprintBinding::new(live_alt),
            FootprintBinding::new(also_gone),
        ];
        let rows = [row];
        let found = dangling_row_refs([("t", &rows[..])], |slot, target| match slot {
            RefSlot::Symbol => true,
            _ => *target == live || *target == live_alt,
        });
        let slots: Vec<_> = found.iter().map(|d| (d.slot, d.target)).collect();
        assert_eq!(
            slots,
            vec![
                (RefSlot::AlternateFootprint, gone),
                (RefSlot::AlternateFootprint, also_gone)
            ]
        );
    }

    #[test]
    fn an_alternate_symbol_pointing_nowhere_is_dangling() {
        let mut row = crate::component::tests::fixture_row();
        let missing = PrimitiveRef::new(row.symbol_ref.library_id, Uuid::new_v4());
        let default_ref = row.symbol_ref;
        row.alternate_symbols
            .push(AlternateSymbol::new(missing, "0.0.1", "IEC"));
        let rows = vec![row];

        let out = dangling_row_refs([("R", rows.as_slice())], |slot, r| {
            slot != RefSlot::Symbol || *r == default_ref
        });
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].slot, RefSlot::Symbol);
        assert_eq!(out[0].target, missing);
    }
}
