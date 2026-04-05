// Document model — immutable with undo/redo stack (command pattern)
// Phase 1 will implement full editing

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentType {
    Schematic,
    Pcb,
    Library,
    OutputJob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub doc_type: DocumentType,
    pub path: Option<String>,
    pub dirty: bool,
}
