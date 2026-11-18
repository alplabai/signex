//! A library row read for placing, as the off-thread read hands it back.

/// What the off-thread read of a library row hands back to `update`:
/// everything needed to register the `LibSymbol` and place the part,
/// with no further IO.
#[derive(Debug, Clone)]
pub struct ResolvedPlacement {
    /// `"<library uuid>:<symbol uuid>"` — `LibSymbol.id` and `Symbol.lib_id`.
    pub lib_id: String,
    pub lib_symbol: signex_types::schematic::LibSymbol,
    pub library_id: uuid::Uuid,
    pub row_id: uuid::Uuid,
    pub library_version: String,
    /// The row values the placed part starts in agreement with; it travels
    /// with `library_version` so the two are stamped together (#2004).
    pub stamp: signex_types::schematic::RowStamp,
    pub internal_pn: String,
    pub value: String,
    /// The symbol is a power port: the placed instance is `is_power` and
    /// `value` is its net name.
    pub is_power: bool,
    /// Reference prefix from the library symbol's designator (`U?` -> `U`):
    /// the legacy prefix, used when the owning project follows no designator
    /// standard or does not cover `class`.
    pub prefix: String,
    /// The row's component class key (`"diode"`, `"generic"`), for the
    /// project's designator standard (#1232).
    pub class: String,
    /// The row's raw `designator_prefix` parameter, not yet validated: it
    /// beats the standard's table entry for `class` (#1315).
    pub prefix_override: Option<String>,
    /// The bound footprint's name (empty when none): a placement stamps it
    /// on the symbol, where ERC, the netlist and the BOM read it (#1842).
    pub footprint: String,
    /// The part's primary datasheet link and identity fields (Manufacturer,
    /// MPN and the row's parameters). A placement stamps the
    /// Manufacturer and MPN (#1867); Change Component replaces the old
    /// part's with all of them.
    pub datasheet: String,
    pub fields: Vec<(String, String)>,
}
