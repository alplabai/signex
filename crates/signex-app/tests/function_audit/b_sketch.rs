//! Function audit, wave B: mockup rows m39, m110, m124 to m126, m140 to m142,
//! m179, m179b, m179d, m179e and m187 to m192 (History panel, board sketch,
//! footprint Properties, the footprint and symbol bars, Grid Properties and
//! the sketch mode). Each control is driven through `Signex::update` and
//! checked by what it changes.

#[path = "b_sketch/bake.rs"]
mod bake;
#[path = "b_sketch/bars.rs"]
mod bars;
#[path = "b_sketch/board.rs"]
mod board;
#[path = "b_sketch/constraints.rs"]
mod constraints;
#[path = "b_sketch/grid.rs"]
mod grid;
#[path = "b_sketch/history.rs"]
mod history;
#[path = "b_sketch/modes.rs"]
mod modes;
#[path = "b_sketch/pads_link.rs"]
mod pads_link;
#[path = "b_sketch/parameters.rs"]
mod parameters;
#[path = "b_sketch/patterns.rs"]
mod patterns;
#[path = "b_sketch/properties.rs"]
mod properties;
#[path = "b_sketch/support.rs"]
mod support;
#[path = "b_sketch/symbol_bar.rs"]
mod symbol_bar;
