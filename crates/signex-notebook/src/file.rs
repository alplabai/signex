//! One `.snxnbk` file: its header and its Typst body, read and written
//! byte for byte.
//!
//! The file is the header between two `+++` lines, then Typst source:
//!
//! ```text
//! +++
//! format = "snxnbk/1"
//!
//! [[references]]
//! kind = "component"
//! component = "R7"
//! id = "0193a0c4-…"
//! +++
//! #import "@signex/notebook:0.1.0": *
//! = Power stage
//! ```
//!
//! A file saved with no change is written back exactly as it was read, its
//! header text included; a header that changed is written in the canonical
//! form of [`Header::to_toml`].

use crate::header::{Header, HeaderError, NOTEBOOK_FORMAT};
use crate::scan::{FrontMatter, front_matter_end};

/// The line that opens and closes the header.
pub const HEADER_FENCE: &str = "+++";

/// A notebook file in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct NotebookFile {
    /// The header: the stored reference ids and any keys kept for a newer
    /// build.
    pub header: Header,
    /// The Typst source after the header, as written.
    pub body: String,
    /// The token the file was read as.
    token: String,
    /// The header exactly as read, kept for a no-change save.
    as_read: Option<ReadHeader>,
    /// Whether the file is written with a header even when it is empty.
    keeps_header: bool,
}

/// The bytes around and inside the header as read, and the header they
/// gave.
#[derive(Debug, Clone, PartialEq)]
struct ReadHeader {
    /// Everything before the TOML: a byte order mark and the opening fence.
    open: String,
    toml: String,
    /// The closing fence line.
    close: String,
    header: Header,
}

/// Why a notebook file could not be read. The caller adds the file name.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NotebookFileError {
    /// The header's opening `+++` has no closing one.
    #[error("the notebook header has no closing +++ line")]
    UnclosedHeader,
    #[error(transparent)]
    Header(#[from] HeaderError),
}

impl NotebookFile {
    /// A new notebook with `body` as its Typst source. It is written with a
    /// header carrying the format token.
    pub fn new(body: impl Into<String>) -> Self {
        Self {
            header: Header::default(),
            body: body.into(),
            token: NOTEBOOK_FORMAT.current.to_string(),
            as_read: None,
            keeps_header: true,
        }
    }

    /// Read `source`, the whole text of a `.snxnbk` file. A file with no
    /// header reads as `snxnbk/1` with an empty header.
    pub fn parse(source: &str) -> Result<Self, NotebookFileError> {
        match front_matter_end(source) {
            FrontMatter::None => Ok(Self {
                header: Header::default(),
                body: source.to_string(),
                token: NOTEBOOK_FORMAT.current.to_string(),
                as_read: None,
                keeps_header: false,
            }),
            FrontMatter::Unclosed => Err(NotebookFileError::UnclosedHeader),
            FrontMatter::Closed { toml, body, .. } => {
                let (header, token) = Header::read(&source[toml.clone()])?;
                Ok(Self {
                    header: header.clone(),
                    body: source[body..].to_string(),
                    token,
                    as_read: Some(ReadHeader {
                        open: source[..toml.start].to_string(),
                        toml: source[toml.clone()].to_string(),
                        close: source[toml.end..body].to_string(),
                        header,
                    }),
                    keeps_header: true,
                })
            }
        }
    }

    /// The token the file was read as (`snxnbk/1`).
    pub fn token(&self) -> &str {
        &self.token
    }

    /// True when a newer Signex wrote the file: it opens for viewing and
    /// [`Self::write`] refuses it.
    pub fn is_read_only(&self) -> bool {
        NOTEBOOK_FORMAT.is_newer_minor(&self.token)
    }

    /// The whole file text. Unchanged since it was read, it is the text
    /// that was read; else the header is written canonically, and a file
    /// that had no header gets one only once it stores something. A file a
    /// newer Signex wrote is refused, so nothing it holds is dropped.
    pub fn write(&self) -> Result<String, NotebookFileError> {
        NOTEBOOK_FORMAT
            .refuse_save(&self.token)
            .map_err(HeaderError::from)?;
        if let Some(read) = &self.as_read
            && read.header == self.header
        {
            let ReadHeader {
                open,
                toml: text,
                close,
                ..
            } = read;
            return Ok(format!("{open}{text}{close}{}", self.body));
        }
        if !self.keeps_header && self.header.is_empty() {
            return Ok(self.body.clone());
        }
        Ok(format!(
            "{HEADER_FENCE}\n{}{HEADER_FENCE}\n{}",
            self.header.to_toml(),
            self.body
        ))
    }
}
