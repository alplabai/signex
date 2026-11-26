//! The settings of one notebook PDF export (mockup row 202): the same in
//! the Export Design Notebook PDF dialog, the `export_notebook_pdf`
//! command and the Design Notebook PDF output of an Output Job.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use signex_types::project::BASE_VARIANT;

use crate::annotation::AnnotationKind;

/// Which pages go into the PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSelection {
    /// Every page.
    #[default]
    All,
    /// The page the notebook tab shows, 1-based.
    Current { page: usize },
    /// Pages `first` to `last`, both 1-based and included.
    Range { first: usize, last: usize },
}

impl PageSelection {
    /// `all`, `current`, a range `3-7` (an en dash reads too) or one page
    /// `4`; `current` takes `current_page`. `None` for anything else, a
    /// page 0 or a range that runs backwards.
    pub fn parse(text: &str, current_page: usize) -> Option<Self> {
        let text = text.trim();
        match text {
            "all" => return Some(Self::All),
            "current" => {
                return Some(Self::Current {
                    page: current_page.max(1),
                });
            }
            _ => {}
        }
        let (first, last) = match text.split_once(['-', '\u{2013}']) {
            Some((first, last)) => (first.trim(), last.trim()),
            None => (text, text),
        };
        let first: usize = first.parse().ok()?;
        let last: usize = last.parse().ok()?;
        (first >= 1 && last >= first).then_some(Self::Range { first, last })
    }

    /// The pages chosen out of `page_count`, 1-based, first and last
    /// included; `None` when no page of the notebook is chosen.
    pub fn resolve(self, page_count: usize) -> Option<(usize, usize)> {
        let (first, last) = match self {
            Self::All => (1, page_count),
            Self::Current { page } => (page, page),
            Self::Range { first, last } => (first, last.min(page_count)),
        };
        (first >= 1 && first <= last && last <= page_count).then_some((first, last))
    }
}

/// The paper the pages are laid out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    /// What the notebook's report layout sets (A4 portrait unless it says
    /// otherwise).
    #[default]
    FromDocument,
    A4,
    /// US Letter: the pages flow again on the other size.
    Letter,
}

impl Paper {
    pub const ALL: [Paper; 3] = [Paper::FromDocument, Paper::A4, Paper::Letter];

    /// The operand token: `document`, `a4` or `letter`.
    pub fn token(self) -> &'static str {
        match self {
            Paper::FromDocument => "document",
            Paper::A4 => "a4",
            Paper::Letter => "letter",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|paper| paper.token() == token.trim())
    }

    /// The Typst paper name the export sets; `None` keeps the document's.
    pub(crate) fn typst_name(self) -> Option<&'static str> {
        match self {
            Paper::FromDocument => None,
            Paper::A4 => Some("a4"),
            Paper::Letter => Some("us-letter"),
        }
    }
}

/// What a stale reference does to the export. Stale means the part, pin or
/// net was renamed or deleted after the notebook named it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleRule {
    /// Export, the stale reference printed as plain text with a mark in the
    /// margin.
    #[default]
    Warn,
    /// Refuse the export.
    Fail,
}

impl StaleRule {
    pub const ALL: [StaleRule; 2] = [StaleRule::Warn, StaleRule::Fail];

    /// The operand token: `warn` or `fail`.
    pub fn token(self) -> &'static str {
        match self {
            StaleRule::Warn => "warn",
            StaleRule::Fail => "fail",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|rule| rule.token() == token.trim())
    }
}

/// Every setting of a notebook PDF export. The default is the dialog as it
/// opens (mockup row 202): every page, the document's paper, variant Base,
/// title block, contents and bookmarks in, Python source out, every
/// annotation kind, and a stale reference warns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportOptions {
    pub pages: PageSelection,
    pub paper: Paper,
    /// The variant the references resolve in.
    pub variant: String,
    /// The title block and revision at the top of the first page.
    pub title_block: bool,
    /// A table of contents after the title block.
    pub contents: bool,
    /// PDF bookmarks for the sections, figures and tables.
    pub bookmarks: bool,
    /// The source of Python cells; their results and plots always print.
    pub python_source: bool,
    /// The annotation kinds printed; a kind left out prints nothing.
    pub annotations: BTreeSet<AnnotationKind>,
    pub stale: StaleRule,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            pages: PageSelection::All,
            paper: Paper::FromDocument,
            variant: BASE_VARIANT.to_string(),
            title_block: true,
            contents: true,
            bookmarks: true,
            python_source: false,
            annotations: AnnotationKind::ALL.into_iter().collect(),
            stale: StaleRule::Warn,
        }
    }
}

impl ExportOptions {
    /// The `sys.inputs` the export compiles with, read by the bundled
    /// package (`docs/NOTEBOOK_PDF.md`, "What the export passes to the
    /// page"). `stale` are the names of the stale references, which print
    /// with a margin mark.
    pub fn typst_inputs(&self, stale: &[String]) -> Vec<(String, String)> {
        let flag = |on: bool| if on { "true" } else { "false" }.to_string();
        let hidden: Vec<&str> = AnnotationKind::ALL
            .into_iter()
            .filter(|kind| !self.annotations.contains(kind))
            .map(AnnotationKind::token)
            .collect();
        let mut inputs = vec![
            ("signex-export".to_string(), flag(true)),
            ("signex-title-block".to_string(), flag(self.title_block)),
            ("signex-contents".to_string(), flag(self.contents)),
            ("signex-bookmarks".to_string(), flag(self.bookmarks)),
            ("signex-python-source".to_string(), flag(self.python_source)),
            ("signex-hidden-kinds".to_string(), hidden.join(",")),
            ("signex-variant".to_string(), self.variant.clone()),
            ("signex-stale".to_string(), stale.join("\n")),
        ];
        if let Some(paper) = self.paper.typst_name() {
            inputs.push(("signex-paper".to_string(), paper.to_string()));
        }
        inputs.sort();
        inputs
    }
}

/// Read from its token; it is written as its token (`annotation.rs`).
impl<'de> Deserialize<'de> for AnnotationKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let token = String::deserialize(deserializer)?;
        AnnotationKind::parse(&token)
            .ok_or_else(|| serde::de::Error::custom(format!("`{token}` is not an annotation kind")))
    }
}
