//! Named unions in the app (mockup row 158): the slice's state and messages
//! (`state`), its update (`update`) and the Create Union popover
//! (`popover`). The rules live in the engine (`signex_engine::unions`); the
//! Unions panel is `panels/unions/`, the canvas outline
//! `schematic_runtime/unions.rs`. Behaviour: `docs/SCHEMATIC_UNIONS.md`.

mod popover;
mod state;
mod update;

#[cfg(test)]
mod state_tests;

pub(crate) use popover::{UNION_NAME_INPUT_ID, tag_pill, view as popover_view};
pub use state::{
    CreateUnionPopover, UnionFoot, UnionGroup, UnionPopoverMsg, UnionRow, UnionScope, UnionsMsg,
    UnionsPanelView, UnionsState,
};
