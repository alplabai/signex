use std::path::PathBuf;

use signex_types::library::Library;
use signex_types::pcb::PcbBoard;

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
/// this enum; PCB and Library tabs carry their document here. Kept as
/// an enum so future tab kinds (symbol editor, footprint editor, 3D
/// viewer) can slot in without reshaping callers.
#[derive(Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum TabDocument {
    Pcb(PcbBoard),
    /// `.snxlib` package opened in the read-only Library Browser tab.
    /// The browser reads `library.components` to populate the table;
    /// edits are out of scope until v0.10.7+ wires Component Editor.
    Library(Library),
}

impl TabDocument {
    #[allow(dead_code)]
    pub fn as_pcb(&self) -> Option<&PcbBoard> {
        match self {
            Self::Pcb(board) => Some(board),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_library(&self) -> Option<&Library> {
        match self {
            Self::Library(library) => Some(library),
            _ => None,
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
    pub project_id: Option<super::states::ProjectId>,
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
