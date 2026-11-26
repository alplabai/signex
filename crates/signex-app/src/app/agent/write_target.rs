//! Where an agent's file writes may land (#2470): the `path` of a command in
//! `allow::WRITES_IN_PROJECT` lies inside the folder of the project its
//! `document` belongs to. The gate refuses, by the path's text, a relative
//! path and one holding `..`, so `update` reads no disk; whether it lies in
//! the project is decided by the job that writes, which resolves both the
//! folder and the target, links followed (`notebook_project::confined_write`,
//! the rule an agent's notebook export follows), so a folder reached through
//! a link (`/var` and `/private/var` on macOS, a linked home) is not refused
//! by its spelling. See `docs/AGENT_SOCKET.md`, "Allowlist".

use std::path::{Component, Path, PathBuf};

use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};

/// Why the `path` of `args` may not be written by an agent, if it may not:
/// it must be absolute and hold no `..`. A missing `path` is left to the
/// handler, which refuses it with its own reason.
pub(super) fn path_refusal(args: &CommandArgs) -> Option<String> {
    let Some(OperandValue::Path(text)) = args.operand("path") else {
        return None;
    };
    let path = Path::new(text);
    let plain = path.is_absolute()
        && path
            .components()
            .all(|part| !matches!(part, Component::ParentDir));
    (!plain).then(|| {
        format!(
            "`path` {} is not an absolute path without `..`: an agent writes only inside its project",
            path.display()
        )
    })
}

impl Signex {
    /// The folder a command's write is confined to when it runs: `None`
    /// outside an agent's request; else the folder of the open project
    /// that holds `document` (the board an export names), or an empty path
    /// when none does, which `confined_write` refuses for every target.
    pub(crate) fn agent_write_root(&self, document: &Path) -> Option<PathBuf> {
        if !self.agent.serving {
            return None;
        }
        // The project the document's tab belongs to, else the one whose
        // folder holds it.
        let state = &self.document_state;
        let by_tab = state
            .tabs
            .iter()
            .find(|tab| tab.path == document)
            .and_then(|tab| tab.project_id)
            .and_then(|id| state.project_by_id(id));
        let folder = by_tab
            .or_else(|| state.project_for_path(document))
            .map(|project| project.dir().to_path_buf());
        Some(folder.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::AppCommandId;

    fn args(path: &str) -> CommandArgs {
        let command = AppCommandId::new("export_board_pdf").expect("an id");
        let path = format!("path={path}");
        CommandArgs::parse_operands(&command, &["document=/work/p/b.snxpcb", &path])
            .expect("operands")
    }

    #[cfg(unix)]
    #[test]
    fn only_a_plain_absolute_path_passes_the_gate() {
        assert_eq!(path_refusal(&args("/work/p/out/board.pdf")), None);
        // Containment is the writer's: the gate does not read the disk.
        assert_eq!(path_refusal(&args("/work/other/board.pdf")), None);
        let why = path_refusal(&args("/work/p/../other/board.pdf")).expect("refused");
        assert!(why.contains("without `..`"), "{why}");
    }
}
