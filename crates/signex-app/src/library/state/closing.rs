//! Closing a library, and the close that waits for its writes (#893,
//! #1728). Design: `docs/architecture/library-writes.md`, "Closing a
//! library".

use super::*;

impl LibraryState {
    /// Libraries whose close was deferred and whose writes are all done.
    pub fn closes_ready(&self) -> Vec<PathBuf> {
        self.closing
            .iter()
            .filter(|root| self.writes.pending(root) == 0)
            .cloned()
            .collect()
    }

    /// The name a prompt or a Messages line gives the library at `root`:
    /// its display name, else its file name.
    pub fn library_label(&self, root: &Path) -> String {
        self.library_at(root)
            .map(|lib| lib.display_name.clone())
            .unwrap_or_else(|| {
                root.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.display().to_string())
            })
    }

    /// Whether a close of the library at `root` waits for its writes.
    pub fn is_closing(&self, root: &Path) -> bool {
        self.closing.contains(root)
    }

    /// Drop a deferred close of the library at `root`: the user opened it
    /// again, or the close met unsaved editors when it came due. `true`
    /// when a close was pending.
    pub fn cancel_close(&mut self, root: &Path) -> bool {
        self.closing.remove(root)
    }

    /// Drop a deferred close and say why in Messages, when one was pending.
    /// Every re-mount (both mount entry points) and Cancel in the close
    /// prompt come through here.
    pub fn cancel_pending_close(&mut self, root: &Path, why: &str) {
        if self.cancel_close(root) {
            crate::diagnostics::log_info(format!(
                "Pending Close Library of {} cancelled: {why}.",
                self.library_label(root)
            ));
        }
    }

    /// Drop the library backing `root` — unmounts it from `set` and drops
    /// every Component Preview editor pointing at it. The dispatcher asks
    /// about dirty editors first (`dirty_editors_for_library`), both when
    /// Close is requested and again when a deferred close comes due.
    pub fn close_library(&mut self, root: &Path) {
        // A pricing walk of this library must not write after it is closed.
        self.cancel_pricing_refresh_of(&[root.to_path_buf()]);
        // Never unmount under a running write (ADR-0003 D2): the close
        // waits, without blocking, and `closes_ready` finishes it once the
        // library's writes are done.
        if self.writes.pending(root) > 0 {
            self.closing.insert(root.to_path_buf());
            return;
        }
        self.closing.remove(root);
        // Cancel any mount still being prepared off-thread for this path
        // (#99 part 2c). `pending_mounts` doubles as the cancellation
        // tombstone: `take_mount_intent` returns `None` for a path that
        // is gone, so an in-flight preparation that lands after this
        // point drops its payload instead of re-mounting a library the
        // user just closed. Unconditional — the entry only exists while
        // a preparation is in flight, and closing a library that was
        // mounted synchronously simply has nothing to remove.
        self.pending_mounts.remove(root);
        self.refresh_generations.remove(root);
        self.refresh_in_flight
            .retain(|_, in_flight| in_flight.as_path() != root);
        self.pending_follow_ups.remove(root);
        if let Some(idx) = self.open_libraries.iter().position(|lib| lib.root == root) {
            let entry = self.open_libraries.remove(idx);
            self.touch_data();
            // What its last commits left out is reported before it goes.
            if let Some(adapter) = self.set.get(entry.library_id) {
                crate::library::held_back::report_held_back(&entry.display_name, adapter);
            }
            self.set.unmount(entry.library_id);
            // A preview readout planned under this mount must not apply to a
            // later mount of the same library id.
            self.preview_generations.remove(&entry.library_id);
            self.editors.retain(|key, _| key.library_path != root);
            // Names, drift rows and Update must not use an unmounted library.
            self.drift.check_pending = true;
        }
    }
}
