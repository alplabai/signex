//! Why a library import did not happen, as data; the text is in
//! `library.ftl` so the dialog shows it in the user's language.

use std::path::PathBuf;

use signex_i18n::{RenderTranslatable as _, Translatable, ids};

/// A refused or failed library import step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// No program answered as a library converter; `env` is the variable
    /// that can list them.
    NoConverters {
        env: String,
    },
    OverrideNotAFile {
        env: String,
        path: PathBuf,
    },
    /// `describe` did not run or printed something unreadable.
    DescribeFailed {
        converter: String,
        detail: String,
    },
    /// The converter speaks a contract major this build does not.
    UnsupportedContract {
        converter: String,
        contract: String,
    },
    /// Inputs and folders are passed as absolute paths only, so none can
    /// be read as an option.
    PathNotAbsolute {
        path: PathBuf,
    },
    StartFailed {
        program: PathBuf,
        detail: String,
    },
    Cancelled,
    /// No event for `seconds`; the converter was stopped.
    Silent {
        seconds: u64,
    },
    /// The converter's own `error`.
    ConverterFailed {
        message: String,
    },
    /// The run ended without a summary as its last line.
    NoSummary {
        status: String,
        stderr: String,
    },
    /// Exit code and summary disagree.
    ExitMismatch {
        code: String,
        exit: String,
    },
    PathOutside {
        path: PathBuf,
        output: PathBuf,
    },
    /// The summary's `library` is not the `--output` folder.
    LibraryNotOutput {
        library: PathBuf,
        output: PathBuf,
    },
    /// An update was asked for a folder that is not an imported library's
    /// repository.
    NotAnImportedLibrary {
        dir: PathBuf,
    },
    /// An update was asked while the library has unsaved work in the app.
    UnsavedInApp {
        what: String,
    },
    PathUnreadable {
        path: PathBuf,
        detail: String,
    },
    WaitFailed {
        detail: String,
    },
    /// An update would overwrite these uncommitted paths.
    UncommittedChanges {
        paths: Vec<String>,
    },
    /// The library was written but its git setup or commit failed.
    Git {
        detail: String,
    },
    /// The reported library folder holds no `.snxlib`.
    NoLibraryFile {
        dir: PathBuf,
    },
    /// A new import's folder already holds something.
    OutputNotEmpty {
        path: PathBuf,
    },
    /// A new import's folder could not be created.
    OutputUnusable {
        path: PathBuf,
        detail: String,
    },
    /// A new import's folder is a symbolic link; the run never writes
    /// through one.
    OutputIsLink {
        path: PathBuf,
    },
}

/// How many uncommitted paths the refusal names.
const LISTED_PATHS: usize = 5;

impl ImportError {
    pub fn to_translatable(&self) -> Translatable {
        let path = |p: &PathBuf| p.display().to_string();
        match self {
            Self::NoConverters { env } => {
                Translatable::new(ids::LIBRARY_IMPORT_NOT_INSTALLED).arg("env", env.as_str())
            }
            Self::OverrideNotAFile { env, path: p } => {
                Translatable::new(ids::LIBRARY_IMPORT_OVERRIDE_NOT_A_FILE)
                    .arg("env", env.as_str())
                    .arg("path", path(p))
            }
            Self::DescribeFailed { converter, detail } => {
                Translatable::new(ids::LIBRARY_IMPORT_DESCRIBE_FAILED)
                    .arg("converter", converter.as_str())
                    .arg("error", detail.as_str())
            }
            Self::UnsupportedContract {
                converter,
                contract,
            } => Translatable::new(ids::LIBRARY_IMPORT_UNSUPPORTED_CONTRACT)
                .arg("converter", converter.as_str())
                .arg("contract", contract.as_str()),
            Self::PathNotAbsolute { path: p } => {
                Translatable::new(ids::LIBRARY_IMPORT_PATH_NOT_ABSOLUTE).arg("path", path(p))
            }
            Self::StartFailed { program, detail } => {
                Translatable::new(ids::LIBRARY_IMPORT_START_FAILED)
                    .arg("path", path(program))
                    .arg("error", detail.as_str())
            }
            Self::Cancelled => Translatable::new(ids::LIBRARY_IMPORT_CANCELLED),
            Self::Silent { seconds } => {
                Translatable::new(ids::LIBRARY_IMPORT_SILENT).arg("seconds", *seconds)
            }
            Self::ConverterFailed { message } => {
                Translatable::new(ids::LIBRARY_IMPORT_CONVERTER_FAILED)
                    .arg("error", message.as_str())
            }
            Self::NoSummary { status, stderr } => Translatable::new(ids::LIBRARY_IMPORT_NO_SUMMARY)
                .arg("status", status.as_str())
                .arg("stderr", stderr.as_str()),
            Self::ExitMismatch { code, exit } => {
                Translatable::new(ids::LIBRARY_IMPORT_EXIT_MISMATCH)
                    .arg("code", code.as_str())
                    .arg("exit", exit.as_str())
            }
            Self::PathOutside { path: p, output } => {
                Translatable::new(ids::LIBRARY_IMPORT_PATH_OUTSIDE)
                    .arg("path", path(p))
                    .arg("output", path(output))
            }
            Self::LibraryNotOutput { library, output } => {
                Translatable::new(ids::LIBRARY_IMPORT_LIBRARY_NOT_OUTPUT)
                    .arg("path", path(library))
                    .arg("output", path(output))
            }
            Self::NotAnImportedLibrary { dir } => {
                Translatable::new(ids::LIBRARY_IMPORT_NOT_A_REPOSITORY).arg("path", path(dir))
            }
            Self::UnsavedInApp { what } => {
                Translatable::new(ids::LIBRARY_IMPORT_UNSAVED).arg("what", what.as_str())
            }
            Self::PathUnreadable { path: p, detail } => {
                Translatable::new(ids::LIBRARY_IMPORT_PATH_UNREADABLE)
                    .arg("path", path(p))
                    .arg("error", detail.as_str())
            }
            Self::WaitFailed { detail } => {
                Translatable::new(ids::LIBRARY_IMPORT_WAIT_FAILED).arg("error", detail.as_str())
            }
            Self::UncommittedChanges { paths } => {
                let mut listed = paths
                    .iter()
                    .take(LISTED_PATHS)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                if paths.len() > LISTED_PATHS {
                    listed.push_str(", …");
                }
                Translatable::new(ids::LIBRARY_IMPORT_UNCOMMITTED)
                    .arg("count", paths.len())
                    .arg("paths", listed)
            }
            Self::Git { detail } => {
                Translatable::new(ids::LIBRARY_IMPORT_GIT_FAILED).arg("error", detail.as_str())
            }
            Self::NoLibraryFile { dir } => {
                Translatable::new(ids::LIBRARY_IMPORT_NO_LIBRARY_FILE).arg("path", path(dir))
            }
            Self::OutputNotEmpty { path: p } => {
                Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_NOT_EMPTY).arg("path", path(p))
            }
            Self::OutputIsLink { path: p } => {
                Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_LINK).arg("path", path(p))
            }
            Self::OutputUnusable { path: p, detail } => {
                Translatable::new(ids::LIBRARY_IMPORT_DIALOG_TARGET_UNREADABLE)
                    .arg("path", path(p))
                    .arg("error", detail.as_str())
            }
        }
    }
}

impl std::fmt::Display for ImportError {
    /// en-US, for the log; the UI renders [`ImportError::to_translatable`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_translatable().render_en_us())
    }
}

impl std::error::Error for ImportError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_renders_its_arguments() {
        let silent = ImportError::Silent { seconds: 120 }.to_string();
        assert!(silent.contains("120"), "{silent}");
        let dirty = ImportError::UncommittedChanges {
            paths: (0..7).map(|i| format!("f{i}.snxsym")).collect(),
        }
        .to_string();
        assert!(
            dirty.contains("f4.snxsym") && !dirty.contains("f5.snxsym"),
            "{dirty}"
        );
        assert!(dirty.contains('7'), "{dirty}");
        let contract = ImportError::UnsupportedContract {
            converter: "c".into(),
            contract: "2.0".into(),
        }
        .to_string();
        assert!(contract.contains("2.0"), "{contract}");
    }
}
