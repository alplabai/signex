//! The block diagram tab's second view and its export popover (mockup row
//! 171): which view the tab shows, the project's power tree as the last run
//! read it, and the export settings being filled in. Pure data.

use signex_net::power_tree::{PowerTopology, PowerTree};

/// What the tab shows: the diagram, or the project's power tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagramView {
    #[default]
    Blocks,
    PowerTree,
}

/// One power tree run: the parts on the power nets and the tree built from
/// them, for one variant.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerTreeRead {
    /// The variant read; `None` is Base.
    pub variant: Option<String>,
    pub topology: PowerTopology,
    pub tree: PowerTree,
    /// What the run could not read, in its own words.
    pub warnings: Vec<String>,
}

/// The Power tree view's state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PowerTreeView {
    /// A run is going; its result replaces `read`.
    pub loading: bool,
    /// The last run's tree, or why it did not run.
    pub read: Option<Result<PowerTreeRead, String>>,
    /// The rail Properties shows.
    pub rail: Option<String>,
}

/// The export popover's file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Svg,
    Pdf,
}

impl ExportKind {
    /// The `kind` operand and file extension.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }
}

/// The export popover as filled in so far.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportDraft {
    pub kind: ExportKind,
    /// The file, relative to the diagram's folder unless absolute.
    pub file: String,
    /// Print colours: every mark black.
    pub monochrome: bool,
    /// The PDF title block; an SVG has none.
    pub title_block: bool,
}

impl ExportDraft {
    /// The popover for `kind`, offering `outputs/<stem>_blocks.<ext>`.
    pub fn new(kind: ExportKind, stem: &str) -> Self {
        Self {
            kind,
            file: draft_file(stem, kind),
            monochrome: false,
            title_block: true,
        }
    }

    /// Switch the format; a file still named as offered follows it.
    pub fn pick_kind(&mut self, kind: ExportKind, stem: &str) {
        if self.file == draft_file(stem, self.kind) {
            self.file = draft_file(stem, kind);
        }
        self.kind = kind;
    }
}

fn draft_file(stem: &str, kind: ExportKind) -> String {
    format!("outputs/{stem}_blocks.{}", kind.token())
}
