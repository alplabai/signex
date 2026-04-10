//! ERC and DRC violation types.

use serde::{Deserialize, Serialize};

use crate::schematic::Point;

// ─── ERC (Electrical Rules Check) ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErcViolationType {
    PinConflict,
    UnconnectedPin,
    DuplicateReference,
    MissingValue,
    MissingFootprint,
    MultiplePowerFlags,
    NoPowerFlag,
    DuplicateNetName,
    WireDangling,
    LabelDangling,
    BusConflict,
}

// ─── DRC (Design Rules Check) ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrcViolationType {
    Clearance,
    ShortCircuit,
    UnroutedNet,
    MinTrackWidth,
    MinViaDiameter,
    MinViaDrill,
    MinHoleToHole,
    MinAnnularRing,
    MinDrill,
    BoardOutlineClearance,
    SilkToMask,
    SilkToSilk,
    AcuteAngle,
    AcidTrap,
    CopperSliver,
}

// ─── Severity ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

// ─── Violation ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationType {
    Erc(ErcViolationType),
    Drc(DrcViolationType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub violation_type: ViolationType,
    pub severity: Severity,
    pub message: String,
    pub location: Point,
    /// UUIDs or references of the objects involved in this violation.
    #[serde(default)]
    pub objects: Vec<String>,
}
