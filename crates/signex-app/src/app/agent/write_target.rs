//! Where an agent's file writes may land (#2470): the `path` of a command in
//! `allow::WRITES_IN_PROJECT` lies inside the folder of the project its
//! `document` belongs to. The gate checks the path's text, so `update` reads
//! no disk; the job that writes checks it again on disk with links followed
//! (`notebook_project::confined_write`), the rule a notebook export of an
//! agent already follows. See `docs/AGENT_SOCKET.md`, "Allowlist".

use std::path::{Component, Path, PathBuf};

use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};

/// Why the `path` of `args` may not be written by an agent of the project in
/// `project_dir`, if it may not: it must be absolute, hold no `..`, and lie
/// inside that folder. A missing `path` is left to the handler,
/// which refuses it with its own reason.
pub(super) fn path_refusal(args: &CommandArgs, project_dir: &Path) -> Option<String> {
    let Some(OperandValue::Path(text)) = args.operand("path") else {
        return None;
    };
    let path = Path::new(text);
    let plain = path.is_absolute()
        && path
            .components()
            .all(|part| !matches!(part, Component::CurDir | Component::ParentDir));
    if plain && path.starts_with(project_dir) {
        return None;
    }
    Some(format!(
        "`path` {} is not inside the project folder {}: an agent writes only there",
        path.display(),
        project_dir.display()
    ))
}

impl Signex {
    /// The folder an agent's write to `target` is confined to when it runs:
    /// `None` outside an agent's request; else the open project folder that
    /// holds `target`, or an empty path when none does, which
    /// `confined_write` refuses for every target.
    pub(crate) fn agent_write_root(&self, target: &Path) -> Option<PathBuf> {
        if !self.agent.serving {
            return None;
        }
        let holder = self
            .document_state
            .projects
            .iter()
            .map(|project| project.dir())
            .filter(|dir| target.starts_with(dir))
            .max_by_key(|dir| dir.as_os_str().len());
        Some(holder.map(Path::to_path_buf).unwrap_or_default())
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
    fn only_a_plain_absolute_path_inside_the_folder_passes() {
        let dir = Path::new("/work/p");
        assert_eq!(path_refusal(&args("/work/p/out/board.pdf"), dir), None);
        for outside in [
            "/work/other/board.pdf",
            "/work/p/../other/board.pdf",
            "/work/pp/board.pdf",
        ] {
            let why = path_refusal(&args(outside), dir).expect(outside);
            assert!(why.contains("not inside the project folder"), "{why}");
        }
    }
}
