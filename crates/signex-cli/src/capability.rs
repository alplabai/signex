//! The capability groups a run may use (#1923): every catalog command needs
//! exactly one, and a run holds `read,edit` unless it names more.

use std::collections::BTreeSet;
use std::fmt;

use serde::Serialize;
use signex_app::keymap::CommandMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Inspect or select; changes no document.
    Read,
    /// Change a document in memory (undoable edits, undo, redo).
    Edit,
    /// Write a document or project to disk, or create one.
    Save,
    /// Write a report or an export file.
    Export,
    /// Remove files. No catalog command does yet; the group exists so the
    /// first one cannot run unnamed.
    Delete,
    /// Change a library.
    LibraryWrite,
}

impl Capability {
    pub const ALL: [Self; 6] = [
        Self::Read,
        Self::Edit,
        Self::Save,
        Self::Export,
        Self::Delete,
        Self::LibraryWrite,
    ];

    /// What a run holds when it names nothing.
    pub const DEFAULT: [Self; 2] = [Self::Read, Self::Edit];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Save => "save",
            Self::Export => "export",
            Self::Delete => "delete",
            Self::LibraryWrite => "library-write",
        }
    }

    /// Whether a path operand of a command in this group is written to or
    /// removed, rather than read.
    pub const fn writes_files(self) -> bool {
        matches!(
            self,
            Self::Save | Self::Export | Self::Delete | Self::LibraryWrite
        )
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `read,edit` style list; a name that is not a capability is an error.
pub fn parse_list(text: &str) -> Result<BTreeSet<Capability>, String> {
    text.split(',')
        .map(|name| {
            Capability::ALL
                .into_iter()
                .find(|cap| cap.name() == name.trim())
                .ok_or_else(|| {
                    let all: Vec<_> = Capability::ALL.iter().map(|cap| cap.name()).collect();
                    format!(
                        "`{}` is not a capability (one of {})",
                        name.trim(),
                        all.join(", ")
                    )
                })
        })
        .collect()
}

/// Commands that write a file or create a project. The catalog has no flag
/// for it (a `file` command is not `mutates_doc`), so they are named; a test
/// keeps the list in step with the catalog.
const SAVES: &[&str] = &[
    "save_document",
    "save_document_as",
    "new_document",
    "new_project_with_profile",
];

/// The one capability `command` needs, from its catalog flags.
pub fn capability_of(command: &CommandMetadata) -> Capability {
    if SAVES.contains(&command.id) {
        Capability::Save
    } else if command.category == "reports" || command.id == "print" {
        Capability::Export
    } else if command.category == "library" && command.flags.mutates_doc {
        Capability::LibraryWrite
    } else if command.flags.mutates_doc {
        Capability::Edit
    } else {
        Capability::Read
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_app::keymap::{all_command_ids, metadata_for_str};

    fn cap(id: &str) -> Capability {
        capability_of(metadata_for_str(id).expect("a catalog command"))
    }

    #[test]
    fn only_the_file_writing_groups_write_files() {
        let writers: Vec<_> = Capability::ALL
            .into_iter()
            .filter(|cap| cap.writes_files())
            .collect();
        assert_eq!(
            writers,
            [
                Capability::Save,
                Capability::Export,
                Capability::Delete,
                Capability::LibraryWrite
            ]
        );
    }

    #[test]
    fn every_catalog_command_maps_to_one_named_capability() {
        let mut seen = BTreeSet::new();
        for id in all_command_ids() {
            seen.insert(cap(id));
        }
        // The names round-trip, so `--allow <name>` can ask for each one.
        for found in &seen {
            assert_eq!(parse_list(found.name()).unwrap(), BTreeSet::from([*found]));
        }
        assert!(seen.contains(&Capability::Read) && seen.contains(&Capability::Edit));
    }

    /// An agent's edit list and its revert change a document in memory only:
    /// `edit`, never `save`.
    #[test]
    fn an_agents_edit_list_and_its_revert_need_the_edit_capability() {
        assert_eq!(cap("apply_edits"), Capability::Edit);
        assert_eq!(cap("revert"), Capability::Edit);
    }

    #[test]
    fn the_catalogs_writers_and_exporters_are_pinned() {
        let non_default: BTreeSet<_> = all_command_ids()
            .filter(|id| !Capability::DEFAULT.contains(&cap(id)))
            .map(|id| (cap(id), id))
            .collect();
        let pinned = BTreeSet::from([
            (Capability::Save, "new_document"),
            (Capability::Save, "new_project_with_profile"),
            (Capability::Save, "save_document"),
            (Capability::Save, "save_document_as"),
            (Capability::Export, "print"),
            (Capability::Export, "report_manager_bom"),
            (Capability::LibraryWrite, "import_graphics"),
        ]);
        assert_eq!(non_default, pinned);
    }

    #[test]
    fn every_save_command_is_in_the_catalog_and_none_is_missed() {
        for id in SAVES {
            assert!(metadata_for_str(id).is_some(), "{id} left the catalog");
        }
        for id in all_command_ids().filter(|id| id.contains("save")) {
            assert_eq!(cap(id), Capability::Save, "{id} writes: name it in SAVES");
        }
    }

    #[test]
    fn a_list_names_capabilities_and_refuses_others() {
        assert_eq!(
            parse_list("save, export").unwrap(),
            BTreeSet::from([Capability::Save, Capability::Export])
        );
        let err = parse_list("read,nuke").unwrap_err();
        assert!(err.contains("`nuke` is not a capability"), "{err}");
    }
}
