//! The power tree of a project (#1313, mockup row 171): sources, converters,
//! rails and loads, generated from the schematic, with the current each part
//! draws and a power budget per block.
//!
//! [`power_topology`] reads the power nets and the fitted parts on them off
//! the stitched project netlist; [`power_tree`] works out each rail's
//! sources, loads, total and limit, and each converter's input from its
//! outputs; [`sheet_budgets`] and [`block_budgets`] add the draws up per
//! sheet and per block. The numbers come from part parameters (`Rail Load
//! <net>`, `Rail Supply <net>`, `Efficiency`) and from the rail's voltage
//! (the project's rail list, else the net name). Rules:
//! `docs/POWER_TREE.md`.

mod budget;
mod quantity;
mod scenario;
mod topology;
mod tree;

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod quantity_tests;
#[cfg(test)]
mod scenario_tests;
#[cfg(test)]
mod topology_tests;
#[cfg(test)]
mod tree_tests;

pub use budget::{
    BlockBudget, BudgetLevel, FINE_BELOW, OVER_ABOVE, RailDraw, RailFeed, SheetBudget,
    block_budgets, sheet_budgets,
};
pub use quantity::{
    EFFICIENCY_PARAMETER, LOAD_PARAMETER, RailRole, SUPPLY_PARAMETER, is_ground_net,
    parse_current_a, parse_efficiency, rail_parameter, voltage_from_net_name,
};
pub use scenario::{MAX_LOAD_SCENARIO, ScenarioSummary, scenario_summary};
pub use topology::{PowerPart, PowerTopology, power_topology};
pub use tree::{
    Converter, CurrentFrom, ListedRail, PowerProblem, PowerRail, PowerTree, RailLoad, RailSource,
    RailVoltage, VoltageFrom, power_tree,
};
