//! Whether placed parts hold what their row offers, asked for many parts of
//! one `lib_id` in a row (#1939): the stamps are built once, each library
//! opens once and each row's footprint name is read once.

use std::collections::HashMap;

use uuid::Uuid;

use super::{
    ApplyStamp, MountedLibrary, Opened, PartIdentity, instance_stamps, open_library,
    part_sync_against, row_identity,
};

pub(crate) struct PartSyncCheck {
    libs: Vec<MountedLibrary>,
    stamps: Vec<ApplyStamp>,
    opened: HashMap<Uuid, Opened>,
    wanted: HashMap<Uuid, PartIdentity>,
}

impl PartSyncCheck {
    /// A check over the instances of `lib_id` on `doc`.
    pub(crate) fn new(
        doc: &signex_types::schematic::SchematicSheet,
        lib_id: &str,
        libs: Vec<MountedLibrary>,
    ) -> Self {
        Self {
            libs,
            stamps: instance_stamps(doc, lib_id, &HashMap::new()),
            opened: HashMap::new(),
            wanted: HashMap::new(),
        }
    }

    /// Whether `symbol` differs from `row` in a field "Update from library"
    /// would rewrite. A library that cannot be opened counts as pending: the
    /// caller then keeps the old stamp rather than hide a change it could
    /// not check.
    pub(crate) fn pending(&mut self, symbol: Uuid, row: &signex_library::ComponentRow) -> bool {
        let Some(w) = self.stamps.iter().find(|w| w.symbol == symbol) else {
            return false;
        };
        let opened = self
            .opened
            .entry(w.library_id)
            .or_insert_with(|| open_library(&self.libs, w.library_id));
        let opened = match opened {
            Ok(opened) => opened,
            Err(why) => {
                tracing::debug!(target: "signex::library", %why, "part sync check: library unreadable; keeping the stamp");
                return true;
            }
        };
        let libs = &self.libs;
        let want = self
            .wanted
            .entry(row.row_id)
            .or_insert_with(|| row_identity(opened, row, libs));
        part_sync_against(w, want, row).is_some()
    }
}
