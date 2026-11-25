//! The panel and Properties state of the multi-channel surfaces (one
//! `UiState` field). The compiles themselves are document data
//! (`app::core::compiled_design`, one `DocumentState` field). Nothing here is
//! written to disk.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use signex_widgets::property_illustration::IllustrationHover;
use uuid::Uuid;

pub(crate) use crate::app::core::compiled_design::CompiledResult;
use crate::panels::compiled_design::{CompiledMode, NamingTarget, RepeatField};

/// Typed Repeat fields of one sheet symbol, until Enter writes them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepeatDraft {
    pub sheet_symbol: Uuid,
    pub fields: HashMap<RepeatField, String>,
    /// Enter refused these fields; typing again clears it.
    pub refused: bool,
}

/// The open naming format menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamingMenu {
    pub target: NamingTarget,
    pub custom: String,
}

/// The view state of the Compiled Design panel, the channel tabs and the
/// Repeat rows of Properties.
#[derive(Debug, Default)]
pub struct CompiledDesignUi {
    pub mode: CompiledMode,
    pub find: String,
    /// Repeated rows folded by the user, by row index of the result.
    pub folded: HashSet<usize>,
    /// Sheet rows showing their parts and nets, by row index of the result.
    pub expanded: HashSet<usize>,
    /// The selected tree row, by row index of the result.
    pub selected: Option<usize>,
    /// The channel each sheet file shows (1-based index); absent is Editor.
    pub channel_tabs: HashMap<PathBuf, u32>,
    pub repeat_draft: Option<RepeatDraft>,
    pub naming_menu: Option<NamingMenu>,
    pub illustration_hover: IllustrationHover,
    /// The channel refusal last said in Messages, so a drag says it once.
    pub refusal_noted: Option<(PathBuf, u32)>,
}
