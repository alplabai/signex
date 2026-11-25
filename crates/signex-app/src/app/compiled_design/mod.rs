//! Multi-channel sheets and the compiled design (#1279, #1308).
//!
//! `compile_project` stitches the project off the update thread
//! (`job.rs`) and keeps the result in memory per project (`state.rs`); the
//! Compiled Design panel and the channel tabs read it. An edit to a compiled
//! sheet marks it out of date (`stale.rs`); nothing recompiles on its own.
//! The Repeat rows of a sheet symbol write its designator through the
//! child-sheet identity edit (`repeat_edit.rs`), and the naming menus write
//! the project's channel naming (`naming.rs`). Rules: `docs/MULTI_CHANNEL.md`.

mod channels_of;
mod job;
mod naming;
mod repeat_edit;
mod stale;
mod state;
mod update;
mod view_model;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use signex_widgets::property_illustration::IllustrationMsg;
use uuid::Uuid;

pub(crate) use job::CompileOutcome;
pub(crate) use naming::{DESIGNATOR_PRESETS, NET_PRESETS};
pub use state::CompiledDesignUi;

use crate::panels::compiled_design::{CompiledMode, NamingTarget, RepeatField};

impl From<CompiledDesignMsg> for crate::app::Message {
    fn from(msg: CompiledDesignMsg) -> Self {
        Self::Project(crate::app::ProjectMsg::CompiledDesign(msg))
    }
}

/// What happened around multi-channel sheets and the Compiled Design panel.
#[derive(Debug, Clone)]
pub enum CompiledDesignMsg {
    /// `compile_project`, the panel's Recompile or Compile project button.
    CompileRequested,
    /// A compile finished.
    Compiled(Arc<CompileOutcome>),
    ModePicked(CompiledMode),
    FindChanged(String),
    /// The chevron of a repeated row: show or hide its channels.
    RowToggled(usize),
    /// A tree row was clicked: open its sheet (a channel on its tab).
    RowClicked(usize),
    /// A tree row was Ctrl+clicked (Cmd+click on macOS): open its source
    /// sheet, a channel's on its Editor tab (#2658).
    RowSourceClicked(usize),
    /// The chevron of a sheet instance: show or hide its parts and nets.
    RowExpanded(usize),
    /// A part or net row was clicked: open its sheet instance and go to it.
    TargetClicked(signex_net::CompiledTarget),
    /// A tab of the channel strip of `sheet` was picked; `None` is Editor,
    /// else the channel's 1-based index.
    ChannelPicked {
        sheet: PathBuf,
        channel: Option<u32>,
    },
    /// The Repeat switch of a sheet symbol's Properties.
    RepeatToggled {
        sheet_symbol: Uuid,
        on: bool,
    },
    RepeatFieldTyped {
        sheet_symbol: Uuid,
        field: RepeatField,
        text: String,
    },
    /// Enter in a Repeat field: write the typed repeat.
    RepeatFieldSubmitted {
        sheet_symbol: Uuid,
    },
    /// A naming field's menu was opened or closed.
    NamingMenuToggled(NamingTarget),
    NamingMenuClosed,
    /// A preset format was picked in the menu.
    NamingPicked {
        target: NamingTarget,
        format: String,
    },
    NamingCustomTyped(String),
    /// Enter in the Custom format… field.
    NamingCustomSubmitted,
    /// The sheet symbol illustration in Properties was hovered.
    IllustrationHovered(IllustrationMsg),
}
