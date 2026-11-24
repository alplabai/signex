//! Components panel rows (#2535): the "Previews" switch, the row symbol
//! thumbnails read off-thread, and the parts placed from the panel.

use super::*;

use crate::library::state::{ComponentsPanelRowKey, ComponentsRowStyle};

/// The `prefs.json` key the "Previews" switch is written under.
const ROW_STYLE_PREF: &str = "components_panel_previews";

impl Signex {
    /// The "Previews" switch was flipped: draw the rows in `style` and
    /// remember it off the UI thread.
    pub(super) fn handle_components_panel_row_style_picked(
        &mut self,
        style: ComponentsRowStyle,
    ) -> Task<Message> {
        let panel = &mut self.library.components_panel;
        if panel.row_style == style {
            return Task::none();
        }
        panel.row_style = style;
        crate::fonts::background_pref_write(ROW_STYLE_PREF, move || {
            crate::fonts::write_components_row_style_pref(style)
        })
    }

    /// Ask for the row thumbnails the panel draws and has not read yet, and
    /// the part symbols of the Library panel's and the shown Library
    /// Browser's thumbnail rows (mockup rows 118 and 119), which share the
    /// cache. Runs after every library message; a no-op when nothing is
    /// missing.
    pub(super) fn request_components_thumbnails(&mut self) -> Task<Message> {
        let mounted = self.library.open_libraries.len();
        let mut wanted = crate::panels::components_panel::wanted_thumbnails(&self.library);
        wanted.extend(self.library.browser_wanted_thumbnails());
        wanted.extend(crate::library::panel::wanted_thumbnails(&self.library));
        let thumbs = &mut self.library.components_panel.thumbs;
        thumbs.note_mounted(mounted);
        let missing = thumbs.take_missing(wanted);
        if missing.is_empty() {
            return Task::none();
        }
        let libraries = self.library.shared_adapters();
        let asked = missing.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::panels::components_panel::thumbs::load_thumbnails(&missing, &libraries)
                })
                .await
                // A failed worker marks every asked symbol unreadable, so
                // none stays "asked" for ever.
                .unwrap_or_else(|_| asked.into_iter().map(|r| (r, None)).collect())
            },
            |read| Message::Library(LibraryMessage::ComponentsPanelThumbnailsLoaded(read)),
        )
    }

    /// `key` was placed from the panel: it heads Recently placed.
    pub(super) fn record_components_placement(&mut self, key: ComponentsPanelRowKey) {
        self.library.components_panel.recent.record(key);
    }
}
