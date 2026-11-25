//! The Design Notebook's bench work (mockup row 199): the measurement
//! commands, answered from the project's notebook read off the UI thread,
//! and the margin lane's timeline and stale-attachment views. What a
//! reading, a timeline, a stale mark or a scope plot is lives in
//! `signex_notebook`; see `docs/NOTEBOOK_MEASUREMENTS.md`.

mod design;
mod job;
mod operands;
pub(crate) mod views;

pub use job::NotebookRead;
pub(crate) use operands::{
    QUERY_IDS, attach_notebook_file, export_notebook_timeline, mark_notebook_attachment_valid,
    query,
};

#[cfg(test)]
mod tests;
