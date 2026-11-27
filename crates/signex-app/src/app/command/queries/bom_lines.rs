//! `bom_lines document=<sheet> [template=<text>] [variant=<name>]
//! [grouping=grouped|ungrouped|flat]` (#1988): the BOM of the sheet's
//! project, line by line, with the columns of a BOM template, computed
//! columns included. The rollup is the export dialog's
//! (`Signex::bom_lines_answer`). See `docs/CLI.md`, "BOM lines".

use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentKind;

impl Signex {
    pub(super) fn bom_lines(&self, args: &CommandArgs) -> Answer {
        let entry = self.document_operand(args)?;
        let path = match &entry.key {
            Some(DocumentKey::Schematic(path)) => path.clone(),
            None if entry.kind == DocumentKind::Schematic => {
                return Err("the sheet is not loaded".into());
            }
            _ => {
                return Err("`bom_lines` reads a schematic sheet; this document is not one".into());
            }
        };
        self.bom_lines_answer(
            &path,
            text_operand(args, "template"),
            text_operand(args, "variant"),
            text_operand(args, "grouping"),
        )
    }
}
