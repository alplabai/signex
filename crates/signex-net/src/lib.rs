//! Schematic connectivity → the authoritative [`Netlist`].
//!
//! One place derives electrical nets from a `SchematicSheet` (union-find
//! over wires + junctions, named by labels, terminated at component pins).
//! ERC, the net-flood UI, the ratsnest, PCB net assignment, and the netlist
//! exporter are all meant to read this instead of hand-rolling their own
//! union-find (ADR-0001 A3.1). The union-find primitive itself lives in
//! [`uf`], shared with `signex-erc`.
//!
//! [`Netlist`]: signex_types::net::Netlist

pub mod uf;

mod build;
pub use build::build_netlist;
