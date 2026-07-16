use std::path::PathBuf;

use iced::Point;
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

// Per-tab role marker. Schematic / Pcb retain the path on `TabInfo`
// for the existing `engines` HashMap and dirty-paths machinery;
// `ComponentEditor` carries its own `(library_path, table, row_id)`
// payload that the dispatcher uses to look the editor state up out
// of `LibraryState.editors`. The synthetic `TabInfo.path` for
// ComponentEditor tabs is `<library_path>/<table>.tsv#<row_id>`
// (table-fragment notation, see
// `LibraryState::component_tab_path`) so undock / "is this tab
// already undocked?" / per-tab visibility continue to use a single
// PathBuf identity. No `.tsv#<row>` file is ever written to disk —
// the fragment is purely a tab-identity salt that points back at
// the row inside the table file. Per `v0.9-refactor-2-plan.md` §2.2
// the legacy `.snxprt`-per-component file format is fully retired.
#[derive(Debug, Clone)]
pub enum TabKind {
    Schematic,
    Pcb,
    ComponentEditor(ComponentEditorTab),
    /// `.snxsym` opened as a main-window document tab. Editor state
    /// lives in [`crate::app::DocumentState::symbol_editors`] keyed by
    /// the same path that lives on `TabInfo.path`.
    SymbolEditor(PathBuf),
    /// `.snxfpt` opened as a main-window document tab. Editor state
    /// lives in [`crate::app::DocumentState::footprint_editors`] keyed
    /// by the same path that lives on `TabInfo.path`.
    FootprintEditor(PathBuf),
    /// `.snxlib` opened as a Library Browser tab. The browser is the
    /// primary surface for working with library rows: tables on the
    /// left, read-only Symbol+Footprint preview on the right. Per-tab
    /// state lives in
    /// [`crate::library::state::LibraryState::library_browsers`]
    /// keyed by the same path that lives on `TabInfo.path`.
    LibraryBrowser(PathBuf),
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

    /// `Some(path)` if this tab is a Library Browser. The path is the
    /// `.snxlib` directory the browser is bound to.
    pub fn as_library_browser(&self) -> Option<&PathBuf> {
        match self {
            TabKind::LibraryBrowser(p) => Some(p),
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
    /// What kind of document this tab is hosting. Schematic / PCB
    /// tabs continue to use `path` for engine + dirty-paths bookkeeping;
    /// ComponentEditor tabs carry the `(library_path, table, row_id)`
    /// triple that resolves into `LibraryState.editors`.
    pub kind: TabKind,
}

/// Per-tab state for an open `.snxsym` document. Symbol editing
/// happens standalone (not embedded in the Component Preview tab),
/// keyed by file path so the same primitive can be edited without a
/// hosting `ComponentRow`.
///
/// The editor reuses the existing
/// [`crate::library::editor::symbol::canvas::SymbolCanvas`] program for
/// pin layout + the existing
/// [`crate::library::editor::symbol::state`] mutation helpers, so the
/// behaviour matches the in-Component Editor experience verbatim.
/// One of the three symbol-level local-colour slots. Each can carry an
/// independent RGBA override (or inherit from the sheet palette).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalColorSlot {
    Fill,
    Line,
    Pin,
}

/// Transient open-state for a placed graphic's fill colour-picker.
/// `idx` is the graphic's index in the active symbol; `advanced` is
/// `true` once the user expanded the inline palette into the HSV / RGB
/// overlay. UI-only — never serialized, never snapshotted for undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicFillPicker {
    pub idx: usize,
    pub advanced: bool,
}

/// Transient open-state for a symbol-level local-colour picker.
/// `slot` selects Fills / Lines / Pins; `advanced` is `true` once the
/// user expanded into the HSV / RGB overlay. UI-only — never
/// serialized, never snapshotted for undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalColorPicker {
    pub slot: LocalColorSlot,
    pub advanced: bool,
}

#[derive(Debug)]
pub struct SymbolEditorState {
    pub path: PathBuf,
    /// Multi-symbol container backing this `.snxsym` tab. The editor
    /// works against `file.symbols[active_idx]`; access via
    /// [`primitive`](Self::primitive) / [`primitive_mut`](Self::primitive_mut).
    pub file: signex_library::SymbolFile,
    /// Which symbol within the file is currently being edited. The
    /// SCH-Library left-dock panel will eventually drive this index;
    /// for now it always lands on the first symbol.
    pub active_idx: usize,
    /// Active sub-part for multi-part components (Altium "Part X / N"
    /// arrows). `1` is the single-part default; higher values let the
    /// user step through parts. Pins with `part_number == 0` (Part
    /// Zero) render on every part; pins with `part_number == active_part`
    /// render on the active part only.
    pub active_part: u8,
    pub tool: crate::library::editor::symbol::canvas::SymbolTool,
    pub selected: Option<crate::library::editor::symbol::state::SymbolSelection>,
    pub ai_preview: Option<crate::library::editor::symbol::ai_stub::AiPinoutPreview>,
    pub canvas_cache: iced::widget::canvas::Cache,
    pub dirty: bool,
    /// Pan/zoom camera state — mirrors the schematic canvas so the
    /// user's viewport survives across tool / selection changes
    /// within the same `.snxsym` tab. New tabs and active-symbol
    /// switches reset this to a centered-origin home view so the
    /// symbol anchor opens in a predictable place.
    pub camera: crate::canvas::Camera,
    /// Last cursor world position over the canvas, in mm. Drives the
    /// status footer's X/Y readout. `None` when the cursor is
    /// outside the canvas. Per-tab — tracking the cursor is
    /// inherently a single-canvas concept.
    pub cursor_mm: Option<(f64, f64)>,
    /// v0.13 — SchLib selection filter. Per-kind selectable flags
    /// gate which entities the canvas hit-tester considers. Mirrors
    /// the footprint editor's `selection_filter`.
    pub selection_filter: crate::library::editor::symbol::state::SymbolSelectionFilter,
    /// v0.13 — Open active-bar dropdown menu. `None` = no menu open.
    pub active_bar_menu: Option<crate::library::editor::symbol::state::SymActiveBarMenu>,
    /// v0.13 — Pause flag for placement (TAB during Place Pin / etc.).
    pub placement_paused: bool,
    /// Snapshot stack for undo. Each entry is a full clone of the
    /// `Symbol` at the moment before a mutation was applied. Max 100
    /// entries; oldest entries are dropped when the limit is exceeded.
    pub undo_snapshots: Vec<signex_library::Symbol>,
    /// Snapshot stack for redo. Cleared whenever a new mutation is
    /// recorded; populated by `SymbolUndo` so the user can step
    /// forward again after undoing.
    pub redo_snapshots: Vec<signex_library::Symbol>,
    /// Set to `true` on the first `Move`/`MoveAll`/`MoveGraphicHandle`
    /// in a drag sequence so subsequent move events in the same drag do
    /// NOT push additional snapshots (the pre-drag snapshot is already
    /// on the undo stack). Cleared by `SymbolDragCommit` (emitted on
    /// `ButtonReleased`).
    pub mid_drag: bool,
    /// Which placed graphic's fill colour-picker is open, plus whether
    /// it has been expanded to the HSV / RGB overlay. `None` = closed.
    /// Transient UI-only state: never serialized, never snapshotted for
    /// undo.
    pub graphic_fill_picker: Option<GraphicFillPicker>,
    /// Which symbol-level local colour slot's picker is open. `None` =
    /// closed. Transient UI-only state: never serialized, never
    /// snapshotted for undo.
    pub local_color_picker: Option<LocalColorPicker>,
    /// Click-collect vertex stash for the canvas's `PlacePolygon`
    /// tool (grid-snapped mm world positions). Lives here — on the
    /// per-document editor model — rather than on the canvas
    /// `Program::State`, which iced reuses across tab switches for
    /// whichever `.snxsym` tab is currently rendered; a stash kept
    /// there would survive a switch to a different document and could
    /// mis-commit into it. Mirrors the footprint editor's
    /// `FootprintEditorState::place_polygon_vertices`. Mutated only by
    /// `SymbolEditorMsg::PolygonClick` / `PolygonCommit` /
    /// `PolygonCancel`, plus the synchronous flush in the `SetTool`
    /// handler when the user switches away from `PlacePolygon`.
    pub polygon_vertices: Vec<(f64, f64)>,
}

impl SymbolEditorState {
    const DEFAULT_SYMBOL_VIEWPORT_WIDTH: f32 = 800.0;
    const DEFAULT_SYMBOL_VIEWPORT_HEIGHT: f32 = 500.0;

    /// Build a fresh standalone editor state from a `SymbolFile`
    /// container loaded off disk. `path` is the `.snxsym` file the
    /// user opened. The editor opens on the first symbol in the file.
    pub fn new(path: PathBuf, file: signex_library::SymbolFile) -> Self {
        let mut state = Self {
            path,
            file,
            active_idx: 0,
            active_part: 1,
            tool: crate::library::editor::symbol::canvas::SymbolTool::Select,
            selected: None,
            ai_preview: None,
            canvas_cache: iced::widget::canvas::Cache::default(),
            dirty: false,
            camera: crate::canvas::Camera::default(),
            cursor_mm: None,
            selection_filter: Default::default(),
            active_bar_menu: None,
            placement_paused: false,
            undo_snapshots: Vec::new(),
            redo_snapshots: Vec::new(),
            mid_drag: false,
            graphic_fill_picker: None,
            local_color_picker: None,
            polygon_vertices: Vec::new(),
        };
        state.reset_camera_origin_center();
        state
    }

    /// Reset pan/zoom and place world origin in the center of the
    /// default symbol-canvas viewport.
    pub(crate) fn reset_camera_origin_center(&mut self) {
        self.camera = crate::canvas::Camera::default();
        self.camera.offset = Point::new(
            Self::DEFAULT_SYMBOL_VIEWPORT_WIDTH * 0.5,
            Self::DEFAULT_SYMBOL_VIEWPORT_HEIGHT * 0.5,
        );
    }

    /// Borrow the symbol currently being edited. Falls back to the
    /// first symbol in the file when `active_idx` is out of range
    /// (defensive — should never happen in practice). Panics only
    /// when the file has zero symbols, which the loader rejects.
    pub fn primitive(&self) -> &Symbol {
        let idx = self
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        &self.file.symbols[idx]
    }

    /// Mutable borrow of the active symbol — used by canvas mutations
    /// (add/move/delete pin etc.). Same out-of-range fallback as
    /// [`primitive`](Self::primitive).
    pub fn primitive_mut(&mut self) -> &mut Symbol {
        let idx = self
            .active_idx
            .min(self.file.symbols.len().saturating_sub(1));
        &mut self.file.symbols[idx]
    }
}

/// Per-tab state for an open `.snxfpt` document. Mirrors the
/// footprint-editing fields on `ComponentEditorState` but keyed by
/// file path. Reuses the existing
/// [`crate::library::editor::footprint::canvas::FootprintCanvas`] +
/// [`crate::library::editor::footprint::state::FootprintEditorState`]
/// so the behaviour matches the in-Component Editor experience verbatim.
///
/// v0.18.6 — mirrors [`SymbolEditorState`]: the editor backs a multi-
/// footprint container and dispatches every mutation through
/// `file.footprints[active_idx]`. Saves preserve `file_uuid` and any
/// future siblings instead of minting a fresh single-footprint
/// envelope.
#[derive(Debug)]
pub struct FootprintEditorState {
    pub path: PathBuf,
    /// Multi-footprint container backing this `.snxfpt` tab. The
    /// editor works against `file.footprints[active_idx]`; access
    /// via [`primitive`](Self::primitive) /
    /// [`primitive_mut`](Self::primitive_mut).
    pub file: signex_library::FootprintFile,
    /// Which footprint within the file is currently being edited.
    /// The Footprint Library left-dock panel + canvas tab strip
    /// drive this; defaults to the first footprint.
    pub active_idx: usize,
    /// v0.18.8 — Footprint Library panel single-click selection.
    /// Independent of `active_idx`: a single click highlights the
    /// row, double-click (or the panel's Edit button) promotes the
    /// selection to `active_idx`. `None` until the user clicks a
    /// row.
    pub panel_selected_idx: Option<usize>,
    pub state: crate::library::editor::footprint::state::FootprintEditorState,
    pub canvas_cache: iced::widget::canvas::Cache,
    pub dirty: bool,
    /// v0.24 Phase 1 (Track B) — Undo history. Pre-mutation snapshots
    /// pushed by `push_history` ahead of each dispatcher mutation;
    /// `undo()` pops the most recent and swaps it in. Capped at
    /// [`Self::HISTORY_DEPTH`] to keep memory bounded.
    pub history: Vec<FootprintHistorySnapshot>,
    /// v0.24 Phase 1 (Track B) — Redo stack. Populated by `undo()`
    /// with the snapshot it just rolled back; cleared by any new
    /// mutation that calls `push_history` so the redo lineage stays
    /// consistent with single-history-tree semantics.
    pub redo: Vec<FootprintHistorySnapshot>,
}

/// v0.24 Phase 1 (Track B) — coarse-grained undo snapshot. Captures
/// the canonical state needed to roll back any footprint edit:
/// - the full multi-footprint `FootprintFile` (typically <50 KB of
///   in-memory representation),
/// - the active footprint index,
/// - the canvas-side `EditorPad` cache and its selection state,
/// - sketch selection state.
///
/// Stored by-value (not by-Arc) so undo/redo never aliases the live
/// state. Memory cost: ~5 KB × `HISTORY_DEPTH` = ~500 KB per editor.
/// Acceptable for v0.24; fine-grained inverse ops can replace this
/// later if memory becomes load-bearing.
#[derive(Debug, Clone)]
pub struct FootprintHistorySnapshot {
    pub file: signex_library::FootprintFile,
    pub active_idx: usize,
    pub pads: Vec<crate::library::editor::footprint::state::EditorPad>,
    pub selected_pad: Option<usize>,
    pub selected_sketch: Option<signex_sketch::id::SketchEntityId>,
    pub selected_sketch_secondary: Option<signex_sketch::id::SketchEntityId>,
}

impl FootprintEditorState {
    /// Build a fresh standalone editor state from a `FootprintFile`
    /// container loaded off disk. `path` is the `.snxfpt` file the
    /// user opened. The editor opens on the first footprint in the
    /// file. The caller is responsible for confirming the file is
    /// non-empty before this call.
    pub fn new(path: PathBuf, file: signex_library::FootprintFile) -> Self {
        let active_idx = 0;
        let state = crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
            &file.footprints[active_idx],
        );
        Self {
            path,
            file,
            active_idx,
            panel_selected_idx: None,
            state,
            canvas_cache: iced::widget::canvas::Cache::default(),
            dirty: false,
            history: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// v0.24 Phase 1 (Track B) — depth cap for `history` / `redo`
    /// stacks. 100 entries × ~5 KB = ~500 KB per editor; raise if
    /// users start losing far-back undos and we've moved to
    /// fine-grained inverse ops.
    pub const HISTORY_DEPTH: usize = 100;

    /// v0.24 Phase 1 (Track B) — capture a snapshot of the
    /// canonical state ahead of a mutation. Caller invokes this
    /// BEFORE the mutation runs; on `undo()` we swap the snapshot
    /// back in. New mutations clear the redo stack so the history
    /// stays a single timeline.
    ///
    /// Phase 2 wiring: every `dispatch::library` arm that mutates
    /// state should `push_history()` first, then run the mutation.
    /// The dispatcher's existing `with_parts` closure is the
    /// natural integration site.
    pub fn push_history(&mut self) {
        let snap = self.snapshot();
        self.history.push(snap);
        if self.history.len() > Self::HISTORY_DEPTH {
            self.history.remove(0);
        }
        self.redo.clear();
    }

    /// v0.24 Phase 1 — undo the most recent edit. Returns `true` if
    /// a snapshot was popped + applied, `false` if the history was
    /// empty (i.e. nothing to undo). The current state is pushed
    /// onto the redo stack so `redo()` can walk forward again.
    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.history.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo.push(current);
        if self.redo.len() > Self::HISTORY_DEPTH {
            self.redo.remove(0);
        }
        self.restore(snap);
        true
    }

    /// v0.24 Phase 1 — redo the most recently undone edit. Returns
    /// `true` if a redo snapshot was popped + applied, `false` when
    /// the redo stack is empty.
    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.history.push(current);
        if self.history.len() > Self::HISTORY_DEPTH {
            self.history.remove(0);
        }
        self.restore(snap);
        true
    }

    /// v0.24 Phase 1 — capture the current state into a snapshot
    /// without altering the history stack. Used by `push_history`
    /// + `undo`/`redo` to read the canonical state by-value.
    fn snapshot(&self) -> FootprintHistorySnapshot {
        FootprintHistorySnapshot {
            file: self.file.clone(),
            active_idx: self.active_idx,
            pads: self.state.pads.clone(),
            selected_pad: self.state.selected_pad,
            selected_sketch: self.state.selected_sketch,
            selected_sketch_secondary: self.state.selected_sketch_secondary,
        }
    }

    /// v0.24 Phase 1 — write a snapshot back into the live editor
    /// state. Resets the canvas cache so the next render reflects
    /// the rolled-back geometry.
    fn restore(&mut self, snap: FootprintHistorySnapshot) {
        self.file = snap.file;
        self.active_idx = snap.active_idx;
        self.state.pads = snap.pads;
        self.state.selected_pad = snap.selected_pad;
        self.state.selected_sketch = snap.selected_sketch;
        self.state.selected_sketch_secondary = snap.selected_sketch_secondary;
        self.canvas_cache.clear();
        self.dirty = true;
    }

    /// HI-23: builder that seeds `global_snap_disabled` from
    /// `ui_state.snap_enabled` so opening a `.snxfpt` while the user
    /// has the global snap toggle off doesn't surprise them with snap
    /// suddenly back on. Call sites pass `!ui_state.snap_enabled`.
    pub fn with_global_snap_disabled(mut self, disabled: bool) -> Self {
        self.state.global_snap_disabled = disabled;
        self
    }

    /// Borrow the footprint currently being edited. Falls back to
    /// the first footprint when `active_idx` is out of range
    /// (defensive — should never happen in practice). Panics only
    /// when the file has zero footprints, which the loader rejects.
    pub fn primitive(&self) -> &Footprint {
        let idx = self
            .active_idx
            .min(self.file.footprints.len().saturating_sub(1));
        &self.file.footprints[idx]
    }

    /// Mutable borrow of the active footprint — used by canvas
    /// mutations (add/move/delete pad etc.). Same out-of-range
    /// fallback as [`primitive`](Self::primitive).
    pub fn primitive_mut(&mut self) -> &mut Footprint {
        let idx = self
            .active_idx
            .min(self.file.footprints.len().saturating_sub(1));
        &mut self.file.footprints[idx]
    }

    /// Split-borrow accessor returning mutable references to
    /// `state` and the active primitive simultaneously. The two
    /// fields are disjoint (`state` lives next to `file`), but
    /// `&mut self`-shaped methods can't express that — calling
    /// `editor.primitive_mut()` after `&mut editor.state` trips the
    /// borrow checker. Destructuring `Self` makes disjointness
    /// explicit. Callers passing both halves into a helper like
    /// `apply_sketch_edit_with_warnings` reach for this.
    pub fn parts_mut(
        &mut self,
    ) -> (
        &mut crate::library::editor::footprint::state::FootprintEditorState,
        &mut Footprint,
    ) {
        let Self {
            state,
            file,
            active_idx,
            ..
        } = self;
        let idx = (*active_idx).min(file.footprints.len().saturating_sub(1));
        (state, &mut file.footprints[idx])
    }

    /// Closure-shaped split-borrow. The closure runs with both halves
    /// in scope; the borrows are scoped to its body. Equivalent to
    /// [`parts_mut`](Self::parts_mut) but plays nicer when the call
    /// site needs to touch other `editor` fields after the split-
    /// borrowed work returns — the closure boundary forces NLL to
    /// release the borrows promptly.
    pub fn with_parts<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(
            &mut crate::library::editor::footprint::state::FootprintEditorState,
            &mut Footprint,
        ) -> R,
    {
        let (state, primitive) = self.parts_mut();
        f(state, primitive)
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
