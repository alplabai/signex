//! `list_image_assets document=<sheet>` (#1116, mockup row 204): every file
//! under the project's `assets/images/` and every file an image names, with
//! its facts, how many images use it, on which sheets, and whether it is
//! missing. The scan runs off the UI thread (`app::images::assets`) and the
//! query is answered when it lands. See docs/CLI.md, "Images".

use std::path::PathBuf;

use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document_ids::DocumentKind;

impl Signex {
    /// The sheet `list_image_assets` names: its project's assets are listed,
    /// or its folder's for a sheet in no project.
    pub(super) fn image_assets_sheet(&self, args: &CommandArgs) -> Result<PathBuf, String> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err("`list_image_assets` needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(target)?;
        if entry.kind != DocumentKind::Schematic {
            return Err(format!(
                "`list_image_assets` lists the images of a sheet's project, and {} is a {}",
                entry.name,
                entry.kind.as_str()
            ));
        }
        entry
            .path
            .ok_or_else(|| format!("{} has no file", entry.name))
    }
}
