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

mod parts;
pub use parts::{DesignPart, design_parts, design_parts_named};

pub mod cross_probe;

mod directives;
pub use directives::{
    BlanketCatch, BlanketNet, ClassAssignment, NetClassConflict, NetParametersOn, blanket_nets,
    class_assignments, directive_net_classes, fill_pattern_classes, net_class_conflicts,
    net_parameters,
};
pub use directives::{DiffPair, blanket_net_classes, compile_masked, diff_pairs};
mod project_classes;
pub use project_classes::{
    ProjectClassConflicts, ProjectNetClassConflict, SheetClassAssignment,
    project_net_class_conflicts,
};
mod diff_pair_classes;
pub use diff_pair_classes::{
    ClassMember, class_members, diff_pairs_with_classes, pair_count, pairs_by_every_suffix,
};

mod net_class;
pub use net_class::{
    assign_net_classes, element_class_styles, element_net_names, netlist_has_net,
    placement_class_styles, placement_net_names, wire_at_point,
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
    flood_net_elements, has_connection_anchor, is_connection_kind, is_power_flag,
    logical_connectivity, name_carrying_ports, name_carrying_tips, net_extent_at,
    net_reaches_other_sheets, placement_contacts, placement_intent, point_on_segment, pt_key,
    wire_intent,
};
pub use bus::{BusAnalysis, BusGeometry, BusGroup, BusLabelName, EntryAttachment, analyse_buses};

mod incremental;
pub use incremental::{NetBase, NetDelta, point_is_occupied};

mod signature;
pub use signature::{NetSignature, net_members, net_signatures, net_signatures_at};

mod project;
pub use project::{
    ChannelDesignators, EntryBuses, build_project_netlist_indexed_named,
    build_project_netlist_named, channel_uuid, multi_named_nets_named,
    project_name_changes_as_written, project_name_changes_named,
};
pub use project::{
    ComponentPin, ComponentReport, HoverPin, MultiNamedNet, NetAnchor, NetHover, NetIndex,
    NetMember, NetMemberKind, NetMembers, NetNameSource, NumberingIssue, PagePlacement, PlacedPin,
    PlacedSymbol, ProjectGraph, ProjectNetChange, ProjectNetlist, ProjectRoot, SheetKey,
    SheetNumbering, SheetOccurrence, SheetPage, StitchIssue, UnconnectedPin, build_project_netlist,
    build_project_netlist_indexed, instance_entry_designators, is_auto_name, multi_named_nets,
    number_issues, parse_page_number, project_name_changes, project_occurrences, sheet_numbering,
    with_page_number,
};

mod compiled;
#[cfg(test)]
mod compiled_tests;
pub use compiled::{
    CompiledDesign, CompiledNet, CompiledPart, CompiledSheet, CompiledSheetNet, CompiledTarget,
    SheetTitle, compile_design,
};

mod miniature;
#[cfg(test)]
mod miniature_tests;
pub use miniature::{MiniatureText, SheetMiniature, sheet_miniature};

mod harness;
#[cfg(test)]
mod harness_fixture;
pub use harness::{
    HarnessBundle, HarnessLeaf, HarnessMismatch, HarnessPort, HarnessSheetEntry, harness_bundles,
    harness_mismatches, harness_segments_touch, on_harness,
};

mod names_in_use;
pub use names_in_use::sheet_net_names;

#[cfg(test)]
mod net_tie_tests;
