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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

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
    /// Per sheet: the drifted `lib_id`s and the library's current symbol
    /// for each (what "Update from library" applies).
    pub sheets: Vec<(PathBuf, BTreeMap<String, LibSymbol>)>,
    /// `lib_id` -> `"<Library>/<Symbol>"` for every entry whose library is
    /// mounted and whose symbol was readable.
    pub names: HashMap<String, String>,
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
    let mut adapters: HashMap<Uuid, Result<LocalGitAdapter, String>> = HashMap::new();
    let mut report = DriftReport::default();
    // lib_id -> current (symbol, name); None = unreadable, already reported.
    let mut current: HashMap<String, Option<(LibSymbol, String)>> = HashMap::new();
    for (sheet, entries) in sheets {
        let mut drifted = BTreeMap::new();
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
                    .or_insert_with(|| LocalGitAdapter::open(&lib.path).map_err(|e| e.to_string()))
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|a| a.get_symbol(symbol_id).map_err(|e| e.to_string()));
                match read {
                    Ok(symbol) => {
                        let name = format!("{}/{}", lib.display_name, symbol.name);
                        Some((symbol.to_lib_symbol(lib_id.clone()), name))
                    }
                    Err(e) => {
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
                drifted.insert(lib_id.clone(), fresh.clone());
            }
        }
        report.sheets.push((sheet.clone(), drifted));
    }
    report
}

fn split_lib_id(lib_id: &str) -> Option<(Uuid, Uuid)> {
    let (lib, sym) = lib_id.split_once(':')?;
    Some((Uuid::parse_str(lib).ok()?, Uuid::parse_str(sym).ok()?))
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
    /// Per sheet: drifted `lib_id` -> the library's current symbol.
    pub drifted: HashMap<PathBuf, BTreeMap<String, LibSymbol>>,
    /// `lib_id` -> `"<Library>/<Symbol>"` where the library is mounted.
    pub names: HashMap<String, String>,
}

impl DriftState {
    pub fn is_drifted(&self, sheet: &Path, lib_id: &str) -> bool {
        self.drifted
            .get(sheet)
            .is_some_and(|d| d.contains_key(lib_id))
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
            netlist_sheets: vec![sheet],
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
