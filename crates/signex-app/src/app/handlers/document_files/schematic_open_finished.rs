//! Install a File > Open schematic read (#478, #1756).

use std::path::PathBuf;

use iced::Task;

use super::super::super::*;

impl Signex {
    /// A File > Open schematic read ended: install the tab, or report why not.
    pub(crate) fn schematic_open_finished(
        &mut self,
        path: PathBuf,
        title: String,
        result: Result<Box<signex_types::schematic::SchematicSheet>, String>,
        disk: Option<crate::persist::DiskRead>,
    ) -> Task<Message> {
        // #478 review — clear the in-flight guard before acting
        // on the result so a duplicate open fired while this one
        // was in flight is free to spawn again from here on.
        self.document_state.pending_opens.remove_untracked(&path);
        if self.is_open_result_superseded(&path, &result) {
            self.attach_parked_tab_for_superseded_open(&path, title);
            self.activate_tab_for_path(&path);
            return self.finish_update();
        }
        match result {
            Ok(_) if self.has_tab_for(&path) => Self::log_duplicate_open(&path),
            Ok(sheet) => {
                if let Some(disk) = &disk
                    && self.focus_alias_if_open(&path, disk)
                {
                    return self.finish_update();
                }
                // `update` starts the recovery check of every tab opened.
                self.open_schematic_tab(path.clone(), title, *sheet);
                if let Some(disk) = disk {
                    self.record_disk_read(&path, disk);
                }
            }
            Err(message) => self.report_open_failure(message),
        }
        self.finish_update()
    }
}
