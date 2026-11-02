//! [`CommandArgs`] - what a caller knows about an invocation beyond the
//! command id (#278, #1727).
//!
//! A command id behaves the same on every surface: the context menu, the
//! palette, a keystroke, a menu bar row and, later, a CLI or an agent run
//! the same id with the same effect. Whatever a surface knows that the id
//! does not (where the user clicked, which project a tree row stands on)
//! travels here as plain data, never as a surface-only message. Keep the
//! fields `Clone + Debug` shapes a CLI could build from text: no UI
//! handles, no window ids.

/// Plain-data context for one command invocation. [`Self::none`] is what a
/// keystroke or the palette sends.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CommandArgs {
    /// Document point, in mm, the invocation is about: where the user
    /// right-clicked. `find_similar_objects` seeds from the object there.
    pub at: Option<(f64, f64)>,
    /// Id of the project the invocation is about, as `ProjectId` prints
    /// it. `open_project_options` opens that project's tab; without one
    /// it acts on the active document's project.
    pub project: Option<String>,
}

impl CommandArgs {
    /// No context: what a keystroke or the command palette dispatches with.
    pub(crate) fn none() -> Self {
        Self::default()
    }

    /// Context of a click at the document point `at`.
    pub(crate) fn at(at: Option<(f64, f64)>) -> Self {
        Self { at, project: None }
    }

    /// Context of a row standing on the project `id`.
    pub(crate) fn project(id: impl Into<String>) -> Self {
        Self {
            at: None,
            project: Some(id.into()),
        }
    }
}
