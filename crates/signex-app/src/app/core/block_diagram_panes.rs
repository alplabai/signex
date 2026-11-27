//! The block diagram tab's second view and its export popover (mockup row
//! 171): which view the tab shows, the project's power tree as the last run
//! read it, the export settings being filled in, and the Re-sync review
//! (mockup row 210). Pure data.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_net::SheetKey;
use signex_net::power_tree::{PowerTopology, PowerTree};
use uuid::Uuid;

/// The review a Re-sync opens when a block would become unlinked (mockup
/// row 210). The added and the renamed or moved blocks are already applied
/// (one undo step) and are previewed while it is open; each block whose
/// sheet left the project has a row, checked until unticked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResyncReview {
    pub added: Vec<Uuid>,
    pub moved: Vec<Uuid>,
    /// Of `moved`, the blocks whose sheet file was renamed (its name, not
    /// only its folder, changed); tagged "renamed" rather than "moved".
    pub renamed: Vec<Uuid>,
    pub unlink: Vec<UnlinkRow>,
    /// Linked blocks the run left as they were.
    pub unchanged: usize,
    /// The block of the row the pointer is over: outlined on the diagram.
    pub hovered: Option<Uuid>,
}

impl ResyncReview {
    /// The rows checked: the blocks Apply selected unlinks.
    pub fn checked(&self) -> Vec<Uuid> {
        self.unlink
            .iter()
            .filter(|row| row.checked)
            .map(|row| row.block)
            .collect()
    }
}

/// One block that would become unlinked.
#[derive(Debug, Clone, PartialEq)]
pub struct UnlinkRow {
    pub block: Uuid,
    pub name: String,
    /// The sheet path that left the project.
    pub path: String,
    pub checked: bool,
}

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
    /// Each sheet's file, where a click on a part goes.
    pub sheet_paths: HashMap<SheetKey, PathBuf>,
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
    /// The variant picked in the Variant field; `None` reads the project's
    /// active one.
    pub variant: Option<String>,
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

/// One page an export can print (mockup row 171's Pages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExportPage {
    Blocks,
    PowerTree,
    Budget,
}

impl ExportPage {
    pub const ALL: [Self; 3] = [Self::Blocks, Self::PowerTree, Self::Budget];

    /// The token in the `pages` operand.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::PowerTree => "power_tree",
            Self::Budget => "budget",
        }
    }

    pub fn from_token(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|page| page.token() == text.trim())
    }
}

/// The paper a PDF export prints on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportPaper {
    #[default]
    A4,
    A3,
    A2,
    Letter,
    Tabloid,
}

impl ExportPaper {
    pub const ALL: [Self; 5] = [Self::A4, Self::A3, Self::A2, Self::Letter, Self::Tabloid];

    /// The token in the `paper` operand.
    pub const fn token(self) -> &'static str {
        match self {
            Self::A4 => "a4",
            Self::A3 => "a3",
            Self::A2 => "a2",
            Self::Letter => "letter",
            Self::Tabloid => "tabloid",
        }
    }

    pub fn from_token(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|paper| paper.token().eq_ignore_ascii_case(text.trim()))
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
    /// The PDF's pages, in print order; an SVG is the diagram only.
    pub pages: Vec<ExportPage>,
    pub paper: ExportPaper,
    pub portrait: bool,
}

impl ExportDraft {
    /// The popover for `kind`, offering `outputs/<stem>_blocks.<ext>`, the
    /// diagram and power tree pages ticked, on A4 landscape (as drawn).
    pub fn new(kind: ExportKind, stem: &str) -> Self {
        Self {
            kind,
            file: draft_file(stem, kind),
            monochrome: false,
            title_block: true,
            pages: vec![ExportPage::Blocks, ExportPage::PowerTree],
            paper: ExportPaper::A4,
            portrait: false,
        }
    }

    /// Switch the format; a file still named as offered follows it.
    pub fn pick_kind(&mut self, kind: ExportKind, stem: &str) {
        if self.file == draft_file(stem, self.kind) {
            self.file = draft_file(stem, kind);
        }
        self.kind = kind;
    }

    /// Tick or untick `page`, keeping print order.
    pub fn set_page(&mut self, page: ExportPage, on: bool) {
        self.pages.retain(|p| *p != page);
        if on {
            self.pages.push(page);
            self.pages.sort_unstable();
        }
    }
}

fn draft_file(stem: &str, kind: ExportKind) -> String {
    format!("outputs/{stem}_blocks.{}", kind.token())
}
