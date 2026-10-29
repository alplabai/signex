//! "Commit now": retry the git commit of saved changes no commit holds (#858).
//!
//! A save whose file write succeeded stays a save when its commit fails; the
//! adapter lists the file as uncommitted and the browser shows a banner. This
//! is that banner's button. The commit runs off the update thread, like every
//! git write, and reports to the Messages panel.

use iced::Task;

use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::state::CommitNowReport;
use signex_library::CommitOutcome;

impl Signex {
    pub(super) fn handle_browser_commit_pending(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        let Some(adapter) = self.library.begin_commit_pending(&library_path) else {
            return Task::none();
        };
        let done_path = library_path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || adapter.commit_pending(""))
                    .await
                    .map_err(|e| format!("commit task failed: {e}"))
                    .and_then(|result| result.map_err(|e| e.to_string()))
            },
            move |result: Result<CommitOutcome, String>| {
                Message::Library(LibraryMessage::BrowserCommitPendingDone {
                    library_path: done_path.clone(),
                    result,
                })
            },
        )
    }

    pub(super) fn handle_browser_commit_pending_done(
        &mut self,
        library_path: &std::path::Path,
        result: Result<CommitOutcome, String>,
    ) -> Task<Message> {
        let name = library_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| library_path.display().to_string());
        match self.library.finish_commit_pending(library_path, result) {
            CommitNowReport::Committed => {
                crate::diagnostics::log_info(format!("Committed the saved changes of {name}."));
            }
            CommitNowReport::NothingToCommit => {}
            CommitNowReport::StillNotCommitted(reason) => {
                crate::diagnostics::log_warning(format!(
                    "Commit now failed for {name}: {reason}. The changes stay saved on disk."
                ));
            }
            CommitNowReport::Failed(error) => {
                crate::diagnostics::log_warning(format!("Commit now failed for {name}: {error}."));
            }
        }
        Task::none()
    }
}
