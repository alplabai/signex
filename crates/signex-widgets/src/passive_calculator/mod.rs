//! Preferred-value resistor, capacitor, and inductor network calculator.

pub mod color_code;
pub mod control;
pub mod domain;
pub mod network;
pub mod solver;

pub use color_code::{BandColor, ComponentColorCode, ResistorColorCode};
pub use control::{CalculatorControl, CalculatorMessage};
pub use domain::{
    ComponentKind, ESeries, PreferredComponent, PreferredNumber, SiPrefix, Tolerance,
};
pub use network::{BoundaryCondition, Connection, Network};
pub use solver::{MAX_PARTS, SolveOptions, solve};
