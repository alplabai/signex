//! KiCad S-expression parser -- .kicad_sch, .kicad_pcb, .kicad_sym files.
#![allow(
    clippy::collapsible_if,
    clippy::redundant_closure,
    clippy::unnecessary_lazy_evaluations
)]

pub mod error;
pub mod pcb;
pub mod schematic;
pub mod sexpr;
pub mod symbol_lib;

pub use error::ParseError;
pub use pcb::{parse_pcb, parse_pcb_file};
pub use schematic::{parse_project, parse_schematic, parse_schematic_file};
pub use symbol_lib::parse_symbol_lib;
