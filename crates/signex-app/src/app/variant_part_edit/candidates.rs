//! The Alternate part picker's candidates (mockup row 208): the library
//! alternates of the row the part was placed from first, then a search over
//! the project libraries (DECISIONS 2026-11-26). Read off the update thread
//! (`read_candidates`, blocking IO in a `Task`); the rest is pure.

use std::path::PathBuf;

use signex_library::{
    AlternateStatus, AlternateSupply, ComponentRow, LibraryAdapter, LifecycleState, RowPricing,
};
use uuid::Uuid;

/// Search results listed at most; the heading says how many matched.
pub const FOUND_SHOWN: usize = 20;

/// Where a candidate comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    /// A manufacturer alternate of the part's own library row.
    LibraryAlternate,
    /// A row the search over the project libraries found.
    Search,
}

/// What the library says of a candidate: an alternate's approval, or a
/// row's lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateStatus {
    Alternate(AlternateStatus),
    Lifecycle(LifecycleState),
}

/// One part the variant could use instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub source: CandidateSource,
    pub mpn: String,
    pub manufacturer: String,
    pub value: String,
    /// The footprint's name; empty when the row binds none or it is
    /// unreadable.
    pub footprint: String,
    /// `(number, name)` of its symbol's pins, sorted, each once; `None`
    /// when the symbol could not be read.
    pub pins: Option<Vec<(String, String)>>,
    /// Units in stock: a search result's from its cached prices, a library
    /// alternate's from [`Self::supply`]; `None` when unknown.
    pub stock: Option<u64>,
    pub status: CandidateStatus,
    /// A library alternate's stock and lifecycle, hand-entered and fetched
    /// (#3046); `None` for a search result.
    pub supply: Option<AlternateSupply>,
}

/// What one read found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Candidates {
    /// The file name of the library holding the part's row, when found.
    pub library: Option<String>,
    pub alternates: Vec<Candidate>,
    /// The first [`FOUND_SHOWN`] search results.
    pub found: Vec<Candidate>,
    /// How many rows the search matched.
    pub found_total: usize,
    /// Project libraries that could not be read.
    pub unread: usize,
    /// The placed part's pins that carry a wire on its sheet
    /// (`super::wiring`), when this read was asked to work them out.
    pub wired: Option<Vec<String>>,
}

impl Candidates {
    /// Every candidate, alternates first.
    pub fn all(&self) -> impl Iterator<Item = &Candidate> {
        self.alternates.iter().chain(&self.found)
    }
}

/// What a read looks for.
#[derive(Debug, Clone)]
pub struct CandidateQuery {
    /// The part's row: `(library, row)`.
    pub row: Option<(Uuid, Uuid)>,
    /// The part's own part number, left out of the search results.
    pub base_mpn: String,
    pub query: String,
    /// The project's component libraries, in project order.
    pub project_libraries: Vec<PathBuf>,
    /// Every open library, for a row's symbol or footprint in another one.
    pub open_libraries: Vec<(Uuid, PathBuf)>,
    /// The sheet the part is placed on, when the read also works out which
    /// of its pins carry a wire (the first read of a picker).
    pub sheet: Option<std::sync::Arc<signex_types::schematic::SchematicSheet>>,
    /// The part's units on that sheet.
    pub units: Vec<Uuid>,
}

/// The candidates `query` names. Blocking IO: run it in a `Task`.
pub fn read_candidates(query: &CandidateQuery) -> Candidates {
    use signex_library::adapters::local_git::LocalGitAdapter;

    let mut paths = query.project_libraries.clone();
    if let Some((library, _)) = query.row
        && let Some((_, path)) = query.open_libraries.iter().find(|(id, _)| *id == library)
        && !paths.contains(path)
    {
        paths.push(path.clone());
    }
    let other = |library: Uuid| -> Option<Box<dyn LibraryAdapter>> {
        let (_, path) = query.open_libraries.iter().find(|(id, _)| *id == library)?;
        let adapter = LocalGitAdapter::open_to_read(path).ok()?;
        Some(Box::new(adapter))
    };
    // The part's wired pins come from the one connectivity the app derives.
    let wired = query
        .sheet
        .as_deref()
        .map(|sheet| super::wiring::wired_pins(&signex_net::build_netlist(sheet), &query.units));
    let mut out = Candidates {
        wired,
        ..Candidates::default()
    };
    for path in &paths {
        let Ok(adapter) = LocalGitAdapter::open_to_read(path) else {
            out.unread += 1;
            continue;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        collect(&adapter, &name, query, &other, &mut out);
    }
    out
}

/// Add what `adapter` (the library file `name`) holds for `query` to `out`.
pub fn collect(
    adapter: &dyn LibraryAdapter,
    name: &str,
    query: &CandidateQuery,
    other: &dyn Fn(Uuid) -> Option<Box<dyn LibraryAdapter>>,
    out: &mut Candidates,
) {
    let Ok(tables) = adapter.list_tables() else {
        out.unread += 1;
        return;
    };
    let library = adapter.library_id();
    for table in tables {
        let pricing = adapter.table_pricing(&table).unwrap_or_default();
        let fetched = adapter.table_alternate_supply(&table).unwrap_or_default();
        let Ok(rows) = adapter.read_table(&table) else {
            continue;
        };
        for row in &rows {
            if query.row == Some((library, row.row_id)) {
                out.library = Some(name.to_string());
                let facts = RowFacts::read(row, adapter, other);
                out.alternates
                    .extend(row.alternates.iter().map(|alternate| {
                        let supply = AlternateSupply::of(alternate, fetched.get(&row.row_id));
                        Candidate {
                            source: CandidateSource::LibraryAlternate,
                            mpn: alternate.mpn.clone(),
                            manufacturer: alternate.manufacturer.clone(),
                            value: facts.value.clone(),
                            footprint: facts.footprint.clone(),
                            pins: facts.pins.clone(),
                            stock: supply.stock().map(u64::from),
                            status: CandidateStatus::Alternate(alternate.status),
                            supply: Some(supply),
                        }
                    }));
                continue;
            }
            if !row_matches(row, &query.query) || same_part(&row.primary_mpn.mpn, &query.base_mpn) {
                continue;
            }
            out.found_total += 1;
            if out.found.len() < FOUND_SHOWN {
                let facts = RowFacts::read(row, adapter, other);
                out.found.push(Candidate {
                    source: CandidateSource::Search,
                    mpn: row.primary_mpn.mpn.clone(),
                    manufacturer: row.primary_mpn.manufacturer.clone(),
                    value: facts.value,
                    footprint: facts.footprint,
                    pins: facts.pins,
                    stock: pricing.get(&row.row_id).and_then(stock_of),
                    status: CandidateStatus::Lifecycle(row.state),
                    supply: None,
                });
            }
        }
    }
}

/// Whether two part numbers name the same part (trimmed, any case).
fn same_part(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// What a row's part is: its value, footprint name and pins.
struct RowFacts {
    value: String,
    footprint: String,
    pins: Option<Vec<(String, String)>>,
}

impl RowFacts {
    fn read(
        row: &ComponentRow,
        adapter: &dyn LibraryAdapter,
        other: &dyn Fn(Uuid) -> Option<Box<dyn LibraryAdapter>>,
    ) -> Self {
        let own = adapter.library_id();
        let footprint = row
            .footprint_ref
            .and_then(|address| {
                if address.library_id == own {
                    adapter.get_footprint(address.uuid).ok()
                } else {
                    other(address.library_id)?.get_footprint(address.uuid).ok()
                }
            })
            .map(|footprint| footprint.name)
            .unwrap_or_default();
        let address = row.symbol_ref;
        let symbol = if address.library_id == own {
            adapter.get_symbol(address.uuid).ok()
        } else {
            other(address.library_id).and_then(|lib| lib.get_symbol(address.uuid).ok())
        };
        let pins = symbol.map(|symbol| {
            let mut pins: Vec<(String, String)> = symbol
                .pins
                .iter()
                .map(|pin| (pin.number.clone(), pin.name.clone()))
                .collect();
            pins.sort();
            pins.dedup();
            pins
        });
        Self {
            value: row_value(row),
            footprint,
            pins,
        }
    }
}

/// A row's value as the picker lists it: its `Value` parameter, else its
/// description.
pub fn row_value(row: &ComponentRow) -> String {
    row.parameters
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("value"))
        .map(|(_, value)| value.display())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| row.description.trim().to_string())
}

/// Whether `row` matches the search `query`: every word of it in the row's
/// part numbers, manufacturer, description, value or internal number, in
/// any case. An empty query matches nothing.
pub fn row_matches(row: &ComponentRow, query: &str) -> bool {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return false;
    }
    let mut haystack = vec![
        row.primary_mpn.mpn.to_lowercase(),
        row.primary_mpn.manufacturer.to_lowercase(),
        row.description.to_lowercase(),
        row.internal_pn.to_string().to_lowercase(),
        row_value(row).to_lowercase(),
    ];
    haystack.extend(row.alternates.iter().map(|alt| alt.mpn.to_lowercase()));
    words
        .iter()
        .all(|word| haystack.iter().any(|field| field.contains(word.as_str())))
}

/// Units in stock over a row's cached prices: the largest any listing
/// reports; `None` when none does.
pub fn stock_of(pricing: &RowPricing) -> Option<u64> {
    pricing
        .values()
        .flat_map(|by_sku| by_sku.values())
        .filter_map(|cached| cached.stock)
        .map(u64::from)
        .max()
}

/// How a candidate's pins compare with the placed part's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinCompare {
    /// The same numbers and names; `count` pins.
    Same { count: usize },
    /// Numbers or names differ.
    Differ { theirs: usize, ours: usize },
    /// One side's pins are not known.
    Unknown,
}

/// Compare `candidate`'s pins with `base` (each sorted, each once).
pub fn compare_pins(
    base: &[(String, String)],
    candidate: Option<&[(String, String)]>,
) -> PinCompare {
    match candidate {
        Some(theirs) if !base.is_empty() => {
            if theirs == base {
                PinCompare::Same { count: base.len() }
            } else {
                PinCompare::Differ {
                    theirs: theirs.len(),
                    ours: base.len(),
                }
            }
        }
        _ => PinCompare::Unknown,
    }
}

/// Whether two footprint names are the same footprint (trimmed, any case).
pub fn same_footprint(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// What the variant gets from `candidate` for a part with `base`'s value and
/// footprint: its part number, and its value and footprint where they
/// differ (`None` keeps the part's own).
pub fn alternate_values(
    candidate: &Candidate,
    base_value: &str,
    base_footprint: &str,
) -> (String, Option<String>, Option<String>) {
    let differs = |theirs: &str, ours: &str| {
        let theirs = theirs.trim();
        (!theirs.is_empty() && theirs != ours.trim()).then(|| theirs.to_string())
    };
    let footprint = (!same_footprint(&candidate.footprint, base_footprint))
        .then(|| differs(&candidate.footprint, base_footprint))
        .flatten();
    (
        candidate.mpn.trim().to_string(),
        differs(&candidate.value, base_value),
        footprint,
    )
}
