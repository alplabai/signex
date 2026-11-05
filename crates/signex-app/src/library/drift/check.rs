//! Whether placed parts hold what their row offers, asked for many parts of
//! one `lib_id` in a row (#1939): the stamps are built once, each library
//! opens once and each row's footprint name is read once.

use std::collections::HashMap;

use uuid::Uuid;

use super::{
    ApplyStamp, MountedLibrary, Opened, eval_against, instance_stamps, open_library, row_offer,
};
use crate::library::part_sync::{PartEval, RowOffer};

pub(crate) struct PartSyncCheck {
    libs: Vec<MountedLibrary>,
    stamps: Vec<ApplyStamp>,
    opened: HashMap<Uuid, Opened>,
    wanted: HashMap<(Uuid, Uuid), RowOffer>,
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

    /// How `symbol` stands against `row`. `None` when it is not one of the
    /// parts this check was built for, or its library cannot be opened: the
    /// caller then keeps the old stamp rather than hide a change it could not
    /// check.
    pub(crate) fn eval(
        &mut self,
        symbol: Uuid,
        row: &signex_library::ComponentRow,
    ) -> Option<PartEval> {
        let w = self.stamps.iter().find(|w| w.symbol == symbol)?;
        let opened = self
            .opened
            .entry(w.library_id)
            .or_insert_with(|| open_library(&self.libs, w.library_id));
        let opened = match opened {
            Ok(opened) => opened,
            Err(why) => {
                tracing::debug!(target: "signex::library", %why, "part sync check: library unreadable; keeping the stamp");
                return None;
            }
        };
        let libs = &self.libs;
        let offer = self
            .wanted
            .entry((w.library_id, row.row_id))
            .or_insert_with(|| row_offer(opened, row, libs));
        Some(eval_against(w, offer, row))
    }
}
