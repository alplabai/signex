//! The `Stock.snxlib` manifest that wraps the stock footprints.
//!
//! Emitted through [`LibraryFile::write`] like any user library, so the
//! file cannot drift from what the loader accepts. Every field is a
//! constant: no UUID is minted and no clock is read, so a rerun is
//! byte-identical (the #323 idempotency rule).

use std::collections::BTreeMap;

use signex_library::library_file::{
    ClassEntry, FORMAT_TOKEN, LibraryFile, LibraryFileError, LibrarySection, LibraryTable,
    SnxlibManifest,
};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig, WorkflowMode};
use signex_library::tables::TABLE_HEADER;
use uuid::Uuid;

use super::ids::STOCK_LIBRARY_ID;

/// File name of the library entry point inside `assets/stock-library/`.
pub const SNXLIB_FILE_NAME: &str = "Stock.snxlib";

/// Name of the component table. It ships with the full column schema
/// and zero rows; the pairing task fills the rows.
const COMPONENT_TABLE: &str = "components";

/// Class taxonomy for the New Component modal, scoped to what a
/// footprint-only stock library can usefully classify.
const STOCK_CLASSES: &[(&str, &str)] = &[
    ("resistor", "Resistor"),
    ("capacitor", "Capacitor"),
    ("inductor", "Inductor"),
    ("diode", "Diode"),
    ("led", "LED"),
    ("transistor", "Transistor"),
    ("ic", "Integrated Circuit"),
    ("connector", "Connector"),
    ("switch", "Switch"),
    ("crystal", "Crystal / Oscillator"),
    ("mechanical", "Mechanical"),
    ("fiducial", "Fiducial"),
    ("generic", "Generic"),
];

/// Assemble the stock library file: constant identity, `local_git`
/// mode, personal workflow, seeded classes, one empty component table.
pub fn stock_library_file() -> LibraryFile {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.to_string(),
        library_id: Uuid::parse_str(STOCK_LIBRARY_ID)
            .expect("stock library id constant is a well-formed UUID"),
        library: LibrarySection {
            name: "Stock".to_string(),
            description: Some("Reference footprints and symbols bundled with Signex.".to_string()),
        },
        mode: LibraryMode::LocalGit,
        workflow: WorkflowConfig {
            mode: WorkflowMode::Personal,
            ..WorkflowConfig::default()
        },
        users: UsersConfig::default(),
        classes: STOCK_CLASSES
            .iter()
            .map(|(key, label)| ClassEntry {
                key: (*key).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
    };
    let table = LibraryTable {
        columns: TABLE_HEADER.iter().map(|c| (*c).to_string()).collect(),
        rows: Vec::new(),
        column_types: BTreeMap::new(),
        pricing: Default::default(),
    };
    LibraryFile {
        layout: Default::default(),
        manifest,
        tables: BTreeMap::from([(COMPONENT_TABLE.to_string(), table)]),
    }
}

/// Render the `Stock.snxlib` text.
pub fn render_manifest() -> Result<String, LibraryFileError> {
    stock_library_file().write()
}
