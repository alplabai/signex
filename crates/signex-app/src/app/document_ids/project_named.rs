//! The open project a `document` operand names: one rule for every command
//! that acts on a project through one of its documents (Update PCB, New PCB,
//! Back Annotate, the board against the schematic, the priced BOM, the
//! Rules and Project tab commands).

use super::DocumentTarget;
use super::entries::DocumentKind;
use crate::app::Signex;
use crate::app::state::LoadedProject;

/// Why a `document` operand names no open project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NoProjectNamed {
    /// The operand names no document; the reason in words.
    Unresolved(String),
    /// The document, by its name, is in no open project.
    NotInProject(String),
}

impl NoProjectNamed {
    /// The reason in words, for a refusal that has no catalog line of its own.
    pub(crate) fn english(&self) -> String {
        match self {
            Self::Unresolved(why) => why.clone(),
            Self::NotInProject(name) => format!("{name} is not part of an open project"),
        }
    }
}

impl Signex {
    /// The open project `target` names: the project file itself; else the
    /// project that owns the document, its tab's project first
    /// (`owning_project`), then the project that lists it (a board by its
    /// `pcb_file`, the first loaded when two list it).
    pub(crate) fn project_named(
        &self,
        target: &DocumentTarget,
    ) -> Result<&LoadedProject, NoProjectNamed> {
        let entry = self
            .resolve_document_target(target)
            .map_err(NoProjectNamed::Unresolved)?;
        let docs = &self.document_state;
        let found = match (entry.kind, entry.path.as_deref()) {
            (DocumentKind::Project, Some(path)) => docs.projects.iter().find(|p| p.path == path),
            (kind, Some(path)) => self.owning_project(path).or_else(|| {
                (kind == DocumentKind::Pcb)
                    .then(|| docs.board_project(path, None))
                    .flatten()
            }),
            (_, None) => None,
        };
        let found = found.or_else(|| {
            let listed = entry.project.as_ref()?;
            docs.projects.iter().find(|p| p.path == listed.path)
        });
        found.ok_or(NoProjectNamed::NotInProject(entry.name))
    }
}

#[cfg(test)]
#[path = "project_named_tests.rs"]
mod tests;
