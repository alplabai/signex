//! The project a board belongs to: the one Run DRC and `check_drc` read
//! net classes and rules from (#2332).

use std::path::Path;

use super::CaseFold;
use super::{DocumentState, LoadedProject};
use crate::app::document_ids::{DocumentKind, referenced_files};
use crate::app::sheet_hierarchy::normalised_key;
use crate::recent_projects::strip_verbatim_prefix;

impl DocumentState {
    /// The project of `board`: the one its document entry names (`named`),
    /// else the open project whose `pcb_file` is the board; `None` for a
    /// loose board. An open board's entry names its project too (#2331); the
    /// fallback covers an entry that names none. Paths compare
    /// by [`board_path_key`], so `\\?\` and `sub/../` spellings still match.
    pub(crate) fn board_project(
        &self,
        board: &Path,
        named: Option<&Path>,
    ) -> Option<&LoadedProject> {
        if let Some(named) = named {
            return self.projects.iter().find(|project| {
                let key = |path: &Path| board_path_key(path, project.case_fold);
                key(&project.path) == key(named)
            });
        }
        // Two open projects listing the same `pcb_file`: the first loaded wins.
        self.projects.iter().find(|project| {
            let key = |path: &Path| board_path_key(path, project.case_fold);
            referenced_files(project)
                .iter()
                .any(|(path, kind)| *kind == DocumentKind::Pcb && key(path) == key(board))
        })
    }
}

/// `path` as `board_project` compares it: the Windows `\\?\` prefix dropped,
/// `.` and `..` resolved lexically, then the volume's key.
fn board_path_key(path: &Path, fold: CaseFold) -> String {
    normalised_key(&strip_verbatim_prefix(path.to_path_buf()), fold)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::app::handlers::menu::export::tests::app_workspace;

    /// A project at `/w/a` whose `pcb_file` is `board.snxpcb`.
    fn with_board() -> crate::app::Signex {
        let mut app = app_workspace("/w/a", &["top.snxsch"]);
        app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".to_string());
        app
    }

    #[test]
    fn a_board_spelled_through_a_dot_dot_segment_finds_its_project() {
        let app = with_board();
        let docs = &app.document_state;
        let board = PathBuf::from("/w/a/sub/../board.snxpcb");
        assert!(docs.board_project(&board, None).is_some());
        let named = PathBuf::from("/w/a/sub/../ProjectA.snxprj");
        assert!(docs.board_project(&board, Some(&named)).is_some());
        let other = Path::new("/w/b/sub/../board.snxpcb");
        assert!(docs.board_project(other, None).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn a_board_spelled_with_the_verbatim_prefix_finds_its_project() {
        let mut app = app_workspace(r"C:\w\a", &["top.snxsch"]);
        app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".to_string());
        let docs = &app.document_state;
        let board = Path::new(r"\\?\C:\w\a\board.snxpcb");
        assert!(docs.board_project(board, None).is_some());
        let named = Path::new(r"\\?\C:\w\a\ProjectA.snxprj");
        assert!(docs.board_project(board, Some(named)).is_some());
    }
}
