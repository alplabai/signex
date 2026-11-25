//! Board view preference IO (#2480): the store of the debounced board view
//! writes, one `prefs.json` key merged board by board.

use super::*;
use crate::board_view_prefs::{BoardViews, merge_board_views};

/// Writes the kept board views into `prefs.json`, off the UI thread via
/// `Task`. Each board's entry is merged into what is on disk under the
/// prefs IO lock, so a board this session never showed keeps its view.
pub struct PrefsBoardViewStore {
    path: PathBuf,
}

impl PrefsBoardViewStore {
    /// The store for the user's real `prefs.json`.
    pub fn user_prefs() -> Self {
        Self::at(prefs_path())
    }

    /// The store for an explicit prefs file (tests).
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
}

impl crate::persist::DebouncedStore<BoardViews> for PrefsBoardViewStore {
    fn write(&self, views: &BoardViews) -> Result<(), String> {
        try_update_prefs_json(&self.path, "board views", |prefs| {
            merge_board_views(prefs, views);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_view_prefs::read_board_views_from;
    use crate::persist::DebouncedStore as _;
    use signex_renderer::pcb::PcbLayerView;

    /// What is written is what the next launch reads, and the other keys
    /// of the file stay.
    #[test]
    fn the_board_views_survive_a_write_and_a_read() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{ "ui_font": "Inter" }"#).expect("seeded");
        let mut view = PcbLayerView::default();
        view.toggle_flipped();
        let board = PathBuf::from("/w/a.snxpcb");
        let views: BoardViews = [(board.clone(), view.clone())].into_iter().collect();
        PrefsBoardViewStore::at(path.clone())
            .write(&views)
            .expect("written");
        let json = read_prefs_json(&path);
        assert_eq!(
            read_board_views_from(json.as_ref()).get(&board),
            Some(&view)
        );
        assert_eq!(json.expect("a file")["ui_font"], "Inter");
    }
}
