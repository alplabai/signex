//! The channel naming a project saved, and whether it can be used. A
//! `schematic_design` section this build cannot read leaves the default naming
//! in memory, which is not what the project saved: an output that names a
//! channel part from it would be wrong, so it is refused. A project with no
//! repeated sheet names nothing through it and is unaffected. See
//! `docs/MULTI_CHANNEL.md`, "Stitching".

use signex_i18n::ids;
use signex_net::{ChannelDesignators, ProjectGraph};
use signex_types::project::{ChannelNaming, ProjectData};

/// A project's channel naming as read from its file.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SavedChannelNaming {
    naming: ChannelNaming,
    is_unreadable: bool,
}

/// The project has repeated sheets and its saved naming cannot be read. Shows
/// as the unreadable-settings banner text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChannelNamingUnreadable;

impl std::fmt::Display for ChannelNamingUnreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let banner = signex_i18n::I18n::en_us_ref()
            .message_plain(ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE, &[]);
        f.write_str(&banner)
    }
}

impl From<ChannelNaming> for SavedChannelNaming {
    fn from(naming: ChannelNaming) -> Self {
        Self {
            naming,
            is_unreadable: false,
        }
    }
}

impl SavedChannelNaming {
    pub(crate) fn of(data: &ProjectData) -> Self {
        Self {
            naming: data.schematic_design.channel_naming.clone(),
            is_unreadable: data.has_unreadable_schematic_design(),
        }
    }

    /// A project whose section could not be read.
    #[cfg(test)]
    pub(crate) fn unreadable() -> Self {
        Self {
            naming: ChannelNaming::default(),
            is_unreadable: true,
        }
    }

    /// The naming, or `None` when the file's section could not be read.
    pub(crate) fn readable(&self) -> Option<&ChannelNaming> {
        (!self.is_unreadable).then_some(&self.naming)
    }

    /// The naming to name `design`'s parts and nets through; refused when the
    /// design has a channel and the saved naming cannot be read.
    pub(crate) fn for_design(
        &self,
        design: &ProjectGraph,
    ) -> Result<&ChannelNaming, ChannelNamingUnreadable> {
        if self.is_unreadable && ChannelDesignators::of_project(design, &self.naming).has_channels()
        {
            return Err(ChannelNamingUnreadable);
        }
        Ok(&self.naming)
    }
}
