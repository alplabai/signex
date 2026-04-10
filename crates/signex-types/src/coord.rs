use std::fmt;
use std::ops::{Add, Neg, Sub};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core type -- everything stored in nanometers
// ---------------------------------------------------------------------------

pub type Coord = i64;

// ---------------------------------------------------------------------------
// Conversion constants
// ---------------------------------------------------------------------------

pub const NM_PER_MM: Coord = 1_000_000;
pub const NM_PER_MIL: Coord = 25_400;
pub const NM_PER_INCH: Coord = 25_400_000;
pub const NM_PER_UM: Coord = 1_000;

// ---------------------------------------------------------------------------
// Unit enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Mm,
    Mil,
    Inch,
    Micrometer,
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unit::Mm => write!(f, "mm"),
            Unit::Mil => write!(f, "mil"),
            Unit::Inch => write!(f, "in"),
            Unit::Micrometer => write!(f, "um"),
        }
    }
}

// ---------------------------------------------------------------------------
// Vec2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: Coord,
    pub y: Coord,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0, y: 0 };

    pub const fn new(x: Coord, y: Coord) -> Self {
        Vec2 { x, y }
    }
}

impl Add for Vec2 {
    type Output = Vec2;

    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Vec2;

    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Vec2 {
    type Output = Vec2;

    fn neg(self) -> Vec2 {
        Vec2 {
            x: -self.x,
            y: -self.y,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers -- from user units to nanometers
// ---------------------------------------------------------------------------

pub fn from_mm(v: f64) -> Coord {
    (v * NM_PER_MM as f64).round() as Coord
}

pub fn from_mil(v: f64) -> Coord {
    (v * NM_PER_MIL as f64).round() as Coord
}

pub fn from_inch(v: f64) -> Coord {
    (v * NM_PER_INCH as f64).round() as Coord
}

pub fn from_um(v: f64) -> Coord {
    (v * NM_PER_UM as f64).round() as Coord
}

/// KiCad stores coordinates as f64 millimeters -- convert to nanometers.
pub fn from_kicad_mm(v: f64) -> Coord {
    from_mm(v)
}

// ---------------------------------------------------------------------------
// Conversion helpers -- from nanometers to user units
// ---------------------------------------------------------------------------

pub fn to_mm(c: Coord) -> f64 {
    c as f64 / NM_PER_MM as f64
}

pub fn to_mil(c: Coord) -> f64 {
    c as f64 / NM_PER_MIL as f64
}

pub fn to_inch(c: Coord) -> f64 {
    c as f64 / NM_PER_INCH as f64
}

pub fn to_um(c: Coord) -> f64 {
    c as f64 / NM_PER_UM as f64
}

pub fn to_unit(c: Coord, unit: Unit) -> f64 {
    match unit {
        Unit::Mm => to_mm(c),
        Unit::Mil => to_mil(c),
        Unit::Inch => to_inch(c),
        Unit::Micrometer => to_um(c),
    }
}

// ---------------------------------------------------------------------------
// Grid presets (in nanometers)
// ---------------------------------------------------------------------------

/// Common mm-based grids: 0.1 mm, 0.25 mm, 0.5 mm, 1.0 mm, 2.5 mm, 5.0 mm
pub const GRID_MM: &[Coord] = &[
    100_000,   // 0.1 mm
    250_000,   // 0.25 mm
    500_000,   // 0.5 mm
    1_000_000, // 1.0 mm
    2_500_000, // 2.5 mm
    5_000_000, // 5.0 mm
];

/// Common mil-based grids: 1 mil, 5 mil, 10 mil, 25 mil, 50 mil, 100 mil
pub const GRID_MIL: &[Coord] = &[
    25_400,    // 1 mil
    127_000,   // 5 mil
    254_000,   // 10 mil
    635_000,   // 25 mil
    1_270_000, // 50 mil
    2_540_000, // 100 mil
];

/// Micrometer-based grids: 1 um, 5 um, 10 um, 25 um, 50 um, 100 um
pub const GRID_MICROMETER: &[Coord] = &[
    1_000,   // 1 um
    5_000,   // 5 um
    10_000,  // 10 um
    25_000,  // 25 um
    50_000,  // 50 um
    100_000, // 100 um
];
