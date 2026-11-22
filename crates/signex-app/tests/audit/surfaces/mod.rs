//! The audited surfaces, one table per mockup group file
//! (`mockups/group-<name>.html`; the `group-new-*` files share
//! `new_features`). Each group file is edited on its own, so several rows
//! can be filled in parallel without touching the same file.

use crate::surface::Surface;

mod annot_valid;
mod board_counters;
mod canvas_pick;
pub mod checks;
mod drawn_fixtures;
mod global;
mod helpers_c;
mod helpers_d;
mod helpers_f;
mod lib;
mod new_features;
mod output_prefs;
mod pcb;
pub mod pointer;
mod project;
mod sch_a;
mod sch_b;
mod states;
mod states_drawn;
mod states_final;
mod states_more;
mod states_sketch;
mod view_click;

/// Every group, by the name `AUDIT_ONLY` accepts, with its rows.
pub fn groups() -> Vec<(&'static str, Vec<Surface>)> {
    vec![
        ("global", global::rows()),
        ("project", project::rows()),
        ("sch_a", sch_a::rows()),
        ("sch_b", sch_b::rows()),
        ("annot_valid", annot_valid::rows()),
        ("pcb", pcb::rows()),
        ("output_prefs", output_prefs::rows()),
        ("lib", lib::rows()),
        ("new_features", new_features::rows()),
        ("states", states::rows()),
        ("states_drawn", states_drawn::rows()),
        ("states_more", states_more::rows()),
        ("states_final", states_final::rows()),
        ("states_sketch", states_sketch::rows()),
    ]
}
