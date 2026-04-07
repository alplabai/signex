// Document model — reserved for future native .sxsch format

use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentType {
    Schematic,
    Pcb,
    Library,
    OutputJob,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub doc_type: DocumentType,
    pub path: Option<String>,
    pub dirty: bool,
}
