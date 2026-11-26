//! Python cells (mockup row 200): a `#python(label: "...")[```python ...
//! ```]` block computes from schematic values and prints its result into
//! the page. This module is everything about cells but the interpreter:
//! reading cells from the notebook ([`cells_in`]), the values they read
//! with their units ([`value_reads`], [`Quantity`]), the result cache
//! ([`CellCache`]), when cells run and what each shows ([`run_cells`],
//! [`CellState`]), the sandbox rules ([`SandboxPolicy`]), asking before a
//! notebook from outside the project runs ([`TrustAnswers`]) and what the
//! page prints ([`PageCells`]). The runtime is a [`CellRunner`];
//! `docs/NOTEBOOK_CELLS.md` has the rules.

mod cache;
mod inputs;
mod model;
mod page;
mod quantity;
mod runner;
mod sandbox;
mod schedule;
mod trust;

pub use cache::{
    CELL_CACHE_FORMAT, CELL_CACHE_PATH, CacheNote, CachedCell, CellCache, source_hash,
};
pub use inputs::{DesignValues, Field, InputValue, ValueRead, current_inputs, value_reads};
pub use model::{
    CELL_FUNCTION, CELL_LANGUAGE, CellProblem, CellProblemKind, PythonCell, cells_in,
    notebook_cells,
};
pub use page::{PageCell, PageCells, PageFigure, PageValue, caption, note};
pub use quantity::{Quantity, Unit};
pub use runner::{
    CellError, CellRun, CellRunner, Figure, NoRuntime, RunOutput, RuntimeStatus, Script,
    ScriptedRunner,
};
pub use sandbox::{Access, Refusal, RefusalKind, SANDBOX_RULE, SandboxPolicy, classify, is_inside};
pub use schedule::{
    CellState, CellStatus, CellSummary, InputChange, RunContext, RunReport, Trigger, cell_states,
    run_cells, state_of,
};
pub use trust::{Trust, TrustAnswers};
