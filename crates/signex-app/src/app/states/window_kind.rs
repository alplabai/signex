/// Role of a non-main window opened by Signex. Phase 2 adds detached
/// modals; Phase 3 adds `UndockedTab(tab_index)` so a schematic sheet
/// can live in its own OS window.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WindowKind {
    DetachedModal(super::ModalId),
    /// Undocked document tab. Stores the tab's file path (unique per
    /// open tab in Signex) so the mapping survives tab reordering or
    /// unrelated tabs closing. The `title` copy is used as the OS
    /// window title without re-reading tabs.
    UndockedTab {
        path: std::path::PathBuf,
        title: String,
    },
    /// Detached dock panel. Opened automatically when the user drags a
    /// floating panel past the main window edge. Closing the OS window
    /// reattaches the panel to its last dock region.
    DetachedPanel(crate::panels::PanelKind),
}
