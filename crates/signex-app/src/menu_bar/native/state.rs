//! The native menu's state in the Model, and the pure decision of how to
//! bring an installed menu up to date.

use std::path::PathBuf;

use crate::app::command::CommandContext;

use super::{ActionMap, NativeMenu, NativeMenuSpec, NativeNode};

/// What an installed native menu needs to match a new spec.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS sync uses it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refresh {
    Unchanged,
    /// Same rows, only enablement moved: `(id, enabled)` for each that did.
    Enablement(Vec<(String, bool)>),
    /// Rows, labels or accelerators changed: build the menu anew.
    Rebuild,
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

fn same_shape(a: &[NativeMenu], b: &[NativeMenu], changes: &mut Vec<(String, bool)>) -> bool {
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

fn same_node(a: &NativeNode, b: &NativeNode, changes: &mut Vec<(String, bool)>) -> bool {
    match (a, b) {
        (NativeNode::Item(a), NativeNode::Item(b)) => {
            let same = a.id == b.id && a.label == b.label && a.accelerator == b.accelerator;
            if same && a.enabled != b.enabled {
                changes.push((b.id.clone(), b.enabled));
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

/// Everything the native spec is built from. The spec carries no shortcut
/// text, so the keymap is not an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuInputs {
    commands: CommandContext,
    recent: Vec<PathBuf>,
}

impl MenuInputs {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // built by the macOS sync
    pub(crate) fn new(commands: CommandContext, recent: &[PathBuf]) -> Self {
        Self {
            commands,
            recent: recent.to_vec(),
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn commands(&self) -> CommandContext {
        self.commands
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
    /// The native menu is the menu: the in-window row is not drawn.
    pub(super) installed: bool,
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

    /// Pretend the system menu bar is up, for view tests.
    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn mark_installed_for_test(&mut self) {
        self.installed = true;
    }

    /// Whether `inputs` differ from the last sync and the menu is live.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn needs_sync(&self, inputs: &MenuInputs) -> bool {
        self.armed && !self.failed && self.applied_inputs.as_ref() != Some(inputs)
    }

    /// Whether the system menu bar is showing the main menu.
    pub(crate) fn is_installed(&self) -> bool {
        self.installed
    }
}

impl std::fmt::Debug for NativeMenuState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMenuState")
            .field("armed", &self.armed)
            .field("installed", &self.installed)
            .field("failed", &self.failed)
            .field("live_rows", &self.actions.len())
            .finish()
    }
}
