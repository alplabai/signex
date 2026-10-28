//! Library symbol drift (#912): has the library's symbol changed since the
//! sheet embedded its copy?
//!
//! A sheet keeps its own `LibSymbol` per `"<library uuid>:<symbol uuid>"`
//! key. Drift is decided by comparing a content hash of that embedded copy
//! with the hash of the library's current symbol converted the same way
//! (`Symbol::to_lib_symbol`). Both sides are computed on demand: no hash is
//! stored in the `.snxsch`, so there is no format change and old files need
//! nothing. The embedded copies are stored as TOML (`[extras.sheet]`) and
//! round-trip through save/load, so an unchanged library never reads as
//! drifted (pinned by a save-and-reopen regression test).
//!
//! [`compute_drift`] does the IO and runs in `spawn_blocking`; the
//! `update`-side half is `app/handlers/library_drift.rs`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use signex_library::LibraryError;
use signex_types::schematic::LibSymbol;
use uuid::Uuid;

/// Stable content hash of a `LibSymbol` (FNV-1a 64 over the serde JSON of
/// the in-memory value, whose field order is the struct's declaration
/// order; the JSON is only a hash input, never stored). `LibSymbol` has no
/// `PartialEq`, and `std`'s `DefaultHasher` is not stable across releases.
pub fn content_hash(symbol: &LibSymbol) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let json = serde_json::to_vec(symbol).unwrap_or_default();
    json.iter()
        .fold(OFFSET, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME))
}

/// A mounted library the worker may read.
#[derive(Debug, Clone)]
pub struct MountedLibrary {
    pub id: Uuid,
    pub path: PathBuf,
    pub display_name: String,
}

/// What one drift pass found. Sheets with no drifted entry are listed with
/// an empty map so a cleared drift replaces the previous result.
#[derive(Debug, Clone, Default)]
pub struct DriftReport {
    /// Per sheet: the `lib_id`s whose embedded copy differs from the
    /// library's current symbol.
    pub sheets: Vec<(PathBuf, BTreeSet<String>)>,
    /// `lib_id` -> `"<Library>/<Symbol>"` for every entry whose library is
    /// mounted and whose symbol was readable.
    pub names: HashMap<String, String>,
    /// `lib_id`s whose library is mounted but whose current symbol could not
    /// be read this pass. Their previous rows are kept, not cleared.
    pub unreadable: HashSet<String>,
    /// Failures to surface in the Messages panel.
    pub problems: Vec<String>,
    /// The [`DriftState::generation`] this pass was started under; a report
    /// whose generation is no longer current is stale and dropped.
    pub generation: u64,
}

/// One sheet's embedded entries: `(sheet path, [(lib_id, embedded copy)])`.
/// The copies are cloned on the UI thread and hashed in the worker.
pub type SheetEntries = (PathBuf, Vec<(String, LibSymbol)>);

/// One requested pass, ready to run off the UI thread.
#[derive(Debug)]
pub struct DriftJob {
    pub generation: u64,
    pub sheets: Vec<SheetEntries>,
    pub libs: Vec<MountedLibrary>,
}

impl DriftJob {
    /// Do the IO and hashing; the report carries this job's generation.
    pub fn run(self) -> DriftReport {
        DriftReport {
            generation: self.generation,
            ..compute_drift(&self.sheets, &self.libs)
        }
    }
}

/// Compare every embedded entry with its mounted library's current symbol.
/// Entries whose library is not mounted are skipped (nothing to compare).
pub fn compute_drift(sheets: &[SheetEntries], libs: &[MountedLibrary]) -> DriftReport {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    // One adapter per library per pass (`Err` = open failed, reported once).
    let mut adapters: HashMap<Uuid, Result<LocalGitAdapter, LibraryError>> = HashMap::new();
    let mut report = DriftReport::default();
    // lib_id -> current (symbol, name); None = unreadable, already reported.
    let mut current: HashMap<String, Option<(LibSymbol, String)>> = HashMap::new();
    for (sheet, entries) in sheets {
        let mut drifted = BTreeSet::new();
        for (lib_id, embedded) in entries {
            let Some((library_id, symbol_id)) = split_lib_id(lib_id) else {
                continue;
            };
            let Some(lib) = libs.iter().find(|l| l.id == library_id) else {
                continue;
            };
            let entry = current.entry(lib_id.clone()).or_insert_with(|| {
                let read = adapters
                    .entry(lib.id)
                    .or_insert_with(|| LocalGitAdapter::open(&lib.path))
                    .as_ref()
                    .map_err(|e| LibraryError::Backend(e.to_string()))
                    .and_then(|a| a.get_symbol(symbol_id));
                match read {
                    Ok(symbol) => {
                        let name = format!("{}/{}", lib.display_name, symbol.name);
                        Some((symbol.to_lib_symbol(lib_id.clone()), name))
                    }
                    Err(e) => {
                        // A symbol the library no longer has is a real
                        // removal (its row goes); anything else is a failed
                        // read, which says nothing about drift.
                        if !matches!(e, LibraryError::NotFound(_)) {
                            report.unreadable.insert(lib_id.clone());
                        }
                        report.problems.push(format!(
                            "Could not check library symbol {lib_id} in {}: {e}",
                            lib.display_name
                        ));
                        None
                    }
                }
            });
            let Some((fresh, name)) = entry else { continue };
            report.names.insert(lib_id.clone(), name.clone());
            if content_hash(fresh) != content_hash(embedded) {
                drifted.insert(lib_id.clone());
            }
        }
        report.sheets.push((sheet.clone(), drifted));
    }
    report
}

/// Whether the row still offers the symbol `lib_id` names, as its default
/// or as an alternate. A row re-bound to other symbols must not write
/// another symbol into this `lib_id`'s shared embedded copy.
pub(crate) fn row_binds_lib_id(row: &signex_library::ComponentRow, lib_id: &str) -> bool {
    split_lib_id(lib_id).is_some_and(|(lib, sym)| {
        row.binds_symbol(&signex_library::primitive::PrimitiveRef::new(lib, sym))
    })
}

/// What happens to an instance whose row is re-bound: its version is not
/// stamped (whether the shared copy is refreshed is decided elsewhere).
pub(crate) const VERSION_NOT_STAMPED: &str = "version not stamped";

/// The Messages line for an instance whose library row now points at a
/// different symbol than the one it embeds. The outcome comes before the
/// long `lib_id` because the Messages panel cuts a line at 160 bytes.
pub(crate) fn rebound_row_warning(
    reference: &str,
    row_id: impl std::fmt::Display,
    lib_id: &str,
    outcome: &str,
) -> String {
    format!("{reference}: row {row_id} binds a different symbol; {outcome} ({lib_id}).")
}

/// One instance the worker stamps: its row is read by the worker, fresh,
/// from the library the row itself lives in.
#[derive(Debug, Clone)]
pub struct ApplyStamp {
    pub symbol: Uuid,
    pub reference: String,
    pub row_id: Uuid,
    /// The library the row lives in. It can differ from the library of the
    /// `lib_id`'s symbol: a row may bind a symbol of another library.
    pub library_id: Uuid,
    /// The row version the user approved in the modal. A row read at another
    /// version has moved since: the instance is not applied. `None` (Update
    /// from library) stamps whatever was read.
    pub approved_version: Option<String>,
}

/// Everything the off-thread apply read needs, built on the UI thread from
/// the Library Updates modal or from "Update from library".
#[derive(Debug, Clone)]
pub struct ApplyInput {
    pub sheet: PathBuf,
    /// [`signex_engine::Engine::instance_id`] of the sheet when the input
    /// was built.
    pub engine_instance: u64,
    pub any_unchecked: bool,
    pub scan_failed: bool,
    /// Whether the completion records the outcome in the sheet's
    /// `skipped_updates_for` (the modal does; "Update from library" updates
    /// one `lib_id` and says nothing about the others).
    pub tracks_pending: bool,
    /// Per `lib_id`, the instances to stamp.
    pub wanted: Vec<(String, Vec<ApplyStamp>)>,
}

/// Every instance of `lib_id` on `doc` that has a library row, as worker
/// stamps. `approved` maps instances the user approved to that version.
pub(crate) fn instance_stamps(
    doc: &signex_types::schematic::SchematicSheet,
    lib_id: &str,
    approved: &HashMap<Uuid, String>,
) -> Vec<ApplyStamp> {
    doc.symbols
        .iter()
        .filter(|s| s.lib_id == lib_id)
        .filter_map(|s| {
            Some(ApplyStamp {
                symbol: s.uuid,
                reference: s.reference.clone(),
                row_id: s.row_id?,
                library_id: s.library_id?,
                approved_version: approved.get(&s.uuid).cloned(),
            })
        })
        .collect()
}

/// The worker input for the modal's current selection; `None` when nothing
/// is selected. The copy is shared, so every instance of a selected `lib_id`
/// on `doc` is included, not only the ticked rows.
pub fn apply_input(
    state: &super::updates_dialog::LibraryUpdatesState,
    doc: &signex_types::schematic::SchematicSheet,
) -> Option<ApplyInput> {
    let selected: Vec<_> = state.entries.iter().filter(|e| e.selected).collect();
    let lib_ids: BTreeSet<&str> = selected.iter().map(|e| e.lib_id.as_str()).collect();
    if lib_ids.is_empty() {
        return None;
    }
    let approved: HashMap<Uuid, String> = selected
        .iter()
        .map(|e| (e.symbol_uuid, e.latest_version.clone()))
        .collect();
    Some(ApplyInput {
        sheet: state.schematic_path.clone(),
        engine_instance: state.engine_instance,
        any_unchecked: state.entries.iter().any(|e| !e.selected),
        scan_failed: state.scan_failed,
        tracks_pending: true,
        wanted: lib_ids
            .into_iter()
            .map(|id| (id.to_string(), instance_stamps(doc, id, &approved)))
            .collect(),
    })
}

/// One `lib_id`'s fresh library symbol (or why it could not be read) and the
/// `(placed symbol, row version)` stamps that go with it.
#[derive(Debug, Clone)]
pub struct SymbolRead {
    pub lib_id: String,
    pub symbol: Result<LibSymbol, String>,
    pub stamps: Vec<(Uuid, String)>,
    /// One line per instance whose version is not stamped, saying why.
    pub skipped: Vec<String>,
    /// An approved instance's row moved on since the approval: the copy and
    /// every stamp of this `lib_id` are held back.
    pub held_back: bool,
}

/// What the off-thread read for an apply hands back to `update`.
#[derive(Debug, Clone)]
pub struct SymbolsRead {
    /// The sheet the input was built for.
    pub sheet: PathBuf,
    /// The sheet's engine instance the input was built against; a
    /// completion for another instance (closed and reopened) is dropped.
    pub engine_instance: u64,
    /// Some entries were left unchecked: the sheet keeps its pending update.
    pub any_unchecked: bool,
    /// The open-time scan could not check or update some symbol.
    pub scan_failed: bool,
    /// See [`ApplyInput::tracks_pending`].
    pub tracks_pending: bool,
    /// The worker itself failed (panicked); logged on completion.
    pub problem: Option<String>,
    pub reads: Vec<SymbolRead>,
}

impl SymbolsRead {
    /// The result for a worker that produced nothing.
    pub fn failed(input: &ApplyInput, problem: String) -> Self {
        Self {
            sheet: input.sheet.clone(),
            engine_instance: input.engine_instance,
            any_unchecked: true,
            scan_failed: true,
            tracks_pending: input.tracks_pending,
            problem: Some(problem),
            reads: Vec::new(),
        }
    }
}

/// A library opened once per read, with its table list.
struct OpenedLibrary {
    adapter: signex_library::adapters::local_git::LocalGitAdapter,
    tables: Result<Vec<String>, String>,
}

type Opened = Result<OpenedLibrary, String>;

fn open_library(libs: &[MountedLibrary], id: Uuid) -> Opened {
    use signex_library::LibraryAdapter as _;
    let lib = libs
        .iter()
        .find(|l| l.id == id)
        .ok_or_else(|| "its library is not open".to_string())?;
    let adapter = signex_library::adapters::local_git::LocalGitAdapter::open(&lib.path)
        .map_err(|e| e.to_string())?;
    let tables = adapter.list_tables().map_err(|e| e.to_string());
    Ok(OpenedLibrary { adapter, tables })
}

/// What a fresh read of one row found.
enum RowRead {
    Found(Box<signex_library::ComponentRow>),
    Missing,
    Failed(String),
}

/// Look for `row_id` in each table of the opened library. The table name
/// isn't stored on the schematic side. A table that fails to read is not
/// "row missing": it is reported, unless a later table holds the row.
fn read_row(opened: &OpenedLibrary, row_id: Uuid) -> RowRead {
    use signex_library::LibraryAdapter as _;
    let tables = match &opened.tables {
        Ok(t) => t,
        Err(e) => return RowRead::Failed(format!("its tables could not be listed: {e}")),
    };
    let id = signex_library::RowId::from_uuid(row_id);
    let mut failure = None;
    for table in tables {
        match opened.adapter.read_row(table, id) {
            Ok(row) => return RowRead::Found(Box::new(row)),
            Err(signex_library::adapter::LibraryError::NotFound(_)) => {}
            Err(e) => failure = failure.or(Some(e.to_string())),
        }
    }
    failure.map_or(RowRead::Missing, RowRead::Failed)
}

/// Why an instance got no stamp.
enum NoStamp {
    /// Its row is unreadable, missing or re-bound.
    Unreadable(String),
    /// Its row moved past the version the user approved: "U1 moved to X since you approved Y".
    Moved(String),
}

/// The stamp for one instance, or the Messages line saying why it has none.
fn stamp_or_reason(
    w: &ApplyStamp,
    lib_id: &str,
    libraries: &mut HashMap<Uuid, Opened>,
    libs: &[MountedLibrary],
) -> Result<(Uuid, String), NoStamp> {
    let nothing = VERSION_NOT_STAMPED;
    let (reference, row) = (&w.reference, w.row_id);
    let opened = libraries
        .entry(w.library_id)
        .or_insert_with(|| open_library(libs, w.library_id));
    let opened = opened.as_ref().map_err(|e| {
        NoStamp::Unreadable(format!(
            "{reference}: {nothing}: the library of row {row} is unreadable ({e}) ({lib_id})."
        ))
    })?;
    let why = NoStamp::Unreadable;
    match read_row(opened, w.row_id) {
        RowRead::Found(r) if row_binds_lib_id(&r, lib_id) => match &w.approved_version {
            Some(approved) if *approved != r.version => Err(NoStamp::Moved(format!(
                "{reference} moved to {} since you approved {approved}",
                r.version
            ))),
            _ => Ok((w.symbol, r.version)),
        },
        RowRead::Found(_) => Err(why(rebound_row_warning(reference, row, lib_id, nothing))),
        RowRead::Missing => Err(why(format!(
            "{reference}: {nothing}: row {row} is not in its library ({lib_id})."
        ))),
        RowRead::Failed(e) => Err(why(format!(
            "{reference}: {nothing}: row {row} could not be read ({e}) ({lib_id})."
        ))),
    }
}

/// What the stamp decisions of one `lib_id` came to.
struct Decided {
    stamps: Vec<(Uuid, String)>,
    skipped: Vec<String>,
    /// "U1 moved to X since you approved Y" per approved instance whose row
    /// moved (then nothing is stamped).
    moved: Vec<String>,
    /// References of the other instances, left on the old copy by a hold-back.
    left: Vec<String>,
}

/// The stamps and Messages lines for one `lib_id`'s instances.
fn decide_stamps(
    wanted: &[ApplyStamp],
    lib_id: &str,
    libraries: &mut HashMap<Uuid, Opened>,
    libs: &[MountedLibrary],
) -> Decided {
    let mut d = Decided {
        stamps: Vec::new(),
        skipped: Vec::new(),
        moved: Vec::new(),
        left: Vec::new(),
    };
    for w in wanted {
        match stamp_or_reason(w, lib_id, libraries, libs) {
            Ok(stamp) => {
                d.stamps.push(stamp);
                d.left.push(w.reference.clone());
            }
            Err(NoStamp::Unreadable(why)) => {
                d.skipped.push(why);
                d.left.push(w.reference.clone());
            }
            Err(NoStamp::Moved(what)) => d.moved.push(what),
        }
    }
    if d.moved.is_empty() {
        d.left.clear();
    } else {
        d.stamps.clear();
    }
    d
}

/// The one Messages line for a held-back `lib_id`: none of it was applied.
fn held_back_summary(name: &str, moved: &[String], left: &[String]) -> String {
    let moved = moved.join(", ");
    if left.is_empty() {
        format!("{name}: not applied ({moved}).")
    } else {
        format!(
            "{name}: not applied ({moved}); {} left on the old copy.",
            left.join(", ")
        )
    }
}

/// Read each wanted `lib_id`'s current symbol and its instances' row
/// versions from the mounted libraries, as of now; runs in `spawn_blocking`.
/// Like the drift pass it opens each library from its path, so no adapter is
/// shared with the UI thread. A row is read through the library it lives in
/// (`ApplyStamp::library_id`), not the symbol's. Each instance is decided
/// from its fresh row: bound to the `lib_id` -> stamped; anything else -> a
/// line saying why, and no stamp. The shared copy is refreshed regardless,
/// except when an approved instance's row moved on since the approval: then
/// the whole `lib_id` is held back, with one summary line.
pub fn read_symbols(input: ApplyInput, libs: &[MountedLibrary]) -> SymbolsRead {
    use signex_library::LibraryAdapter as _;

    let mut libraries: HashMap<Uuid, Opened> = HashMap::new();
    let reads = input
        .wanted
        .iter()
        .map(|(lib_id, wanted)| {
            let mut decided = decide_stamps(wanted, lib_id, &mut libraries, libs);
            let read = split_lib_id(lib_id)
                .ok_or_else(|| "its id is not <library>:<symbol>".to_string())
                .and_then(|(library, symbol_id)| {
                    let opened = libraries
                        .entry(library)
                        .or_insert_with(|| open_library(libs, library))
                        .as_ref()
                        .map_err(Clone::clone)?;
                    let symbol = opened
                        .adapter
                        .get_symbol(symbol_id)
                        .map_err(|e| e.to_string())?;
                    let shown = libs.iter().find(|l| l.id == library).map_or_else(
                        || symbol.name.clone(),
                        |l| format!("{}/{}", l.display_name, symbol.name),
                    );
                    Ok((shown, symbol.to_lib_symbol(lib_id.clone())))
                });
            let held_back = !decided.moved.is_empty();
            if held_back {
                let name = read.as_ref().map_or(lib_id.as_str(), |(n, _)| n.as_str());
                let line = held_back_summary(name, &decided.moved, &decided.left);
                decided.skipped.push(line);
            }
            SymbolRead {
                lib_id: lib_id.clone(),
                symbol: read.map(|(_, s)| s),
                stamps: decided.stamps,
                skipped: decided.skipped,
                held_back,
            }
        })
        .collect();
    SymbolsRead {
        sheet: input.sheet,
        engine_instance: input.engine_instance,
        any_unchecked: input.any_unchecked,
        scan_failed: input.scan_failed,
        tracks_pending: input.tracks_pending,
        problem: None,
        reads,
    }
}

pub(crate) fn split_lib_id(lib_id: &str) -> Option<(Uuid, Uuid)> {
    let (lib, sym) = lib_id.split_once(':')?;
    Some((Uuid::parse_str(lib).ok()?, Uuid::parse_str(sym).ok()?))
}

/// The library uuid a `"<library uuid>:<symbol uuid>"` key belongs to.
pub(crate) fn library_of(lib_id: &str) -> Option<Uuid> {
    split_lib_id(lib_id).map(|(library, _)| library)
}

/// Stamp the readable `Library/Symbol` name as the `LibRef` field of every
/// placed symbol that has no explicit `LibRef` / `Part Number`, so the BOM's
/// LibRef column shows names instead of raw `uuid:uuid` ids (#912). Symbols
/// whose library is not mounted keep the raw id.
pub fn apply_library_names(
    ctx: &mut signex_output::ExportContext,
    names: &HashMap<String, String>,
) {
    let sheets = ctx
        .sheets
        .iter_mut()
        .map(|s| &mut s.schematic)
        .chain(ctx.netlist_sheets.iter_mut());
    for sheet in sheets {
        for sym in &mut sheet.symbols {
            let Some(name) = names.get(&sym.lib_id) else {
                continue;
            };
            let has = |k: &str| sym.fields.get(k).is_some_and(|v| !v.trim().is_empty());
            if !has("LibRef") && !has("Part Number") {
                sym.fields.insert("LibRef".into(), name.clone());
            }
        }
    }
}

/// Latest drift results, kept on [`super::state::LibraryState`].
#[derive(Debug, Default)]
pub struct DriftState {
    /// A recheck is wanted; drained by `finish_update` into a worker task.
    pub check_pending: bool,
    /// Bumped each time a pass is started (or the mounted set changes with
    /// nothing to check); only the report carrying the current value is
    /// stored, so an out-of-order worker result cannot overwrite a newer one.
    pub generation: u64,
    /// Per sheet: the `lib_id`s whose embedded copy has drifted.
    pub drifted: HashMap<PathBuf, BTreeSet<String>>,
    /// `lib_id` -> `"<Library>/<Symbol>"` where the library is mounted.
    pub names: HashMap<String, String>,
}

impl DriftState {
    pub fn is_drifted(&self, sheet: &Path, lib_id: &str) -> bool {
        self.drifted.get(sheet).is_some_and(|d| d.contains(lib_id))
    }

    /// Human-readable `Library/Symbol`, or the raw id when the library is
    /// not mounted.
    pub fn display_name<'a>(&'a self, lib_id: &'a str) -> &'a str {
        self.names.get(lib_id).map_or(lib_id, String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(description: &str) -> LibSymbol {
        LibSymbol {
            id: "L:S".into(),
            reference: "U".into(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: description.into(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: Vec::new(),
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        }
    }

    #[test]
    fn hash_is_stable_and_content_sensitive() {
        assert_eq!(content_hash(&sym("a")), content_hash(&sym("a")));
        assert_ne!(content_hash(&sym("a")), content_hash(&sym("b")));
    }

    #[test]
    fn bom_libref_gets_names_only_for_mounted_libraries() {
        let mut sheet = signex_types::schematic::SchematicSheet::default();
        for lib_id in ["L:S", "X:Y"] {
            sheet.symbols.push(signex_types::schematic::Symbol {
                lib_id: lib_id.into(),
                ..Default::default()
            });
        }
        let mut explicit = signex_types::schematic::Symbol {
            lib_id: "L:S".into(),
            ..Default::default()
        };
        explicit.fields.insert("LibRef".into(), "Mine".into());
        sheet.symbols.push(explicit);
        let mut ctx = signex_output::ExportContext {
            sheets: Vec::new(),
            metadata: Default::default(),
            netlist: None,
            netlist_placements: Vec::new(),
            netlist_sheets: vec![sheet],
            current_sheet: None,
        };
        let names = HashMap::from([("L:S".to_string(), "Passives/R".to_string())]);
        apply_library_names(&mut ctx, &names);
        let syms = &ctx.netlist_sheets[0].symbols;
        assert_eq!(syms[0].fields["LibRef"], "Passives/R");
        assert!(!syms[1].fields.contains_key("LibRef"));
        assert_eq!(syms[2].fields["LibRef"], "Mine");
    }

    #[test]
    fn display_name_falls_back_to_the_raw_id() {
        let mut state = DriftState::default();
        state.names.insert("L:S".into(), "Passives/R".into());
        assert_eq!(state.display_name("L:S"), "Passives/R");
        assert_eq!(state.display_name("X:Y"), "X:Y");
    }
}
