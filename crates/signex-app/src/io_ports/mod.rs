//! The two native side effects a command can ask for, behind one port each
//! (#1931): a file picker and an opener (browser, file viewer, file manager).
//!
//! The GUI uses the native adapters in [`native`], the only module allowed to
//! name `rfd` or `webbrowser` or start a launcher process
//! (`scripts/check_io_ports.sh`). A headless run takes picker answers from
//! plain arguments and refuses every launch, reporting why.

mod native;
mod picker;

pub(crate) use picker::replace_refusal;
pub use picker::{FileDialog, HeadlessAnswers, Picker};

use std::path::Path;

/// Ask a two-answer question in a native dialog; `true` when `yes` is
/// pressed. Only the GUI boot asks (the one-time macOS shortcut prompt), so a
/// headless run never reaches it.
pub(crate) async fn ask_two(title: &str, description: &str, yes: &str, no: &str) -> bool {
    native::ask_two(title, description, yes, no).await
}

/// The ports `update` hands to the tasks it returns.
#[derive(Debug, Clone, Default)]
pub struct IoPorts {
    pub picker: Picker,
    pub opener: Opener,
    pub git: GitCommits,
}

impl IoPorts {
    /// A headless run: dialogs answered from `answers` (replaced per command,
    /// see [`HeadlessAnswers::replace_all`]), launches refused, no git.
    pub fn headless(answers: HeadlessAnswers) -> Self {
        Self {
            picker: Picker::Headless(answers),
            opener: Opener::Refuse,
            git: GitCommits::Skip,
        }
    }
}

/// Whether a save of a project with git enabled commits to its repository.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GitCommits {
    /// The project's own repository: opened, created if missing, committed to.
    #[default]
    Native,
    /// A headless run never opens, creates or commits to a repository: where
    /// a repository is (a `gitdir:` file, a link, config includes) is the
    /// project folder's say, not the caller's, and the commit would write
    /// outside it.
    Skip,
}

/// Where a browser or viewer launch goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Opener {
    /// The platform's browser, viewer and file manager.
    #[default]
    Native,
    /// A headless run: nothing is launched and the caller reports why.
    Refuse,
}

/// Why a launch did not happen.
#[derive(Debug)]
pub enum OpenError {
    /// A headless run launches nothing.
    Refused,
    /// The launcher failed to start or reported a failure.
    Failed(std::io::Error),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused => f.write_str("a headless run does not launch browsers or viewers"),
            Self::Failed(error) => write!(f, "{error}"),
        }
    }
}

impl Opener {
    /// Open `url` in the default browser. Blocks on the launcher.
    pub fn browse(self, url: &str) -> Result<(), OpenError> {
        self.launch(|| native::browse(url))
    }

    /// Open `path` in its default viewer. Blocks on the launcher.
    pub fn view(self, path: &Path) -> Result<(), OpenError> {
        self.launch(|| native::view(path))
    }

    /// Show `path` in the file manager.
    pub fn reveal(self, path: &Path) -> Result<(), OpenError> {
        self.launch(|| native::reveal(path))
    }

    fn launch(self, native: impl FnOnce() -> std::io::Result<()>) -> Result<(), OpenError> {
        match self {
            Self::Native => native().map_err(OpenError::Failed),
            Self::Refuse => Err(OpenError::Refused),
        }
    }
}

#[cfg(test)]
mod headless_tests;
