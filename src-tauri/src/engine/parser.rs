// KiCad S-expression parser — Phase 0, Week 2
// Will parse .kicad_sch, .kicad_pcb, .kicad_pro files

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchematicSheet {
    pub title: String,
    pub paper_size: String,
    pub symbols: Vec<Symbol>,
    pub wires: Vec<Wire>,
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub lib_id: String,
    pub reference: String,
    pub value: String,
    pub position: Point,
    pub rotation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wire {
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub text: String,
    pub position: Point,
    pub label_type: LabelType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LabelType {
    Net,
    Global,
    Hierarchical,
    Power,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
