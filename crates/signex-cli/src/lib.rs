//! Signex headless command runner (#1923).
//!
//! Runs catalog commands by id against a project with no window, through
//! `signex_app::headless`: JSON Lines in, one JSON result line out per
//! command. The security rules (no writes without `--save`, path confinement,
//! capability groups, input limits, audit log) live in [`policy`], [`confine`],
//! [`capability`] and [`input`]; see `docs/CLI.md`.

pub mod capability;
pub mod catalog;
pub mod cli;
pub mod confine;
pub mod input;
pub mod policy;
pub mod protocol;
mod roots;
mod run;
mod session;

pub use run::{RunReport, run};
