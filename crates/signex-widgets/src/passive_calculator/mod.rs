//! Preferred-value resistor, capacitor, and inductor network calculator.

pub mod color_code;
mod color_code_view;
pub mod component_card;
pub mod control;
pub mod domain;
pub mod network;
pub mod production_date_code;
pub mod rkm_code;
pub mod rkm_encoder;
pub mod solver;

pub use color_code::{BandColor, ComponentColorCode, ResistorColorCode};
pub use component_card::{ComponentCard, ComponentCardMessage};
pub use control::{CalculatorControl, CalculatorMessage, CalculatorTab};
pub use domain::{
    ComponentKind, ESeries, PreferredComponent, PreferredNumber, SiPrefix, Tolerance,
};
pub use network::{BoundaryCondition, Connection, Network};
pub use production_date_code::{ProductionDateCode, ProductionDateCycle, ProductionMonth};
pub use rkm_code::{RatedPower, RkmCode, TemperatureCoefficient};
pub use rkm_encoder::{RkmEncoder, RkmEncoderMessage};
pub use solver::{MAX_PARTS, SolveOptions, solve};
