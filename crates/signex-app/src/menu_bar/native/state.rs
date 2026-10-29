//! The native menu's state in the Model, and the pure decision of how to
//! bring an installed menu up to date.

use std::path::PathBuf;

use crate::app::command::CommandContext;
use crate::panels::PanelKind;

use super::{ActionMap, NativeMenu, NativeMenuSpec, NativeNode};

/// What an installed native menu needs to match a new spec.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS sync uses it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refresh {
    Unchanged,
    /// Same rows, only enablement moved: `(id, enabled)` for each that did.
    Enablement(Vec<ItemChange>),
    /// Rows, labels or accelerators changed: build the menu anew.
    Rebuild,
}

/// One row whose enablement or check mark moved.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS sync reads it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemChange {
    pub id: String,
    pub enabled: bool,
    pub checked: Option<bool>,
}

/// How `next` differs from the spec on screen. Pure.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS sync uses it
pub(crate) fn plan_refresh(applied: Option<&NativeMenuSpec>, next: &NativeMenuSpec) -> Refresh {
    let Some(applied) = applied else {
        return Refresh::Rebuild;
    };
    if applied == next {
        return Refresh::Unchanged;
    }
    let mut changes = Vec::new();
    if same_shape(&applied.menus, &next.menus, &mut changes) {
        Refresh::Enablement(changes)
    } else {
        Refresh::Rebuild
    }
}

fn same_shape(a: &[NativeMenu], b: &[NativeMenu], changes: &mut Vec<ItemChange>) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.title == b.title
                && a.nodes.len() == b.nodes.len()
                && a.nodes
                    .iter()
                    .zip(&b.nodes)
                    .all(|(a, b)| same_node(a, b, changes))
        })
}

fn same_node(a: &NativeNode, b: &NativeNode, changes: &mut Vec<ItemChange>) -> bool {
    match (a, b) {
        (NativeNode::Item(a), NativeNode::Item(b)) => {
            let same = a.id == b.id && a.label == b.label && a.accelerator == b.accelerator;
            if same && (a.enabled != b.enabled || a.checked != b.checked) {
                changes.push(ItemChange {
                    id: b.id.clone(),
                    enabled: b.enabled,
                    checked: b.checked,
                });
            }
            same
        }
        (NativeNode::Submenu(a), NativeNode::Submenu(b)) => {
            same_shape(std::slice::from_ref(a), std::slice::from_ref(b), changes)
        }
        (NativeNode::Predefined(a), NativeNode::Predefined(b)) => a == b,
        (NativeNode::Separator, NativeNode::Separator) => true,
        (NativeNode::Item(_), _)
        | (NativeNode::Submenu(_), _)
        | (NativeNode::Predefined(_), _)
        | (NativeNode::Separator, _) => false,
    }
}

/// Everything the native spec is built from: the enablement facts of the
/// focused window, the recent projects, and whether a confirm dialog locks
/// the model rows. The spec carries no shortcut text, so the keymap is not
/// an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuInputs {
    commands: CommandContext,
    recent: Vec<PathBuf>,
    locked: bool,
    open_panels: Vec<PanelKind>,
}

impl MenuInputs {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // built by the macOS sync
    pub(crate) fn new(
        commands: CommandContext,
        recent: &[PathBuf],
        locked: bool,
        open_panels: Vec<PanelKind>,
    ) -> Self {
        Self {
            commands,
            recent: recent.to_vec(),
            locked,
            open_panels,
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn locked(&self) -> bool {
        self.locked
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn commands(&self) -> CommandContext {
        self.commands
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn open_panels(&self) -> &[PanelKind] {
        &self.open_panels
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn recent(&self) -> &[PathBuf] {
        &self.recent
    }
}

/// Model-owned state of the native menu. The muda objects are `!Send` and
/// live inside `shell` (macOS only); everything else is plain data.
#[derive(Default)]
pub(crate) struct NativeMenuState {
    /// The main window has opened, so winit's own default menu exists and
    /// ours may replace it (docs/MENU_BAR.md, "Native macOS menu").
    pub(super) armed: bool,
    /// Installing failed once; the in-window row stays and nothing retries.
    pub(super) failed: bool,
    /// Live rows of the menu on screen, by id.
    pub(crate) actions: ActionMap,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // read by the macOS sync
    pub(super) applied: Option<NativeMenuSpec>,
    /// Inputs of the spec in `applied`; equal inputs skip the rebuild.
    pub(super) applied_inputs: Option<MenuInputs>,
    #[cfg(target_os = "macos")]
    pub(super) shell: Option<super::macos::Shell>,
}

impl NativeMenuState {
    /// The main window opened: the next sync installs the menu.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS window-open path arms it
    pub(crate) fn arm(&mut self) {
        self.armed = true;
    }

    /// Pretend installing failed, so the in-window row shows (view tests).
    #[cfg(test)]
    pub(crate) fn mark_failed_for_test(&mut self) {
        self.failed = true;
    }

    /// A row with a check mark was chosen: the OS has already flipped it,
    /// which may disagree with the panel (choosing an open panel that was
    /// hidden only reveals it). Forget what is on screen so the next sync
    /// redraws every mark from the Model.
    pub(crate) fn invalidate(&mut self) {
        self.applied = None;
        self.applied_inputs = None;
    }

    /// Whether `inputs` differ from the last sync and the menu is live.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn needs_sync(&self, inputs: &MenuInputs) -> bool {
        self.armed && !self.failed && self.applied_inputs.as_ref() != Some(inputs)
    }

    /// Whether the in-window menu row is left out: on macOS the system menu
    /// bar carries the menus from the first frame on (the row would flash
    /// until the first install), unless installing failed.
    pub(crate) fn hides_in_window_menu(&self) -> bool {
        cfg!(target_os = "macos") && !self.failed
    }
}

impl std::fmt::Debug for NativeMenuState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMenuState")
            .field("armed", &self.armed)
            .field("failed", &self.failed)
            .field("live_rows", &self.actions.len())
            .finish()
    }
}
