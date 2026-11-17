//! Interactive routing geometry (#2389), in integer nm: what a new track
//! leg is checked against ([`RouteWorld`]), the search that walks a leg
//! around other nets' copper ([`walkaround`]), the push that moves other
//! nets' tracks out of its way ([`shove`]), the accordion that lengthens
//! a track ([`accordion`]), and the side-by-side tracks of a differential
//! pair or a bus ([`parallel_paths`]). Clearances come from the caller's
//! rules ([`ClearanceRule`]), so the router keeps exactly the gaps the
//! design rule check asks for. How it fits together: `docs/PCB_TOOLS.md`,
//! "Interactive Routing".

mod meander;
mod parallel;
mod search;
mod shove;
mod world;

pub use meander::{accordion, accordion_room};
pub use parallel::{bus_offsets, offset_path, parallel_paths};
pub use search::{Walk, rule_legs, walkaround};
pub use shove::{Pushed, shove};
pub use world::{ClearanceRule, NewCopper, NewKind, RouteWorld};

#[cfg(test)]
mod meander_tests;
#[cfg(test)]
mod parallel_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod shove_tests;
#[cfg(test)]
pub(crate) mod test_board;
