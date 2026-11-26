//! One project's Design Notebook as a document being edited (#2795, mockup
//! row 195): where the file stands, the file as read or last saved, the
//! body as typed and the chapters it includes. UI-free: the notebook tab
//! keeps the editor, the preview and the view beside it, and copies the
//! body here after every edit (`docs/DESIGN_NOTEBOOK_TAB.md`).
//!
//! The text is held with `\n` line ends; a file read with `\r\n` is written
//! back with `\r\n`, so an edit never changes every line of it.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::DEFAULT_NOTEBOOK_FILE;
use crate::file::{NotebookFile, NotebookFileError};
use crate::sources::NotebookSources;
use crate::status::header_line_count;

/// Where the notebook file stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotebookLoad {
    /// The read is running.
    Reading,
    /// There is no notebook file yet: the empty state.
    Missing,
    /// The file is open for editing.
    Open,
    /// The file is there and could not be read; nothing is edited, so
    /// nothing can overwrite it.
    Failed(String),
}

/// A notebook being edited, kept while its project is loaded, so a closed
/// tab keeps its unsaved text.
#[derive(Debug, Clone)]
pub struct NotebookDocument {
    /// The notebook file: `<project>/notebook/design.snxnbk`.
    pub path: PathBuf,
    pub load: NotebookLoad,
    /// The file as read or last saved; `None` for a notebook not saved yet.
    pub file: Option<NotebookFile>,
    /// The body the file on disk holds; `None` before the first save. The
    /// body is unsaved while it differs.
    pub saved_body: Option<String>,
    /// The body as typed (after the header); `None` while there is no text
    /// to edit.
    pub body: Option<String>,
    /// How many lines the header takes in the file: line numbers count from
    /// one past them.
    pub header_lines: u32,
    /// The chapters the notebook includes, as read, by path inside the
    /// notebook folder; the notebook file itself is replaced by the body as
    /// typed before each compile.
    pub sources: NotebookSources,
    /// A save is running.
    pub saving: bool,
    /// Why the last save failed, until the next one.
    pub save_error: Option<String>,
    /// When the last save landed; the view says it in the person's clock.
    pub saved_at: Option<SystemTime>,
    /// The file was read with `\r\n` line ends, and is written with them.
    pub crlf: bool,
}

/// `text` with `\n` line ends.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

impl NotebookDocument {
    /// A notebook whose file is being read.
    pub fn reading(path: PathBuf) -> Self {
        NotebookDocument {
            path,
            load: NotebookLoad::Reading,
            file: None,
            saved_body: None,
            body: None,
            header_lines: 0,
            sources: NotebookSources::new(DEFAULT_NOTEBOOK_FILE, Vec::new()),
            saving: false,
            save_error: None,
            saved_at: None,
            crlf: false,
        }
    }

    /// True when the body as typed is not what the file holds.
    pub fn is_unsaved(&self) -> bool {
        match &self.body {
            Some(body) => self.saved_body.as_deref() != Some(body.as_str()),
            None => false,
        }
    }

    /// True when the file was written by a newer Signex: it opens for
    /// viewing and Save is refused.
    pub fn is_read_only(&self) -> bool {
        self.file.as_ref().is_some_and(NotebookFile::is_read_only)
    }

    /// True when the notebook was never saved: its first Save makes the
    /// file, and must not replace one written meanwhile.
    pub fn is_new(&self) -> bool {
        self.saved_body.is_none()
    }

    /// The sources a compile reads: the chapters as read and the notebook
    /// file as typed, its header lines blank so every line keeps its number.
    pub fn compile_sources(&self) -> NotebookSources {
        let mut sources = self.sources.clone();
        if let Some(body) = &self.body {
            let blank = "\n".repeat(self.header_lines as usize);
            // The main path is always inside the notebook folder.
            let _inserted = sources.insert(DEFAULT_NOTEBOOK_FILE, format!("{blank}{body}"));
        }
        sources
    }

    /// The 1-based line of the file for 0-based body line `line`.
    pub fn file_line(&self, line: usize) -> u32 {
        let line = u32::try_from(line).unwrap_or(u32::MAX);
        line.saturating_add(self.header_lines).saturating_add(1)
    }

    /// The 0-based body line for 1-based file line `line`; `None` for a
    /// line in the header.
    pub fn body_line(&self, line: u32) -> Option<usize> {
        let line = line.checked_sub(self.header_lines + 1)?;
        usize::try_from(line).ok()
    }

    /// The file `text` was read: its body is the text to edit.
    pub fn open_text(&mut self, text: &str) -> Result<(), NotebookFileError> {
        self.crlf = text.contains("\r\n");
        let text = lf(text);
        let file = NotebookFile::parse(&text)?;
        self.header_lines = header_line_count(&text);
        self.body = Some(file.body.clone());
        self.saved_body = Some(file.body.clone());
        self.file = Some(file);
        self.load = NotebookLoad::Open;
        Ok(())
    }

    /// An empty notebook now holds the new `file`, not saved.
    pub fn start_new(&mut self, file: NotebookFile) {
        self.body = Some(lf(&file.body));
        self.header_lines = 0;
        self.saved_body = None;
        self.file = Some(file);
        self.load = NotebookLoad::Open;
    }

    /// The whole file text `text` replaces the body and the header, as a
    /// reference fix writes it; written on Save like any edit.
    pub fn replace_file_text(&mut self, text: &str) -> Result<(), NotebookFileError> {
        let text = lf(text);
        let file = NotebookFile::parse(&text)?;
        self.header_lines = header_line_count(&text);
        self.body = Some(file.body.clone());
        self.file = Some(file);
        Ok(())
    }

    /// The file text Save writes: the file (or a new one) with the body as
    /// typed, in the line ends the file was read with. `None` when there
    /// is nothing to write.
    pub fn written_text(&self) -> Option<Result<String, NotebookFileError>> {
        let body = self.body.as_ref()?;
        let mut file = self
            .file
            .clone()
            .unwrap_or_else(|| NotebookFile::new(String::new()));
        file.body = lf(body);
        let crlf = self.crlf;
        Some(file.write().map(|text| {
            let text = lf(&text);
            if crlf {
                text.replace('\n', "\r\n")
            } else {
                text
            }
        }))
    }

    /// The file now holds `written`: keep it as the file read, so the next
    /// save of an unchanged header writes it back byte for byte. `at` is
    /// when the save landed.
    pub fn mark_written(&mut self, written: &str, at: SystemTime) -> Result<(), NotebookFileError> {
        self.load = NotebookLoad::Open;
        self.saved_at = Some(at);
        let written = lf(written);
        let file = NotebookFile::parse(&written)?;
        self.saved_body = Some(file.body.clone());
        self.header_lines = header_line_count(&written);
        self.file = Some(file);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRLF: &str = "+++\r\nformat = \"snxnbk/1\"\r\n+++\r\n= Power\r\nText\r\n";

    #[test]
    fn a_crlf_file_is_held_with_lf_and_written_back_with_crlf() {
        let mut doc = NotebookDocument::reading(PathBuf::from("design.snxnbk"));
        doc.open_text(CRLF).expect("opens");
        assert!(!doc.body.as_deref().unwrap_or_default().contains('\r'));
        assert!(!doc.is_unsaved());
        assert_eq!(
            doc.written_text().expect("text").expect("writes"),
            CRLF,
            "unchanged, byte for byte"
        );
        doc.body = Some("= Power\nText more\n".into());
        let written = doc.written_text().expect("text").expect("writes");
        assert!(written.contains("Text more\r\n"), "{written:?}");
        assert!(
            !written.replace("\r\n", "").contains('\n'),
            "no bare LF: {written:?}"
        );
        doc.mark_written(&written, SystemTime::UNIX_EPOCH)
            .expect("reads back");
        assert!(!doc.is_unsaved());
    }
}
