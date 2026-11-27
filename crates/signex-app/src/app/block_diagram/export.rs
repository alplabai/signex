//! The tab's export popover and Re-sync (mockup row 171). SVG and PDF… on
//! the toolbar open the popover for that format; its Export runs
//! `export_block_diagram` through the translator with the popover's file,
//! format and colours, the same handler the command line runs. Re-sync
//! runs `regenerate_block_diagram apply=true` on this diagram
//! (`BlockDiagramSyncMsg::Requested`), which merges into the open diagram
//! as one undo step (`block_diagram_sync`).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{ids, t};

use super::ExportEvent;
use super::state::{ExportDraft, ExportKind};
use crate::app::command::{CommandOutcome, OperandValue, translator};
use crate::app::{BlockDiagramSyncMsg, Message, Signex};

/// The file stem of `path`, which the popover's file is named after.
fn stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

/// The popover's file as a path: relative to the diagram's folder.
fn export_path(diagram: &Path, file: &str) -> PathBuf {
    let given = Path::new(file.trim());
    if given.is_absolute() {
        return given.to_path_buf();
    }
    diagram
        .parent()
        .map_or_else(|| given.to_path_buf(), |dir| dir.join(given))
}

impl Signex {
    /// Re-sync from the toolbar: what the palette's regenerate sends, with
    /// the changes applied.
    pub(super) fn resync_block_diagram(&mut self) -> Task<Message> {
        self.dispatch_block_diagram_sync_message(BlockDiagramSyncMsg::Requested {
            regenerate: true,
            apply: true,
        })
    }

    pub(super) fn block_diagram_export_event(
        &mut self,
        path: &Path,
        event: ExportEvent,
    ) -> Task<Message> {
        let stem = stem(path);
        let Some(tab) = self.open_diagram_view_mut(path) else {
            return Task::none();
        };
        match event {
            ExportEvent::Opened(kind) => tab.export = Some(ExportDraft::new(kind, &stem)),
            ExportEvent::Cancelled => tab.export = None,
            ExportEvent::ExportPressed => return self.export_from_popover(path),
            edit => {
                if let Some(draft) = tab.export.as_mut() {
                    edit_draft(draft, edit, &stem);
                }
            }
        }
        Task::none()
    }

    /// Export with what the popover holds. It closes once the export
    /// starts; a refusal keeps it open and says why in Messages.
    fn export_from_popover(&mut self, path: &Path) -> Task<Message> {
        let Some(draft) = self
            .open_diagram_view(path)
            .and_then(|tab| tab.export.clone())
        else {
            return Task::none();
        };
        let target = export_path(path, &draft.file);
        let variant = self
            .open_diagram_view(path)
            .and_then(|tab| tab.power.variant.clone());
        let mut operands = vec![
            (
                "path",
                OperandValue::Path(target.to_string_lossy().into_owned()),
            ),
            ("kind", OperandValue::Text(draft.kind.token().to_string())),
            ("monochrome", OperandValue::Bool(draft.monochrome)),
            ("title_block", OperandValue::Bool(draft.title_block)),
        ];
        // An SVG is the diagram alone; a PDF prints the pages ticked.
        if draft.kind == ExportKind::Pdf {
            let pages: Vec<&str> = draft.pages.iter().map(|page| page.token()).collect();
            let orientation = if draft.portrait {
                "portrait"
            } else {
                "landscape"
            };
            operands.extend([
                ("pages", OperandValue::Text(pages.join(","))),
                ("paper", OperandValue::Text(draft.paper.token().to_string())),
                ("orientation", OperandValue::Text(orientation.to_string())),
            ]);
            if let Some(variant) = variant {
                operands.push(("variant", OperandValue::Text(variant)));
            }
        }
        let (outcome, task) = translator::run(self, "export_block_diagram", path, operands);
        if let CommandOutcome::Disabled { reason } = outcome {
            let i18n = &self.ui_state.i18n;
            let line = t!(
                i18n,
                ids::BLOCK_DIAGRAM_EXPORT_FAILED,
                "document" => super::file_name(path),
                "reason" => reason.render(i18n)
            );
            crate::diagnostics::log_warning(line.into_string());
            return task;
        }
        if let Some(tab) = self.open_diagram_view_mut(path) {
            tab.export = None;
        }
        task
    }
}

fn edit_draft(draft: &mut ExportDraft, event: ExportEvent, stem: &str) {
    match event {
        ExportEvent::KindPicked(kind) => draft.pick_kind(kind, stem),
        ExportEvent::FileEdited(file) => draft.file = file,
        ExportEvent::MonochromePicked(on) => draft.monochrome = on,
        ExportEvent::TitleBlockToggled(on) => draft.title_block = on,
        ExportEvent::PageToggled(page, on) => draft.set_page(page, on),
        ExportEvent::PaperPicked(paper) => draft.paper = paper,
        ExportEvent::PortraitPicked(on) => draft.portrait = on,
        ExportEvent::Opened(_) | ExportEvent::Cancelled | ExportEvent::ExportPressed => {}
    }
}

/// The formats the popover offers, in order.
pub(super) const KINDS: [ExportKind; 2] = [ExportKind::Svg, ExportKind::Pdf];
