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
    Sim,
}

impl RefSlot {
    /// Lower-case noun for messages ("symbol", "footprint", "sim").
    pub fn label(self) -> &'static str {
        match self {
            RefSlot::Symbol => "symbol",
            RefSlot::Footprint => "footprint",
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
            let bindings = [
                (RefSlot::Symbol, Some(row.symbol_ref)),
                (RefSlot::Footprint, row.footprint_ref),
                (RefSlot::Sim, row.sim_ref),
            ];
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
