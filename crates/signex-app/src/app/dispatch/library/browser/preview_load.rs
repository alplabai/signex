//! The Library Browser preview pane's symbol and footprint (#2260), read
//! off-thread with the Components panel's loader and kept on the browser's
//! state, and the pane's ⋯ menu.

use super::*;
use crate::library::state::ComponentsPanelRowKey;

impl Signex {
    /// Start a read for each Library Browser whose selected row has no
    /// current preview: another row was selected, or the row was saved since
    /// it was read. Runs after every library message, so a selection made by
    /// any path (a click, a new row, a reveal) is drawn. A read that failed
    /// is not retried until the row changes.
    pub(in crate::app::dispatch::library) fn refresh_browser_previews(&mut self) -> Task<Message> {
        let mut wanted = Vec::new();
        for (path, browser) in &self.library.library_browsers {
            let Some(row_id) = browser.selected_row else {
                continue;
            };
            let Some(table) = browser
                .active_table
                .clone()
                .or_else(|| self.library.shown_browser_table(path))
            else {
                continue;
            };
            wanted.push(ComponentsPanelRowKey {
                library_path: path.clone(),
                table,
                row_id,
            });
        }
        for browser in self.library.library_browsers.values_mut() {
            if browser.selected_row.is_none() {
                browser.more_menu_open = false;
                if browser.preview.key.is_some() {
                    browser.preview = Default::default();
                }
            }
        }
        let mut tasks = Vec::new();
        for key in wanted {
            let stamp = self.components_row_stamp(&key);
            let Some(browser) = self.library.library_browsers.get_mut(&key.library_path) else {
                continue;
            };
            let same_row = browser.preview.key.as_ref() == Some(&key);
            if same_row && (browser.preview.in_flight() || browser.preview.stamp == stamp) {
                continue;
            }
            if !same_row {
                browser.more_menu_open = false;
            }
            tasks.push(self.start_browser_preview_load(key, stamp));
        }
        Task::batch(tasks)
    }

    /// Read `key`'s symbol and footprint on a worker; the result lands as
    /// `BrowserPreviewLoaded`, keyed by the read's generation.
    fn start_browser_preview_load(
        &mut self,
        key: ComponentsPanelRowKey,
        stamp: Option<crate::panels::components_panel::details::RowStamp>,
    ) -> Task<Message> {
        let libraries = self.library.shared_adapters();
        let own = self.library.shared_adapter_at(&key.library_path);
        let Some(browser) = self.library.library_browsers.get_mut(&key.library_path) else {
            return Task::none();
        };
        let generation = browser.preview.begin(key.clone(), stamp);
        let Some(own) = own else {
            let why = format!(
                "The library {} is not open, so the preview cannot be read.",
                key.library_path.display()
            );
            browser.preview.finish(&key, generation, Err(why));
            return Task::none();
        };
        let read_key = key.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::panels::components_panel::details::load::load_details(
                        &read_key,
                        own.as_ref(),
                        &libraries,
                    )
                })
                .await
                .unwrap_or_else(|e| Err(format!("preview worker failed: {e}")))
            },
            move |result| {
                Message::Library(LibraryMessage::BrowserPreviewLoaded {
                    key: key.clone(),
                    generation,
                    result: result.map(Box::new),
                })
            },
        )
    }

    /// A preview read finished: keep it when it is still the newest read
    /// of the selected row.
    pub(in crate::app::dispatch::library) fn handle_browser_preview_loaded(
        &mut self,
        key: ComponentsPanelRowKey,
        generation: u64,
        result: Result<Box<crate::panels::components_panel::details::ComponentDetails>, String>,
    ) -> Task<Message> {
        if let Some(browser) = self.library.library_browsers.get_mut(&key.library_path) {
            browser.preview.finish(&key, generation, result);
        }
        Task::none()
    }

    /// The preview's ⋯ button: open or close its menu.
    pub(in crate::app::dispatch::library) fn handle_browser_more_menu_toggled(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(browser) = self.library.library_browsers.get_mut(&library_path) {
            browser.more_menu_open = !browser.more_menu_open;
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use crate::app::Signex;
    use crate::library::state::{ComponentsPanelRowKey, LibraryBrowserState};

    fn app_with_browser() -> (Signex, std::path::PathBuf) {
        let (mut app, _task) = Signex::new();
        let root = std::path::PathBuf::from("/tmp/preview-load.snxlib");
        app.library
            .library_browsers
            .insert(root.clone(), LibraryBrowserState::new(root.clone()));
        (app, root)
    }

    /// The ⋯ button opens and closes the menu.
    #[test]
    fn the_more_button_toggles_its_menu() {
        let (mut app, root) = app_with_browser();
        let _ = app.handle_browser_more_menu_toggled(root.clone());
        assert!(app.library.library_browsers[&root].more_menu_open);
        let _ = app.handle_browser_more_menu_toggled(root.clone());
        assert!(!app.library.library_browsers[&root].more_menu_open);
    }

    /// Selecting a row of an unmounted library records why its preview
    /// cannot be read instead of reading on the UI thread; dropping the
    /// selection drops the preview and closes the menu.
    #[test]
    fn a_selection_starts_a_preview_read_and_a_cleared_one_drops_it() {
        let (mut app, root) = app_with_browser();
        let row_id = signex_library::RowId::new();
        {
            let browser = app.library.library_browsers.get_mut(&root).unwrap();
            browser.active_table = Some("R".into());
            browser.selected_row = Some(row_id);
        }
        let _ = app.refresh_browser_previews();
        let preview = &app.library.library_browsers[&root].preview;
        assert_eq!(
            preview.key,
            Some(ComponentsPanelRowKey {
                library_path: root.clone(),
                table: "R".into(),
                row_id,
            })
        );
        assert!(preview.error.is_some(), "the library is not mounted");
        // A failed read is not retried while the row is unchanged.
        let generation = preview.generation;
        let _ = app.refresh_browser_previews();
        assert_eq!(
            app.library.library_browsers[&root].preview.generation,
            generation
        );

        {
            let browser = app.library.library_browsers.get_mut(&root).unwrap();
            browser.selected_row = None;
            browser.more_menu_open = true;
        }
        let _ = app.refresh_browser_previews();
        let browser = &app.library.library_browsers[&root];
        assert!(browser.preview.key.is_none());
        assert!(!browser.more_menu_open);
    }
}
