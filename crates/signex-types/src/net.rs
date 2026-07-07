use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Net identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetClassId(pub String);

// ---------------------------------------------------------------------------
// Net class
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetClass {
    pub name: String,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub trace_width: f64,
    #[serde(default)]
    pub via_diameter: f64,
    #[serde(default)]
    pub via_drill: f64,
    #[serde(default)]
    pub diff_pair_gap: f64,
    #[serde(default)]
    pub diff_pair_width: f64,
}

// ---------------------------------------------------------------------------
// Differential pair
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffPair {
    pub positive_net: String,
    pub negative_net: String,
    pub class: String,
}

// ---------------------------------------------------------------------------
// Netlist — the authoritative schematic-derived connectivity contract
// ---------------------------------------------------------------------------

/// One pin instance connected to a net: the placed symbol's `uuid`, its
/// reference designator (`R1`, `U3`), and the pin identifier (number or name).
///
/// `symbol` disambiguates terminals a bare reference string collapses —
/// unannotated `R?` and duplicate designators (the same refdes on two sheet
/// occurrences) — and links the terminal back to the placed symbol. `reference`
/// and `pin` stay for exporters and display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terminal {
    #[serde(default)]
    pub symbol: Uuid,
    pub reference: String,
    pub pin: String,
}

/// A logical net: a set of electrically-connected terminals derived from the
/// schematic (wires + junctions + labels + pins). `id` is a build-time stable
/// number (also usable as the PCB net number); `name` comes from the
/// highest-priority label on the net, or is auto-assigned when unlabelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Net {
    pub id: NetId,
    pub name: String,
    pub class: String,
    pub terminals: Vec<Terminal>,
}

/// The authoritative netlist: every net derived from a schematic. This is the
/// single connectivity source the net-flood UI, the ratsnest, PCB net
/// assignment, and the netlist exporter are meant to read — replacing the
/// ad-hoc union-find copies scattered across the app (ADR-0001 A3.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Netlist {
    pub nets: Vec<Net>,
}
