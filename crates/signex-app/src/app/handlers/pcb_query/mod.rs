//! Reading a board (#2403): Measure Distance, Find and Replace, Find Similar
//! Objects and the nets (Select Net, Highlight Net, the Net Inspector) on
//! the board in view. The numbers and matches come from the domain crates
//! (`pcb_geom::measure`, `pcb_geom::net_reports`,
//! `signex_engine::pcb::{find, similar}`); this slice keeps what the person
//! sees in step with them. A replace is one board undo step.

mod find;
mod inspector;
mod measure;
mod net;
mod similar;

pub(crate) use inspector::board_net_rows;

#[cfg(test)]
mod tests;
