//! The Properties FOOTPRINT card of the selected placed part: resolve its
//! footprint, read it off the UI thread, and show the result in every
//! panel context that waits for it.

use super::super::*;
use crate::library::LibraryMessage;
use crate::panels::{FootprintCardFacts, FootprintCardKey, FootprintCardStatus, FootprintCardView};
use signex_library::LibraryAdapter as _;
use signex_library::primitive::PrimitiveRef;
use std::sync::Arc;

impl Signex {
    /// The card of the single selected placed part, from the cache. A
    /// library footprint not read at the current library version is queued
    /// for [`Self::start_footprint_card_reads`]. `None` when the selection
    /// is not one placed part or the part holds no footprint.
    pub(crate) fn selected_footprint_card(&mut self) -> Option<FootprintCardView> {
        let ctx = &self.document_state.panel_ctx;
        if ctx.selected_kind != Some(signex_types::schematic::SelectedKind::Symbol) {
            return None;
        }
        let uuid = ctx.selected_uuid?;
        let path = self.document_state.active_path.as_ref()?;
        let engine = self.document_state.engines.get(path)?;
        let symbol = engine.document().symbols.iter().find(|s| s.uuid == uuid)?;
        let (name, reference) = self.footprint_card_target(symbol)?;
        let version = self.library.data_version;
        let key = reference.map(|footprint| FootprintCardKey { footprint, version });
        let status = match key {
            Some(key) => self.library.properties_footprints.status(key),
            None => FootprintCardStatus::Unlinked,
        };
        Some(FootprintCardView {
            name,
            key,
            version,
            status,
        })
    }

    /// The name the part shows and the library footprint behind it. A name
    /// typed over the bound footprint is not that footprint, so it gets no
    /// library footprint. `None` when the part has no footprint at all.
    fn footprint_card_target(
        &self,
        symbol: &signex_types::schematic::Symbol,
    ) -> Option<(String, Option<PrimitiveRef>)> {
        let held = symbol.footprint.trim();
        let mut reference = self.bound_footprint_of(symbol);
        let listed = reference.and_then(|r| self.library.footprint_name(&r));
        if listed
            .as_deref()
            .is_some_and(|name| !held.is_empty() && held != name)
        {
            reference = None;
        }
        let name = if held.is_empty() {
            listed?
        } else {
            held.to_string()
        };
        Some((name, reference))
    }

    /// After an update: re-resolve a card resolved at an older library
    /// version (a footprint was saved, a library opened), then start the
    /// queued reads.
    pub(crate) fn refresh_footprint_card(&mut self) -> Task<Message> {
        let version = self.library.data_version;
        let stale = self
            .document_state
            .panel_ctx
            .footprint_card
            .as_ref()
            .is_some_and(|card| card.version != version);
        if stale {
            self.document_state.panel_ctx.footprint_card = self.selected_footprint_card();
        }
        self.start_footprint_card_reads()
    }

    /// Read every queued footprint on a worker.
    fn start_footprint_card_reads(&mut self) -> Task<Message> {
        let keys = self.library.properties_footprints.take_queued();
        let mut reads = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(adapter) = self.library.set.get_shared(key.footprint.library_id) else {
                self.finish_footprint_card_load(key, Err("its library is not open".to_string()));
                continue;
            };
            reads.push(Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        adapter
                            .get_footprint(key.footprint.uuid)
                            .map(|footprint| Arc::new(FootprintCardFacts::of(&footprint)))
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("footprint worker failed: {e}")))
                },
                move |result| {
                    Message::Library(LibraryMessage::PropertiesFootprintCardLoaded { key, result })
                },
            ));
        }
        Task::batch(reads)
    }

    /// A footprint read finished: cache it and show it in every panel
    /// context (main and undocked windows) whose card waits for it.
    pub(crate) fn finish_footprint_card_load(
        &mut self,
        key: FootprintCardKey,
        result: Result<Arc<FootprintCardFacts>, String>,
    ) {
        if let Err(why) = &result {
            crate::diagnostics::log_warning(format!(
                "Could not read footprint {} for the Properties panel: {why}.",
                key.footprint.uuid
            ));
        }
        let Some(status) = self.library.properties_footprints.finish(key, result) else {
            return;
        };
        let contexts = std::iter::once(&mut self.document_state.panel_ctx)
            .chain(self.document_state.window_panel_ctx.values_mut());
        for ctx in contexts {
            if let Some(card) = ctx.footprint_card.as_mut()
                && card.key == Some(key)
            {
                card.status = status.clone();
            }
        }
    }
}
