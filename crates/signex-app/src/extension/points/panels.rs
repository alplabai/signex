//! Dock panels an extension adds. A GUI adapter: it stays out of `app/core/`
//! (ADR-0006).
//!
//! A panel has a stable layout key under its owner's `ext.<name>.` prefix. A
//! saved layout that names a key no installed extension registered keeps
//! that key (`docs/DECISIONS.md`, 2026-11-25).

use std::fmt;

use signex_i18n::MessageId;

use super::{RegisterError, claim};
use crate::app::{Message, OverlayMsg};
use crate::dock::PanelPosition;
use crate::extension::{ExtensionId, ExtensionView};
use crate::panels::PanelKind;

/// One extension panel, as the dock holds it: its layout key, the catalog
/// id of its tab title and the dock region it opens into. `Copy`, so it
/// travels in [`PanelKind::Extension`] like a built-in kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtensionPanelKind {
    key: &'static str,
    title: MessageId,
    home: PanelPosition,
}

impl ExtensionPanelKind {
    /// The stable layout key (`ext.<name>.<local>`), saved in the dock
    /// layout.
    pub fn key(self) -> &'static str {
        self.key
    }

    /// The catalog id of the tab title.
    pub fn title(self) -> MessageId {
        self.title
    }

    /// The dock region the panel opens into when it is not open yet.
    pub fn home(self) -> PanelPosition {
        self.home
    }

    /// The message that opens the panel: docked in its home region when it
    /// is not open, else brought to the front, as the panel list does for a
    /// built-in panel. An extension returns it from its `update` as a
    /// `Task::done`.
    pub fn open_message(self) -> Message {
        Message::Overlay(OverlayMsg::OpenPanel(PanelKind::Extension(self)))
    }
}

/// One registered panel and its view hooks.
struct PanelEntry {
    kind: ExtensionPanelKind,
    view: Box<dyn ExtensionView>,
}

/// The dock panels one extension adds, in the order it added them.
#[non_exhaustive]
pub struct Panels {
    owner: ExtensionId,
    entries: Vec<PanelEntry>,
    pub(super) errors: Vec<RegisterError>,
}

impl fmt::Debug for Panels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.entries.iter().map(|entry| entry.kind))
            .finish()
    }
}

impl Panels {
    pub(super) fn new(owner: ExtensionId) -> Self {
        Self {
            owner,
            entries: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// An empty set for `owner`, for tests outside the extension module.
    #[cfg(test)]
    pub(crate) fn for_tests(owner: ExtensionId) -> Self {
        Self::new(owner)
    }

    /// Add the dock panel `key` (`ext.<name>.<local>`), titled by the
    /// catalog message `title`, opening into `home`, drawn and updated by
    /// `view`. Refused, and recorded so install refuses the set, when the
    /// key is not under the owner's prefix or was added already.
    pub fn add(
        &mut self,
        key: &'static str,
        title: MessageId,
        home: PanelPosition,
        view: impl ExtensionView,
    ) -> Result<ExtensionPanelKind, RegisterError> {
        let taken = self.find(key).is_some();
        claim(&self.owner, key, taken, &mut self.errors)?;
        let kind = ExtensionPanelKind { key, title, home };
        self.entries.push(PanelEntry {
            kind,
            view: Box::new(view),
        });
        Ok(kind)
    }

    /// Were no panels added?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The panels, in the order they were added.
    pub fn kinds(&self) -> impl Iterator<Item = ExtensionPanelKind> + '_ {
        self.entries.iter().map(|entry| entry.kind)
    }

    /// The panel saved under `key`, if this extension added one.
    pub fn find(&self, key: &str) -> Option<ExtensionPanelKind> {
        self.kinds().find(|kind| kind.key == key)
    }

    /// The view hooks of the panel `key`.
    pub(crate) fn view(&self, key: &str) -> Option<&(dyn ExtensionView + 'static)> {
        self.entries
            .iter()
            .find(|entry| entry.kind.key == key)
            .map(|entry| &*entry.view)
    }

    /// The view hooks of the panel `key`, to update it.
    pub(crate) fn view_mut(&mut self, key: &str) -> Option<&mut (dyn ExtensionView + 'static)> {
        self.entries
            .iter_mut()
            .find(|entry| entry.kind.key == key)
            .map(|entry| &mut *entry.view)
    }
}
