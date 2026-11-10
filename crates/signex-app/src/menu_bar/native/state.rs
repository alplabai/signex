//! The native menu's state in the Model: whether the system menu bar could
//! be built, and the last menu built with the inputs it was built from.
//!
//! The iced runtime asks for the menu after every `update` batch (every
//! canvas mouse move too), so `NativeMenuState::menu` rebuilds it only when
//! an input the menu reads changed.

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app::Message;
use crate::app::command::CommandContext;
use crate::diagnostics::log_warning;
use crate::keymap::CompiledKeymap;
use crate::panels::PanelKind;

/// Everything the menu is built from: the enablement facts of the focused
/// window, the recent projects, whether a confirm dialog locks the model
/// rows, the open panels, the keymap the shortcut text comes from, and the
/// locale the titles render in. The keymap and the locale are replaced,
/// never edited, so pointer identity says whether they changed.
#[derive(Debug, Clone)]
pub(crate) struct MenuInputs {
    commands: CommandContext,
    recent: Vec<PathBuf>,
    locked: bool,
    open_panels: Vec<PanelKind>,
    keymap: Option<Arc<CompiledKeymap>>,
    i18n: Arc<signex_i18n::I18n>,
}

impl PartialEq for MenuInputs {
    fn eq(&self, other: &Self) -> bool {
        self.commands == other.commands
            && self.recent == other.recent
            && self.locked == other.locked
            && self.open_panels == other.open_panels
            && match (&self.keymap, &other.keymap) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && Arc::ptr_eq(&self.i18n, &other.i18n)
    }
}

impl Eq for MenuInputs {}

impl MenuInputs {
    pub(crate) fn new(
        commands: CommandContext,
        recent: &[PathBuf],
        locked: bool,
        open_panels: Vec<PanelKind>,
        i18n: Arc<signex_i18n::I18n>,
    ) -> Self {
        Self {
            commands,
            recent: recent.to_vec(),
            locked,
            open_panels,
            keymap: None,
            i18n,
        }
    }

    pub(crate) fn with_keymap(mut self, keymap: Arc<CompiledKeymap>) -> Self {
        self.keymap = Some(keymap);
        self
    }

    pub(super) fn keymap(&self) -> Option<Arc<CompiledKeymap>> {
        self.keymap.clone()
    }

    pub(super) fn i18n(&self) -> Arc<signex_i18n::I18n> {
        Arc::clone(&self.i18n)
    }

    pub(super) fn locked(&self) -> bool {
        self.locked
    }

    pub(super) fn commands(&self) -> CommandContext {
        self.commands
    }

    pub(super) fn open_panels(&self) -> &[PanelKind] {
        &self.open_panels
    }

    pub(super) fn recent(&self) -> &[PathBuf] {
        &self.recent
    }
}

#[derive(Debug, Default)]
pub(crate) struct NativeMenuState {
    /// The system menu bar could not be built; the in-window row stays.
    /// Sticky: a later successful menu does not bring the system bar back
    /// mid-session.
    failed: bool,
    /// The last menu built and the inputs it came from. A view-style cache
    /// behind `&self`: the runtime asks for the menu through `&Signex`.
    built: RefCell<Option<(MenuInputs, iced::menu::Menu<Message>)>>,
    #[cfg(test)]
    builds: Cell<usize>,
}

impl NativeMenuState {
    /// The menu for `inputs`: the last one built when `inputs` are the same,
    /// else a new one.
    pub(crate) fn menu(&self, inputs: MenuInputs) -> iced::menu::Menu<Message> {
        let mut built = self.built.borrow_mut();
        if let Some((seen, menu)) = built.as_ref()
            && *seen == inputs
        {
            return menu.clone();
        }
        let menu = super::build_for(&inputs);
        #[cfg(test)]
        self.builds.set(self.builds.get() + 1);
        *built = Some((inputs, menu.clone()));
        menu
    }

    /// How many times the menu was built, not served from the cache (tests).
    #[cfg(test)]
    pub(crate) fn builds(&self) -> usize {
        self.builds.get()
    }

    /// The iced runtime reported that the system menu bar cannot be built:
    /// let the in-window row come back, and say why in the Messages panel.
    pub(crate) fn fall_back(&mut self, reason: &str) {
        self.failed = true;
        log_warning(format!(
            "Main menu: could not build the system menu bar ({reason}); using the in-window menu"
        ));
    }

    /// Whether the in-window menu row is left out: on macOS the system menu
    /// bar carries the menus from the first frame on (the row would flash
    /// until the first install), unless building it failed.
    pub(crate) fn hides_in_window_menu(&self) -> bool {
        cfg!(target_os = "macos") && !self.failed
    }

    /// Whether building the system menu bar was reported as failed (tests).
    #[cfg(test)]
    pub(crate) fn has_failed(&self) -> bool {
        self.failed
    }
}
