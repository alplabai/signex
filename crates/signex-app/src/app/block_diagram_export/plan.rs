//! An export planned on the update thread: its settings (colours, title
//! block, pages, paper) turned into the job the worker runs, with every
//! word the pages print taken from the catalog in the user's language.

use std::path::PathBuf;

use signex_i18n::{I18n, MessageId, ids, t};
use signex_output::{
    BlockDiagramFormat, BlockDiagramLook, BlockDiagramPdfOptions, BlockDiagramPdfText, PageSize,
};
use signex_types::schematic::SheetOrientation;

use super::file_label;
use super::job::ExportJob;
use super::power_pages::PowerPageWords;
use crate::app::Signex;
use crate::app::core::block_diagram_panes::{ExportPage, ExportPaper};

/// How an export looks and what it prints.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportSettings {
    pub look: BlockDiagramLook,
    pub title_block: bool,
    pub pages: Vec<ExportPage>,
    pub paper: ExportPaper,
    pub portrait: bool,
}

impl Default for ExportSettings {
    /// The command's defaults: the diagram alone, A4 landscape, a title
    /// block, colours as shown.
    fn default() -> Self {
        Self {
            look: BlockDiagramLook::default(),
            title_block: true,
            pages: vec![ExportPage::Blocks],
            paper: ExportPaper::A4,
            portrait: false,
        }
    }
}

/// The page size `paper` names.
pub fn page_size(paper: ExportPaper) -> PageSize {
    match paper {
        ExportPaper::A4 => PageSize::IsoA4,
        ExportPaper::A3 => PageSize::IsoA3,
        ExportPaper::A2 => PageSize::IsoA2,
        ExportPaper::Letter => PageSize::UsLetter,
        ExportPaper::Tabloid => PageSize::Tabloid,
    }
}

fn words(i18n: &I18n) -> PowerPageWords {
    let w = |id: MessageId| t!(i18n, id).into_string();
    let all = |list: &[MessageId]| list.iter().map(|id| w(*id)).collect();
    PowerPageWords {
        power_heading: w(ids::BLOCK_DIAGRAM_EXPORT_PAGE_POWER),
        budget_heading: w(ids::BLOCK_DIAGRAM_EXPORT_PAGE_BUDGET),
        power_columns: all(&[
            ids::BLOCK_DIAGRAM_EXPORT_COL_ITEM,
            ids::BLOCK_DIAGRAM_EXPORT_COL_ROLE,
            ids::BLOCK_DIAGRAM_EXPORT_COL_VOLTAGE,
            ids::BLOCK_DIAGRAM_EXPORT_COL_CURRENT,
            ids::BLOCK_DIAGRAM_EXPORT_COL_LIMIT,
            ids::BLOCK_DIAGRAM_EXPORT_COL_SHARE,
        ]),
        budget_columns: all(&[
            ids::BLOCK_DIAGRAM_EXPORT_COL_RAIL,
            ids::BLOCK_DIAGRAM_EXPORT_COL_VOLTAGE,
            ids::BLOCK_DIAGRAM_EXPORT_COL_SOURCE,
            ids::BLOCK_DIAGRAM_EXPORT_COL_LIMIT,
            ids::BLOCK_DIAGRAM_EXPORT_COL_LOAD,
            ids::BLOCK_DIAGRAM_EXPORT_COL_HEADROOM,
        ]),
        rail: w(ids::BLOCK_DIAGRAM_EXPORT_ROLE_RAIL),
        source: w(ids::BLOCK_DIAGRAM_EXPORT_ROLE_SOURCE),
        converter: w(ids::BLOCK_DIAGRAM_EXPORT_ROLE_CONVERTER),
        load: w(ids::BLOCK_DIAGRAM_EXPORT_ROLE_LOAD),
        unknown: w(ids::BLOCK_DIAGRAM_EXPORT_NOT_KNOWN),
    }
}

impl Signex {
    /// The job for `diagram` to `path` as `settings` ask; the PDF's words in
    /// the user's language.
    pub(super) fn block_diagram_job(
        &self,
        diagram: PathBuf,
        path: PathBuf,
        format: BlockDiagramFormat,
        settings: &ExportSettings,
    ) -> ExportJob {
        let i18n = &self.ui_state.i18n;
        let label = t!(i18n, ids::BLOCK_DIAGRAM_EXPORT_PAGE_LABEL).into_string();
        let orientation = if settings.portrait {
            SheetOrientation::Portrait
        } else {
            SheetOrientation::Landscape
        };
        ExportJob {
            text: BlockDiagramPdfText {
                title: file_label(&diagram),
                label,
            },
            pdf: BlockDiagramPdfOptions {
                page_size: page_size(settings.paper),
                orientation,
                title_block: settings.title_block,
                monochrome: settings.look.monochrome,
            },
            pages: settings.pages.clone(),
            words: words(i18n),
            diagram,
            path,
            format,
            look: settings.look,
        }
    }
}
