//! The file-picker port: one builder for every open, save and folder dialog.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::native;

/// The answer naming the file or folder a dialog picks.
pub const PATH_ARG: &str = "path";
/// The answer naming the files a multi-select dialog picks, one per line.
pub const PATHS_ARG: &str = "paths";
/// `true` lets a save dialog's answer replace an existing file.
pub const OVERWRITE_ARG: &str = "overwrite";

/// Where dialogs get their answer.
#[derive(Debug, Clone, Default)]
pub enum Picker {
    /// The OS file dialogs.
    #[default]
    Native,
    /// A headless run: answers from plain arguments, refused when missing.
    Headless(HeadlessAnswers),
}

/// The plain arguments a headless run answers dialogs with. Each command's
/// answers replace the last command's ([`Self::replace_all`], fed from
/// `CommandArgs` by #1933), so a command that opens no dialog cannot leave an
/// answer behind. A dialog takes every answer there is, whatever it uses and
/// whether it is refused, so a later dialog never inherits an earlier path or
/// `overwrite`.
///
/// A path answer must be absolute but is not normalised (`..` stays): code
/// that confines paths must resolve it, never prefix-match the raw text.
#[derive(Debug, Clone, Default)]
pub struct HeadlessAnswers(Arc<Mutex<BTreeMap<String, String>>>);

impl HeadlessAnswers {
    /// The answers of the command about to run (`path`, `paths`,
    /// `overwrite`); whatever an earlier command left is dropped. Call it
    /// before every command, with no answers when it has none.
    pub fn replace_all(&self, answers: impl IntoIterator<Item = (String, String)>) {
        *self.lock() = answers.into_iter().collect();
    }

    /// The answer named `name` of the command about to run, left in place.
    pub(crate) fn peek(&self, name: &str) -> Option<String> {
        self.lock().get(name).cloned()
    }

    /// Add one answer to the current command's set.
    #[cfg(test)]
    pub(crate) fn set(&self, name: &str, value: impl Into<String>) {
        self.lock().insert(name.to_string(), value.into());
    }

    fn take_all(&self) -> BTreeMap<String, String> {
        std::mem::take(&mut *self.lock())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, String>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Picker {
    /// A dialog with no title, filter, folder or name yet.
    pub fn dialog(&self) -> FileDialog {
        FileDialog {
            picker: self.clone(),
            title: String::new(),
            filters: Vec::new(),
            directory: None,
            file_name: None,
        }
    }
}

/// One dialog, built the way the native async file dialog is.
#[derive(Debug, Clone)]
pub struct FileDialog {
    picker: Picker,
    pub(super) title: String,
    pub(super) filters: Vec<(String, Vec<String>)>,
    pub(super) directory: Option<PathBuf>,
    pub(super) file_name: Option<String>,
}

impl FileDialog {
    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[impl ToString]) -> Self {
        let extensions = extensions.iter().map(ToString::to_string).collect();
        self.filters.push((name.into(), extensions));
        self
    }

    pub fn set_directory(mut self, dir: impl AsRef<Path>) -> Self {
        self.directory = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    /// The file picked to open; `None` when cancelled or refused.
    pub async fn pick_file(self) -> Option<PathBuf> {
        match &self.picker {
            Picker::Native => native::pick_file(&self).await,
            Picker::Headless(answers) => self.headless_path(&mut answers.take_all()),
        }
    }

    /// The files picked to open; `None` when cancelled or refused.
    pub async fn pick_files(self) -> Option<Vec<PathBuf>> {
        match &self.picker {
            Picker::Native => native::pick_files(&self).await,
            Picker::Headless(answers) => {
                let list = answers.take_all().remove(PATHS_ARG).unwrap_or_default();
                let lines: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
                // The native dialog never returns an empty pick.
                if lines.is_empty() {
                    return self.refuse(&format!("give the `{PATHS_ARG}` argument"));
                }
                lines
                    .into_iter()
                    .map(|line| self.absolute(PathBuf::from(line)))
                    .collect()
            }
        }
    }

    /// The folder picked; `None` when cancelled or refused.
    pub async fn pick_folder(self) -> Option<PathBuf> {
        match &self.picker {
            Picker::Native => native::pick_folder(&self).await,
            Picker::Headless(answers) => self.headless_path(&mut answers.take_all()),
        }
    }

    /// The file to save to; `None` when cancelled or refused. Headless, an
    /// existing file is replaced only with `overwrite=true`, where the OS
    /// dialog would ask.
    pub async fn save_file(self) -> Option<PathBuf> {
        let answers = match &self.picker {
            Picker::Native => return native::save_file(&self).await,
            Picker::Headless(answers) => answers,
        };
        let mut taken = answers.take_all();
        let may_replace = taken.remove(OVERWRITE_ARG).as_deref() == Some("true");
        let path = self.headless_path(&mut taken)?;
        // `symlink_metadata`, not `exists`: a dangling link is still replaced.
        if std::fs::symlink_metadata(&path).is_ok() && !may_replace {
            return self.refuse(&format!(
                "{} exists; give `{OVERWRITE_ARG}=true` to replace it",
                path.display()
            ));
        }
        Some(path)
    }

    fn headless_path(&self, taken: &mut BTreeMap<String, String>) -> Option<PathBuf> {
        match taken.remove(PATH_ARG) {
            Some(path) => self.absolute(PathBuf::from(path)),
            None => self.refuse(&format!("give the `{PATH_ARG}` argument")),
        }
    }

    /// A relative answer would depend on the process's working folder.
    fn absolute(&self, path: PathBuf) -> Option<PathBuf> {
        if path.is_absolute() {
            Some(path)
        } else {
            self.refuse(&format!("{} is not an absolute path", path.display()))
        }
    }

    fn refuse<T>(&self, advice: &str) -> Option<T> {
        crate::diagnostics::log_warning(format!(
            "{}: refused in a headless run; {advice}",
            self.title_or_default()
        ));
        None
    }

    fn title_or_default(&self) -> &str {
        if self.title.is_empty() {
            "File dialog"
        } else {
            &self.title
        }
    }
}
