# EDA Application Patterns for iced

> Patterns derived from building schematic/PCB editors with iced 0.14.
> All coordinates are in nanometers (i64) internally.

---

## Coordinate system — nanometers

Use `i64` nanometers internally. Convert **only** at the parse/render boundary.

```rust
// Domain type — zero rendering deps
// Lives in `your-types` crate, not in the iced crate.

/// Position in schematic space, nanometers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i64, // nm
    pub y: i64, // nm
}

impl Point {
    pub const ORIGIN: Self = Self { x: 0, y: 0 };

    pub fn distance_to(self, other: Self) -> f64 {
        let dx = (self.x - other.x) as f64;
        let dy = (self.y - other.y) as f64;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Axis-aligned bounding box in nanometers.
#[derive(Debug, Clone, Copy)]
pub struct BBox {
    pub min_x: i64,
    pub min_y: i64,
    pub max_x: i64,
    pub max_y: i64,
}

impl BBox {
    pub fn width(self)  -> i64 { self.max_x - self.min_x }
    pub fn height(self) -> i64 { self.max_y - self.min_y }
    pub fn cx(self)     -> i64 { (self.min_x + self.max_x) / 2 }
    pub fn cy(self)     -> i64 { (self.min_y + self.max_y) / 2 }

    pub fn contains(self, p: Point) -> bool {
        p.x >= self.min_x && p.x <= self.max_x &&
        p.y >= self.min_y && p.y <= self.max_y
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min_x < other.max_x && self.max_x > other.min_x &&
        self.min_y < other.max_y && self.max_y > other.min_y
    }
}

/// Standard schematic grid: 50 mil = 1,270,000 nm
pub const GRID_50_MIL_NM: i64 = 1_270_000;
pub const GRID_25_MIL_NM: i64 = 635_000;

pub fn snap_to_grid(value: i64, grid: i64) -> i64 {
    let half = grid / 2;
    ((value + half) / grid) * grid
}
```

---

## Crate workspace separation

```
workspace/
  types/    Domain types (Point, BBox, SchItem, NetList)
            ZERO iced/wgpu deps — pure data + logic.
  parser/   S-expression parser (.kicad_sch / .kicad_pcb / .kicad_sym)
  writer/   S-expression serializer (lossless write-back to KiCad format)
  render/   Rendering logic (types -> Canvas draw calls / wgpu primitives)
  widgets/  Reusable iced widgets (tree view, layer panel, symbol preview)
  app/      iced::application binary (State, Message, update, view)
```

**Rule**: `types` must not import `iced`, `wgpu`, or any rendering crate.
All rendering goes through `render`. This makes `types` trivially testable
without a GPU.

```toml
# types/Cargo.toml — no iced dep
[dependencies]
serde = { version = "1", features = ["derive"] }
uuid  = { version = "1", features = ["v4"] }

# render/Cargo.toml
[dependencies]
iced  = { version = "0.14", features = ["canvas", "wgpu"] }
types = { path = "../types" }

# app/Cargo.toml
[dependencies]
iced    = { version = "0.14", features = ["tokio", "canvas", "wgpu"] }
types   = { path = "../types" }
render  = { path = "../render" }
widgets = { path = "../widgets" }
```

---

## Command pattern — undo/redo

```rust
/// A reversible edit on the schematic.
pub trait Command: std::fmt::Debug + Send + Sync {
    fn execute(&self, schematic: &mut Schematic);
    fn undo(&self,    schematic: &mut Schematic);
    fn description(&self) -> &str;
}

/// Undo/redo stack — 100-step history.
pub struct History {
    past:   Vec<Box<dyn Command>>,
    future: Vec<Box<dyn Command>>,
    limit:  usize,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self { past: Vec::new(), future: Vec::new(), limit }
    }

    pub fn execute(&mut self, cmd: Box<dyn Command>, schematic: &mut Schematic) {
        cmd.execute(schematic);
        self.past.push(cmd);
        self.future.clear();                       // branching invalidates future
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
    }

    pub fn undo(&mut self, schematic: &mut Schematic) -> bool {
        if let Some(cmd) = self.past.pop() {
            cmd.undo(schematic);
            self.future.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, schematic: &mut Schematic) -> bool {
        if let Some(cmd) = self.future.pop() {
            cmd.execute(schematic);
            self.past.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
}

// Example command: move selected items
#[derive(Debug)]
pub struct MoveCommand {
    item_ids: Vec<ItemId>,
    delta:    (i64, i64), // nm
}

impl Command for MoveCommand {
    fn execute(&self, sch: &mut Schematic) {
        for id in &self.item_ids {
            if let Some(item) = sch.get_mut(*id) {
                item.translate(self.delta.0, self.delta.1);
            }
        }
    }

    fn undo(&self, sch: &mut Schematic) {
        for id in &self.item_ids {
            if let Some(item) = sch.get_mut(*id) {
                item.translate(-self.delta.0, -self.delta.1);
            }
        }
    }

    fn description(&self) -> &str { "Move items" }
}

// In app update():
Message::Undo => {
    if self.history.undo(&mut self.schematic) {
        self.canvas_cache.clear();
    }
    Task::none()
}
Message::Redo => {
    if self.history.redo(&mut self.schematic) {
        self.canvas_cache.clear();
    }
    Task::none()
}
Message::MoveSelection(delta) => {
    let cmd = Box::new(MoveCommand {
        item_ids: self.selected.iter().copied().collect(),
        delta,
    });
    self.history.execute(cmd, &mut self.schematic);
    self.canvas_cache.clear();
    Task::none()
}
```

---

## Multi-tab document management

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DocumentTab {
    pub id:       TabId,
    pub path:     Option<PathBuf>,
    pub title:    String,
    pub dirty:    bool,     // unsaved changes
    pub document: Document, // enum { Schematic(...), Board(...) }
}

pub struct TabManager {
    tabs:   Vec<DocumentTab>,
    active: Option<TabId>,
}

impl TabManager {
    pub fn open(&mut self, path: PathBuf, doc: Document) -> TabId {
        let id = TabId::new();
        let title = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        self.tabs.push(DocumentTab { id, path: Some(path), title, dirty: false, document: doc });
        self.active = Some(id);
        id
    }

    pub fn close(&mut self, id: TabId) -> CloseResult {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
            if tab.dirty {
                return CloseResult::NeedsSave;
            }
        }
        self.tabs.retain(|t| t.id != id);
        // Activate adjacent tab
        if Some(id) == self.active {
            self.active = self.tabs.last().map(|t| t.id);
        }
        CloseResult::Closed
    }

    pub fn mark_dirty(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.dirty = true;
        }
    }

    pub fn mark_clean(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.dirty = false;
        }
    }

    pub fn active_tab(&self) -> Option<&DocumentTab> {
        self.active.and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }
}

pub enum CloseResult { Closed, NeedsSave }

// Tab bar view (using iced_aw tabs or manual row):
fn view_tab_bar(tabs: &TabManager) -> Element<'_, Message> {
    use iced::widget::{row, button, text, Space};

    let tab_buttons = tabs.tabs.iter().map(|tab| {
        let label = if tab.dirty {
            format!("● {}", tab.title) // dirty indicator
        } else {
            tab.title.clone()
        };
        let is_active = Some(tab.id) == tabs.active;
        let style = if is_active { button::primary } else { button::secondary };
        row![
            button(text(label)).style(style).on_press(Message::TabActivated(tab.id)),
            button("×").style(button::text).on_press(Message::TabCloseRequested(tab.id)),
        ]
        .into()
    });

    iced::widget::Row::with_children(tab_buttons.collect())
        .spacing(2)
        .into()
}
```

---

## Keyboard shortcuts (Altium-style)

```rust
use iced::keyboard::{self, Key, Modifiers};

fn handle_keyboard(key: Key, mods: Modifiers) -> Option<Message> {
    match (key, mods) {
        // File
        (Key::Character(c), Modifiers::CTRL) if c == "s" => Some(Message::Save),
        (Key::Character(c), Modifiers::CTRL | Modifiers::SHIFT) if c == "s" => {
            Some(Message::SaveAs)
        }
        (Key::Character(c), Modifiers::CTRL) if c == "z" => Some(Message::Undo),
        (Key::Character(c), Modifiers::CTRL | Modifiers::SHIFT) if c == "z" => {
            Some(Message::Redo)
        }
        (Key::Character(c), Modifiers::CTRL) if c == "y" => Some(Message::Redo),
        (Key::Character(c), Modifiers::CTRL) if c == "a" => Some(Message::SelectAll),
        (Key::Character(c), Modifiers::CTRL) if c == "c" => Some(Message::Copy),
        (Key::Character(c), Modifiers::CTRL) if c == "v" => Some(Message::Paste),
        // Edit tools (single key, no modifier)
        (Key::Named(keyboard::Named::Delete), _)       => Some(Message::DeleteSelected),
        (Key::Named(keyboard::Named::Escape), _)        => Some(Message::CancelTool),
        (Key::Character(c), Modifiers::empty()) if c == "r" => Some(Message::RotateSelected),
        (Key::Character(c), Modifiers::empty()) if c == "x" => Some(Message::MirrorX),
        (Key::Character(c), Modifiers::empty()) if c == "y" => Some(Message::MirrorY),
        // View
        (Key::Character(c), Modifiers::empty()) if c == "f" => Some(Message::FitAll),
        _ => None,
    }
}

// Register in subscription:
fn subscription(state: &AppState) -> iced::Subscription<Message> {
    keyboard::on_key_press(|key, mods| handle_keyboard(key, mods))
}
```

---

## Deterministic file output

When writing KiCad files (or any serialised format), sort `HashMap` keys to produce
stable diffs in version control:

```rust
use std::collections::BTreeMap; // Always sorted

// Prefer BTreeMap over HashMap for any data that will be serialised.
pub struct NetList {
    nets: BTreeMap<String, Net>, // sorted by net name
}

// If you must use HashMap (e.g. for O(1) lookup) then sort at write time:
fn write_properties(map: &std::collections::HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = map.iter().collect();
    pairs.sort_by_key(|(k, _)| k.as_str());
    pairs.iter()
        .map(|(k, v)| format!("(property {:?} {:?})", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}
```

---

## Rich text markup (subscript, superscript, overbar)

KiCad supports `~{}` overbar, `_{...}` subscript, `^{...}` superscript in text fields.

```rust
#[derive(Debug, Clone)]
pub enum TextSpan {
    Normal(String),
    Overbar(String),
    Subscript(String),
    Superscript(String),
}

pub fn parse_rich_text(input: &str) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    let mut chars = input.chars().peekable();
    let mut buf   = String::new();

    while let Some(c) = chars.next() {
        match c {
            '~' if chars.peek() == Some(&'{') => {
                chars.next();
                if !buf.is_empty() { spans.push(TextSpan::Normal(buf.drain(..).collect())); }
                let inner: String = chars.by_ref().take_while(|&c| c != '}').collect();
                spans.push(TextSpan::Overbar(inner));
            }
            '_' if chars.peek() == Some(&'{') => {
                chars.next();
                if !buf.is_empty() { spans.push(TextSpan::Normal(buf.drain(..).collect())); }
                let inner: String = chars.by_ref().take_while(|&c| c != '}').collect();
                spans.push(TextSpan::Subscript(inner));
            }
            '^' if chars.peek() == Some(&'{') => {
                chars.next();
                if !buf.is_empty() { spans.push(TextSpan::Normal(buf.drain(..).collect())); }
                let inner: String = chars.by_ref().take_while(|&c| c != '}').collect();
                spans.push(TextSpan::Superscript(inner));
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() { spans.push(TextSpan::Normal(buf)); }
    spans
}
```

---

## Selection overlay (Altium-style)

Draw a cyan highlight with corner grip handles over selected items:

```rust
fn draw_selection_overlay(frame: &mut Frame, selected: &[BBox], viewport: &Viewport) {
    for bbox in selected {
        let tl = viewport.to_canvas(bbox.min_x, bbox.max_y);
        let br = viewport.to_canvas(bbox.max_x, bbox.min_y);
        let w  = br.x - tl.x;
        let h  = br.y - tl.y;

        // Cyan outline
        frame.stroke(
            &Path::rectangle(tl, iced::Size::new(w, h)),
            Stroke::default()
                .with_color(Color::from_rgb(0.0, 0.8, 1.0))
                .with_width(1.0),
        );

        // Corner grip handles (4 px squares)
        let corners = [tl, Point::new(br.x, tl.y), br, Point::new(tl.x, br.y)];
        for corner in corners {
            frame.fill_rectangle(
                Point::new(corner.x - 3.0, corner.y - 3.0),
                iced::Size::new(6.0, 6.0),
                Color::from_rgb(0.0, 0.8, 1.0),
            );
        }
    }
}
```
