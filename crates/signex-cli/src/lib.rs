//! Signex headless command runner (#1923).
//!
//! Runs catalog commands by id against a project with no window, through
//! `signex_app::headless`: JSON Lines in, one JSON result line out per
//! command. A running Signex that has the project open takes the commands
//! instead (#1924: `route`, `attach`). The security rules (no writes without `--save`, path confinement,
//! capability groups, input limits, audit log) live in [`policy`], [`confine`],
//! [`capability`] and [`input`]; see `docs/CLI.md`.

mod attach;
pub mod capability;
pub mod catalog;
pub mod cli;
pub mod confine;
mod edits;
pub mod input;
pub mod policy;
pub mod protocol;
mod roots;
mod route;
mod run;
mod session;

pub use run::{RunReport, run};
