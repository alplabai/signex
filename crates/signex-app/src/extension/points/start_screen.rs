//! Start-screen sections an extension adds. A GUI adapter: it stays out of
//! `app/core/` (ADR-0006).
//!
//! The Start screen has one named slot for extension sections,
//! [`StartScreenSlot::HomeEnd`]: on the Home page, below the project lists.
//! With no section added it renders nothing there.

use std::fmt;

use signex_i18n::MessageId;

use super::{RegisterError, claim};
use crate::extension::{ExtensionId, ExtensionView};

/// Where on the Start screen a section goes. One slot in this version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StartScreenSlot {
    /// The Home page, below the reference design and the project lists.
    HomeEnd,
}

/// One registered section: its key, its heading and its view hooks.
struct SectionEntry {
    key: &'static str,
    title: MessageId,
    slot: StartScreenSlot,
    view: Box<dyn ExtensionView>,
}

/// The Start-screen sections one extension adds, in the order it added
/// them.
#[non_exhaustive]
pub struct StartScreenSections {
    owner: ExtensionId,
    entries: Vec<SectionEntry>,
    pub(super) errors: Vec<RegisterError>,
}

impl fmt::Debug for StartScreenSections {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.entries.iter().map(|entry| (entry.key, entry.slot)))
            .finish()
    }
}

impl StartScreenSections {
    pub(super) fn new(owner: ExtensionId) -> Self {
        Self {
            owner,
            entries: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Add the section `key` (`ext.<name>.<local>`) to `slot`, under the
    /// heading `title` (a catalog message), drawn and updated by `view`.
    /// Refused, and recorded so install refuses the set, when the key is not
    /// under the owner's prefix or was added already.
    pub fn add(
        &mut self,
        key: &'static str,
        title: MessageId,
        slot: StartScreenSlot,
        view: impl ExtensionView,
    ) -> Result<(), RegisterError> {
        let taken = self.keys().any(|taken| taken == key);
        claim(&self.owner, key, taken, &mut self.errors)?;
        self.entries.push(SectionEntry {
            key,
            title,
            slot,
            view: Box::new(view),
        });
        Ok(())
    }

    /// Were no sections added?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The section keys, in the order they were added.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|entry| entry.key)
    }

    /// The sections of `slot` as (key, heading, view hooks), in order.
    pub(crate) fn in_slot(
        &self,
        slot: StartScreenSlot,
    ) -> impl Iterator<Item = (&'static str, MessageId, &(dyn ExtensionView + 'static))> + '_ {
        self.entries
            .iter()
            .filter(move |entry| entry.slot == slot)
            .map(|entry| (entry.key, entry.title, &*entry.view))
    }

    /// The view hooks of the section `key`, to update it.
    pub(crate) fn view_mut(&mut self, key: &str) -> Option<&mut (dyn ExtensionView + 'static)> {
        self.entries
            .iter_mut()
            .find(|entry| entry.key == key)
            .map(|entry| &mut *entry.view)
    }
}
