//! A loaded or replaced schematic engine, drawn into the canvas slots that
//! show it.

use crate::app::Signex;

impl Signex {
    pub(crate) fn active_render_snapshot(
        &self,
    ) -> Option<&crate::schematic_runtime::SchematicRenderSnapshot> {
        self.interaction_state.active_canvas().active_snapshot()
    }

    pub(crate) fn sync_canvas_from_visible_schematic(
        &mut self,
        invalidation: crate::schematic_runtime::RenderInvalidation,
    ) {
        // Active engine drives the render cache — if it's gone the
        // cache is cleared. There is no parked-schematic fallback with
        // HashMap storage: an open schematic tab always has its engine
        // resident in `document_state.engines`.
        if let Some(path) = self
            .document_state
            .active_path
            .clone()
            .filter(|p| self.document_state.engines.contains_key(p))
        {
            self.refresh_slots_from_engine(&path, true, invalidation, false);
            return;
        }

        self.interaction_state
            .active_canvas_mut()
            .set_render_cache(None);
    }

    /// Redraw every canvas slot showing the engine at `path` from it: the
    /// undocked windows on that sheet (#171) and, with `include_main`, the
    /// main window's slot. `clear_selection` also drops each slot's
    /// selection, for an engine that was replaced wholesale: its UUIDs may
    /// not exist in the new document (#102).
    pub(crate) fn refresh_slots_from_engine(
        &mut self,
        path: &std::path::Path,
        include_main: bool,
        invalidation: crate::schematic_runtime::RenderInvalidation,
        clear_selection: bool,
    ) {
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        let sheet = engine.document();
        let (base, revision) = (engine.base_revision(), engine.revision());
        let windows = &self.ui_state.windows;
        let main = include_main.then_some(&mut self.interaction_state.canvas);
        // An unfocused tile pane on that sheet draws it too (#2249).
        let pane = self.interaction_state.tile_panes.schematic.get_mut(path);
        let slots = self
            .interaction_state
            .canvases
            .iter_mut()
            .filter(|(id, _)| {
                matches!(
                    windows.get(id),
                    Some(crate::app::state::WindowKind::UndockedTab { path: shown, .. })
                        if shown == path
                )
            })
            .map(|(_, slot)| slot)
            .chain(main)
            .chain(pane);
        for slot in slots {
            if let Some(cache) = slot.render_cache.as_mut() {
                cache.update_from_sheet(sheet, invalidation, base, revision);
            } else {
                slot.set_render_cache(Some(
                    crate::schematic_runtime::SchematicRenderCache::from_sheet_at(sheet, revision),
                ));
            }
            if clear_selection {
                slot.selected.clear();
            }
            // The retained layers rebuild on a clear, a zoom, a non-whole-pixel
            // pan or a new scene; a whole-pixel pan only moves them.
            slot.clear_content_cache();
        }
    }

    /// Refresh only the main window's canvas from the engine at `path` after
    /// an edit made through an undocked window's swapped-in slot. The other
    /// undocked slots were already refreshed by that edit's own sync.
    pub(crate) fn refresh_main_canvas_from_engine(&mut self, path: &std::path::Path) {
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        let slot = &mut self.interaction_state.canvas;
        let sheet = engine.document();
        let revision = engine.revision();
        match slot.render_cache.as_mut() {
            Some(cache) => cache.update_from_sheet(
                sheet,
                crate::schematic_runtime::RenderInvalidation::FULL,
                engine.base_revision(),
                revision,
            ),
            None => slot.set_render_cache(Some(
                crate::schematic_runtime::SchematicRenderCache::from_sheet_at(sheet, revision),
            )),
        }
        slot.clear_content_cache();
    }
}
