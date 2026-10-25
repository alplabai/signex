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
mod bus;
pub use build::{
    ConnectedPoints, FloodElements, SheetConnectivity, SheetPinContacts, best_net_name_of,
    build_netlist, connected_points, flood_net_elements, is_power_flag, point_on_segment, pt_key,
};
pub use bus::{BusAnalysis, BusGeometry, BusGroup, BusLabelName, EntryAttachment, analyse_buses};

mod signature;
pub use signature::{NetSignature, net_signatures, net_signatures_at};

mod project;
pub use project::{
    ProjectGraph, ProjectNetlist, ProjectRoot, SheetKey, StitchIssue, build_project_netlist,
    sheet_placements,
};
