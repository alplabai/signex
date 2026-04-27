use std::path::PathBuf;

use signex_library::{Footprint, Symbol};
use signex_types::pcb::PcbBoard;

// v0.9-refactor-2: DBLib model. Identity payload for a Component
// Preview tab — `(library_path, table, row_id)` triple from
// `EditorAddress`. The inline tab and any future undock case route
// through the same triple.
#[derive(Debug, Clone)]
pub struct ComponentEditorTab {
    pub library_path: PathBuf,
    pub table: String,
    pub row_id: signex_library::RowId,
}

// WS-I: tab-not-window
// Per-tab role marker. Schematic / Pcb retain the path on `TabInfo`
// for the existing `engines` HashMap and dirty-paths machinery;
// `ComponentEditor` carries its own `(library_path, component_id)`
// payload that the dispatcher uses to look the editor state up out
// of `LibraryState.editors`. The synthetic `TabInfo.path` for
// ComponentEditor tabs is `<library_path>/<component_id>.snxprt` so
// undock / "is this tab already undocked?" / per-tab visibility
// continue to use a single PathBuf identity.
#[derive(Debug, Clone)]
pub enum TabKind {
    Schematic,
    Pcb,
    ComponentEditor(ComponentEditorTab),
    // WS-7 (refactor-2): standalone primitive editor tabs
    /// `.snxsym` opened as a main-window document tab. Editor state
    /// lives in [`crate::app::DocumentState::symbol_editors`] keyed by
    /// the same path that lives on `TabInfo.path`.
    SymbolEditor(PathBuf),
    /// `.snxfpt` opened as a main-window document tab. Editor state
    /// lives in [`crate::app::DocumentState::footprint_editors`] keyed
    /// by the same path that lives on `TabInfo.path`.
    FootprintEditor(PathBuf),
}

impl TabKind {
    #[allow(dead_code)]
    pub fn is_component_editor(&self) -> bool {
        matches!(self, TabKind::ComponentEditor(_))
    }

    pub fn as_component_editor(&self) -> Option<&ComponentEditorTab> {
        match self {
            TabKind::ComponentEditor(c) => Some(c),
            _ => None,
        }
    }

    /// `Some(path)` if this tab is a standalone Symbol editor.
    pub fn as_symbol_editor(&self) -> Option<&PathBuf> {
        match self {
            TabKind::SymbolEditor(p) => Some(p),
            _ => None,
        }
    }

    /// `Some(path)` if this tab is a standalone Footprint editor.
    pub fn as_footprint_editor(&self) -> Option<&PathBuf> {
        match self {
            TabKind::FootprintEditor(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawMode {
    #[default]
    Ortho90,
    Angle45,
    FreeAngle,
}

impl DrawMode {
    pub fn next(self) -> Self {
        match self {
            DrawMode::Ortho90 => DrawMode::Angle45,
            DrawMode::Angle45 => DrawMode::FreeAngle,
            DrawMode::FreeAngle => DrawMode::Ortho90,
        }
    }
}

#[derive(Debug)]
pub struct SchematicTabSession {
    title: String,
    path: PathBuf,
    dirty: bool,
    engine: signex_engine::Engine,
}

impl SchematicTabSession {
    pub fn new(engine: signex_engine::Engine, title: String, path: PathBuf, dirty: bool) -> Self {
        Self {
            title,
            path,
            dirty,
            engine,
        }
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn save(&mut self) -> Result<(), signex_engine::EngineError> {
        self.engine.set_path(Some(self.path.clone()));
        self.engine.save()?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), signex_engine::EngineError> {
        self.engine.save_as(&path)?;
        self.title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Schematic".to_string());
        self.path = path;
        self.dirty = false;
        Ok(())
    }

    pub fn into_parts(self) -> (signex_engine::Engine, String, PathBuf, bool) {
        (self.engine, self.title, self.path, self.dirty)
    }
}

/// Per-tab auxiliary document payload. Schematic tabs keep their
/// engine in `DocumentState::engines` (keyed by path) rather than in
/// this enum; currently only PCB tabs carry a document here. Kept as
/// an enum so future tab kinds (symbol editor, footprint editor, 3D
/// viewer) can slot in without reshaping callers.
#[derive(Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum TabDocument {
    Pcb(PcbBoard),
}

impl TabDocument {
    #[allow(dead_code)]
    pub fn as_pcb(&self) -> Option<&PcbBoard> {
        match self {
            Self::Pcb(board) => Some(board),
        }
    }
}

#[derive(Debug)]
pub struct TabInfo {
    pub title: String,
    pub path: PathBuf,
    pub cached_document: Option<TabDocument>,
    pub dirty: bool,
    /// Which loaded project this tab belongs to, if any. Resolved at
    /// open time via `DocumentState::project_for_path`; a tab opened
    /// without a matching project (lone file open, project closed
    /// mid-session) carries `None`. Per-project actions (Close
    /// Project) filter tabs by this id.
    pub project_id: Option<super::state::ProjectId>,
    // WS-I: tab-not-window
    /// What kind of document this tab is hosting. Schematic / PCB
    /// tabs continue to use `path` for engine + dirty-paths bookkeeping;
    /// ComponentEditor tabs carry the `(library_path, component_id)`
    /// pair that resolves into `LibraryState.editors`.
    pub kind: TabKind,
}

// WS-7 (refactor-2): standalone primitive editor tabs
/// Per-tab state for an open `.snxsym` document. Mirrors the symbol-
/// editing fields the Component Editor's Symbol tab carries on
/// `ComponentEditorState` but keyed by file path so the same primitive
/// can be edited standalone without a hosting `Component`.
///
/// The editor reuses the existing
/// [`crate::library::editor::symbol::canvas::SymbolCanvas`] program for
/// pin layout + the existing
/// [`crate::library::editor::symbol::state`] mutation helpers, so the
/// behaviour matches the in-Component Editor experience verbatim.
#[derive(Debug)]
pub struct SymbolEditorState {
    pub path: PathBuf,
    pub primitive: Symbol,
    pub tool: crate::library::editor::symbol::canvas::SymbolTool,
    pub selected: Option<crate::library::editor::symbol::state::SymbolSelection>,
    pub ai_preview: Option<crate::library::editor::symbol::ai_stub::AiPinoutPreview>,
    pub canvas_cache: iced::widget::canvas::Cache,
    pub dirty: bool,
}

impl SymbolEditorState {
    /// Build a fresh standalone editor state from a primitive loaded
    /// off disk. `path` is the `.snxsym` file the user opened.
    pub fn new(path: PathBuf, primitive: Symbol) -> Self {
        Self {
            path,
            primitive,
            tool: crate::library::editor::symbol::canvas::SymbolTool::Select,
            selected: None,
            ai_preview: None,
            canvas_cache: iced::widget::canvas::Cache::default(),
            dirty: false,
        }
    }
}

/// Per-tab state for an open `.snxfpt` document. Mirrors the
/// footprint-editing fields on `ComponentEditorState` but keyed by
/// file path. Reuses the existing
/// [`crate::library::editor::footprint::canvas::FootprintCanvas`] +
/// [`crate::library::editor::footprint::state::FootprintEditorState`]
/// so the behaviour matches the in-Component Editor experience verbatim.
#[derive(Debug)]
pub struct FootprintEditorState {
    pub path: PathBuf,
    pub primitive: Footprint,
    pub state: crate::library::editor::footprint::state::FootprintEditorState,
    pub canvas_cache: iced::widget::canvas::Cache,
    pub dirty: bool,
}

impl FootprintEditorState {
    /// Build a fresh standalone editor state from a primitive loaded
    /// off disk. `path` is the `.snxfpt` file the user opened.
    pub fn new(path: PathBuf, primitive: Footprint) -> Self {
        let state = crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
            &primitive,
        );
        Self {
            path,
            primitive,
            state,
            canvas_cache: iced::widget::canvas::Cache::default(),
            dirty: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Wire,
    Bus,
    Label,
    Component,
    Text,
    #[allow(dead_code)]
    NoConnect,
    #[allow(dead_code)]
    BusEntry,
    Line,
    Rectangle,
    Circle,
    /// 3-click arc: first click = start, second = mid, third = end.
    Arc,
    /// Click-by-click polyline; Enter / double-click commits.
    Polyline,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::Select => write!(f, "Select"),
            Tool::Wire => write!(f, "Draw Wire"),
            Tool::Bus => write!(f, "Draw Bus"),
            Tool::Label => write!(f, "Place Label"),
            Tool::Component => write!(f, "Place Component"),
            Tool::Text => write!(f, "Place Text"),
            Tool::NoConnect => write!(f, "Place No Connect"),
            Tool::BusEntry => write!(f, "Place Bus Entry"),
            Tool::Line => write!(f, "Draw Line"),
            Tool::Rectangle => write!(f, "Draw Rectangle"),
            Tool::Circle => write!(f, "Draw Circle"),
            Tool::Arc => write!(f, "Draw Arc"),
            Tool::Polyline => write!(f, "Draw Polygon"),
        }
    }
}
