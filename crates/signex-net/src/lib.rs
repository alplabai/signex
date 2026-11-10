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

pub mod board;

mod net_class;
pub use net_class::{
    assign_net_classes, element_class_styles, element_net_names, placement_class_styles,
    placement_net_names, wire_at_point,
};

mod build;
mod bus;
mod bus_ends;
pub use bus_ends::dangling_bus_ends;
mod bus_intent;
pub use bus_intent::{BusExtra, BusExtraKind, BusIntent, bus_intent, bus_reaches_other_sheets};
#[cfg(test)]
mod fixtures;
mod join_rule;
pub use build::{
    ConnectedPoints, EndSpec, ExtraItem, ExtraKind, FloodElements, IntentNet, NameChange,
    NetExtent, PlacementIntent, RunEnd, SegmentIndex, SheetConnectivity, SheetPinContacts,
    WireIntent, best_net_name_of, build_netlist, connected_net_items, connected_points,
    flood_net_elements, has_connection_anchor, is_power_flag, name_carrying_ports, net_extent_at,
    net_reaches_other_sheets, placement_intent, point_on_segment, pt_key, wire_intent,
};
pub use bus::{BusAnalysis, BusGeometry, BusGroup, BusLabelName, EntryAttachment, analyse_buses};

mod incremental;
pub use incremental::{NetBase, NetDelta, point_is_occupied};

mod signature;
pub use signature::{NetSignature, net_members, net_signatures, net_signatures_at};

mod project;
pub use project::{
    HoverPin, MultiNamedNet, NetAnchor, NetHover, NetIndex, NetMember, NetMemberKind, NetMembers,
    NetNameSource, NumberingIssue, PagePlacement, ProjectGraph, ProjectNetChange, ProjectNetlist,
    ProjectRoot, SheetKey, SheetNumbering, SheetOccurrence, SheetPage, StitchIssue,
    build_project_netlist, build_project_netlist_indexed, instance_entry_designators, is_auto_name,
    multi_named_nets, number_issues, parse_page_number, project_name_changes, project_occurrences,
    sheet_numbering, with_page_number,
};
