use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Document type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Schematic,
    Pcb,
    Library,
    OutputJob,
}

// ---------------------------------------------------------------------------
// Document handle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub name: String,
    pub doc_type: DocumentType,
    pub path: String,
    #[serde(default)]
    pub dirty: bool,
}

// ---------------------------------------------------------------------------
// Sheet entry (summary row for the project tree)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetEntry {
    pub name: String,
    pub filename: String,
    #[serde(default)]
    pub symbols_count: usize,
    #[serde(default)]
    pub wires_count: usize,
    #[serde(default)]
    pub labels_count: usize,
}

// ---------------------------------------------------------------------------
// Project data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub name: String,
    pub dir: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    #[serde(default)]
    pub sheets: Vec<SheetEntry>,
}
