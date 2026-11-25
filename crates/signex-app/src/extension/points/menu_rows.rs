//! Menu bar and Active Bar rows an extension adds. A GUI adapter: it stays
//! out of `app/core/` (ADR-0006).
//!
//! A row runs a command by id through `dispatch_command`, the path every
//! built-in menu row takes, so one command keeps one behaviour on every
//! surface. Built-in rows keep their order: an extension's rows come after
//! them, below a separator.

use signex_i18n::MessageId;

use super::{RegisterError, claim};
use crate::extension::ExtensionId;
use crate::keymap::AppCommandId;

pub use crate::app::pcb_bar_groups::PcbBarGroup;

/// A root menu of the menu bar, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuRoot {
    File,
    Edit,
    View,
    Project,
    Place,
    Design,
    Tools,
    Help,
}

impl MenuRoot {
    /// The root menus in the bar's order.
    pub const ALL: [Self; 8] = [
        Self::File,
        Self::Edit,
        Self::View,
        Self::Project,
        Self::Place,
        Self::Design,
        Self::Tools,
        Self::Help,
    ];

    /// The root's place in the bar, from the left.
    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|root| *root == self)
            .unwrap_or_default()
    }
}

/// A menu bar row: at the end of `root`, labelled by a catalog message,
/// running the extension's command `command`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    pub root: MenuRoot,
    pub label: MessageId,
    pub command: AppCommandId,
}

/// An Active Bar row: at the end of the dropdown of the PCB Active Bar's
/// tool group `group`, labelled by a catalog message, running the
/// extension's command `command`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveBarRow {
    pub group: PcbBarGroup,
    pub label: MessageId,
    pub command: AppCommandId,
}

/// The menu bar and Active Bar rows one extension adds, in the order it
/// added them.
#[derive(Debug)]
#[non_exhaustive]
pub struct MenuRows {
    owner: ExtensionId,
    menu: Vec<MenuRow>,
    active_bar: Vec<ActiveBarRow>,
    pub(super) errors: Vec<RegisterError>,
}

impl MenuRows {
    pub(super) fn new(owner: ExtensionId) -> Self {
        Self {
            owner,
            menu: Vec::new(),
            active_bar: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// The command `command` as an id this extension owns.
    fn owned_command(&mut self, command: &str) -> Result<AppCommandId, RegisterError> {
        claim(&self.owner, command, false, &mut self.errors)?;
        AppCommandId::new(command).map_err(|_| {
            let error = RegisterError::InvalidId(command.to_owned());
            self.errors.push(error.clone());
            error
        })
    }

    /// Add a row at the end of the menu `root` that runs `command`
    /// (`ext.<name>.<local>`), labelled `label`. Refused, and recorded so
    /// install refuses the set, when the command is not the owner's.
    pub fn add_menu_row(
        &mut self,
        root: MenuRoot,
        label: MessageId,
        command: &str,
    ) -> Result<(), RegisterError> {
        let command = self.owned_command(command)?;
        self.menu.push(MenuRow {
            root,
            label,
            command,
        });
        Ok(())
    }

    /// Add a row at the end of the PCB Active Bar group `group`'s dropdown
    /// that runs `command`, labelled `label`. Refused like
    /// [`Self::add_menu_row`].
    pub fn add_active_bar_row(
        &mut self,
        group: PcbBarGroup,
        label: MessageId,
        command: &str,
    ) -> Result<(), RegisterError> {
        let command = self.owned_command(command)?;
        self.active_bar.push(ActiveBarRow {
            group,
            label,
            command,
        });
        Ok(())
    }

    /// Were no rows added?
    pub fn is_empty(&self) -> bool {
        self.menu.is_empty() && self.active_bar.is_empty()
    }

    /// The menu bar rows, in the order they were added.
    pub fn menu(&self) -> &[MenuRow] {
        &self.menu
    }

    /// The Active Bar rows, in the order they were added.
    pub fn active_bar(&self) -> &[ActiveBarRow] {
        &self.active_bar
    }
}
