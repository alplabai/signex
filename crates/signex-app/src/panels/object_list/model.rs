//! The Object List's rows (#1259): a presentation projection of a sheet's
//! objects. Pure: [`sheet_rows`] reads one sheet, [`filtered`] and
//! [`kind_counts`] narrow a list by the panel's filter and kind chips. Net
//! names come from the caller (the cached project net index), never from a
//! walk here. Names and values are the text the canvas draws (#2630), by the
//! canvas's own rules ([`SheetLook`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_i18n::I18n;
use signex_types::designator::DesignatorStandard;
use signex_types::schematic::{
    InstancePath, LabelType, Point, SchDirective, SchDrawing, SchematicSheet, SelectedItem,
    SelectedKind, Symbol,
};
use signex_widgets::chips::FilterSet;
use uuid::Uuid;

use crate::canvas::SpecialStrings;
use crate::schematic_runtime::drawn_value::{drawn_designator, drawn_value};

/// How many rows the panel draws before a "more" row; each press of it adds
/// as many again.
pub const ROW_PAGE: usize = 200;

/// What an object is, one per kind chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKind {
    Part,
    Label,
    Wire,
    PowerPort,
    Port,
    SheetSymbol,
    SheetEntry,
    Text,
    Junction,
    Bus,
    Drawing,
    Directive,
}

impl ObjectKind {
    /// Every kind, in chip order.
    pub const ALL: [ObjectKind; 12] = [
        Self::Part,
        Self::Label,
        Self::Wire,
        Self::PowerPort,
        Self::Port,
        Self::SheetSymbol,
        Self::SheetEntry,
        Self::Text,
        Self::Junction,
        Self::Bus,
        Self::Drawing,
        Self::Directive,
    ];
    /// The kinds with a chip of their own; the rest sit behind More.
    pub const MAIN: [ObjectKind; 3] = [Self::Part, Self::Label, Self::Wire];

    pub fn is_main(self) -> bool {
        Self::MAIN.contains(&self)
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// The scope the panel lists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ObjectScope {
    #[default]
    ThisSheet,
    Project,
}

/// A kind chip: All, or one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindChip {
    All,
    Kind(ObjectKind),
}

/// Where an object is: its sheet file, the canvas item and a point to centre
/// on.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectRef {
    pub sheet: PathBuf,
    pub item: SelectedItem,
    pub at: Point,
}

/// The Name column when the object has no name of its own: what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectNoun {
    Wire,
    Text,
    Junction,
    Bus,
    Drawing,
    Blanket,
    CompileMask,
    DiffPair,
    NetParameters,
}

/// A fixed word in the Value column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueWord {
    NetLabel,
    PowerPort,
    Port,
    SheetEntry,
}

/// The Name / designator column.
#[derive(Debug, Clone, PartialEq)]
pub enum RowName {
    Text(String),
    Noun(ObjectNoun),
}

/// The Value / text column.
#[derive(Debug, Clone, PartialEq)]
pub enum RowValue {
    Empty,
    Text(String),
    Word(ValueWord),
    /// A wire or bus: its net when the index knows it, and its length.
    Length {
        net: Option<String>,
        mm: f64,
    },
}

/// One row of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectRow {
    pub kind: ObjectKind,
    pub name: RowName,
    pub value: RowValue,
    /// The sheet as the user knows it (the file stem).
    pub sheet: String,
    pub target: ObjectRef,
}

impl ObjectRow {
    /// Whether `needle` (folded by `i18n`, trimmed) is in the name, the value or
    /// the net. An empty needle matches every row.
    pub fn matches(&self, needle: &str, i18n: &I18n) -> bool {
        if needle.is_empty() {
            return true;
        }
        let hit = |s: &str| i18n.fold_for_search(s).contains(needle);
        let name = matches!(&self.name, RowName::Text(s) if hit(s));
        let value = match &self.value {
            RowValue::Text(s) => hit(s),
            RowValue::Length { net: Some(net), .. } => hit(net),
            _ => false,
        };
        name || value
    }
}

/// How many objects of each kind, in [`ObjectKind::ALL`] order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KindCounts([usize; 12]);

impl KindCounts {
    pub fn of(&self, kind: ObjectKind) -> usize {
        self.0[kind.index()]
    }

    pub fn total(&self) -> usize {
        self.0.iter().sum()
    }

    /// The objects of the kinds behind More.
    pub fn more(&self) -> usize {
        ObjectKind::ALL
            .iter()
            .filter(|k| !k.is_main())
            .map(|k| self.of(*k))
            .sum()
    }
}

/// The display name of the sheet at `path`: its file stem.
pub fn sheet_label(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.display().to_string(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// What the canvas draws a sheet's text with, so a row reads as the canvas
/// does (#2630). The default draws every text as stored.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetLook {
    /// The placement whose designators the sheet shows (#1151); `None`
    /// shows each part's `reference`.
    pub placement: Option<InstancePath>,
    /// What `${TOKEN}`s resolve to on the sheet; `None` for a sheet with no
    /// open document, whose expressions are evaluated on the sheet alone.
    pub special: Option<SpecialStrings>,
    /// The project's designator standard (#1416).
    pub standard: Option<DesignatorStandard>,
}

impl SheetLook {
    /// A part's designator as the canvas draws it.
    fn designator<'a>(&self, symbol: &'a Symbol) -> std::borrow::Cow<'a, str> {
        drawn_designator(symbol, self.placement.as_ref(), self.standard)
    }

    /// A part's (or a power port's) value as the canvas draws it.
    fn value(&self, symbols: &[Symbol], symbol: &Symbol) -> String {
        drawn_value(symbols, symbol, self.special.as_ref(), self.standard)
    }

    /// A text note as the canvas draws it.
    fn note(&self, text: &str) -> String {
        match &self.special {
            Some(special) => special.resolve_text(text),
            None => text.to_string(),
        }
    }
}

/// The look of each sheet of a Project read, by path; a sheet not in it
/// draws with `fallback`.
#[derive(Debug, Clone, Default)]
pub struct SheetLooks {
    pub by_path: HashMap<PathBuf, SheetLook>,
    pub fallback: SheetLook,
}

impl SheetLooks {
    pub fn of(&self, path: &Path) -> &SheetLook {
        self.by_path.get(path).unwrap_or(&self.fallback)
    }
}

/// Every listed object of `sheet`, read from the file at `path`, its names
/// and values as `look` draws them. `wire_net` names the net a wire or bus
/// is on, when the caller's index knows it.
pub fn sheet_rows(
    sheet: &SchematicSheet,
    path: &Path,
    look: &SheetLook,
    wire_net: &dyn Fn(Uuid) -> Option<String>,
) -> Vec<ObjectRow> {
    let label = sheet_label(path);
    let mut rows = Vec::new();
    let mut push = |kind, name, value, item: SelectedItem, at: Point| {
        rows.push(ObjectRow {
            kind,
            name,
            value,
            sheet: label.clone(),
            target: ObjectRef {
                sheet: path.to_path_buf(),
                item,
                at,
            },
        });
    };
    for s in &sheet.symbols {
        let item = SelectedItem::new(s.uuid, SelectedKind::Symbol);
        if s.is_power {
            let name = RowName::Text(look.value(&sheet.symbols, s));
            let value = RowValue::Word(ValueWord::PowerPort);
            push(ObjectKind::PowerPort, name, value, item, s.position);
        } else {
            let name = RowName::Text(look.designator(s).into_owned());
            let value = RowValue::Text(look.value(&sheet.symbols, s));
            push(ObjectKind::Part, name, value, item, s.position);
        }
    }
    for l in &sheet.labels {
        let (kind, word) = match l.label_type {
            LabelType::Net => (ObjectKind::Label, ValueWord::NetLabel),
            LabelType::Power => (ObjectKind::PowerPort, ValueWord::PowerPort),
            LabelType::Global | LabelType::Hierarchical => (ObjectKind::Port, ValueWord::Port),
        };
        let item = SelectedItem::new(l.uuid, SelectedKind::Label);
        push(
            kind,
            RowName::Text(l.text.clone()),
            RowValue::Word(word),
            item,
            l.position,
        );
    }
    for c in &sheet.child_sheets {
        let item = SelectedItem::new(c.uuid, SelectedKind::ChildSheet);
        let name = RowName::Text(c.name.clone());
        let value = RowValue::Text(c.filename.clone());
        push(ObjectKind::SheetSymbol, name, value, item, c.position);
        for pin in &c.pins {
            let item = SelectedItem::new(pin.uuid, SelectedKind::SheetPin);
            let name = RowName::Text(pin.name.clone());
            let value = RowValue::Word(ValueWord::SheetEntry);
            push(ObjectKind::SheetEntry, name, value, item, pin.position);
        }
    }
    for w in &sheet.wires {
        let item = SelectedItem::new(w.uuid, SelectedKind::Wire);
        let value = RowValue::Length {
            net: wire_net(w.uuid),
            mm: length(w.start, w.end),
        };
        let name = RowName::Noun(ObjectNoun::Wire);
        push(
            ObjectKind::Wire,
            name,
            value,
            item,
            midpoint(w.start, w.end),
        );
    }
    for b in &sheet.buses {
        let item = SelectedItem::new(b.uuid, SelectedKind::Bus);
        let value = RowValue::Length {
            net: wire_net(b.uuid),
            mm: length(b.start, b.end),
        };
        let name = RowName::Noun(ObjectNoun::Bus);
        push(ObjectKind::Bus, name, value, item, midpoint(b.start, b.end));
    }
    for t in &sheet.text_notes {
        let item = SelectedItem::new(t.uuid, SelectedKind::TextNote);
        let name = RowName::Noun(ObjectNoun::Text);
        let value = RowValue::Text(look.note(&t.text));
        push(ObjectKind::Text, name, value, item, t.position);
    }
    for j in &sheet.junctions {
        let item = SelectedItem::new(j.uuid, SelectedKind::Junction);
        let name = RowName::Noun(ObjectNoun::Junction);
        push(
            ObjectKind::Junction,
            name,
            RowValue::Empty,
            item,
            j.position,
        );
    }
    for d in &sheet.drawings {
        let item = SelectedItem::new(d.uuid(), SelectedKind::Drawing);
        let name = RowName::Noun(ObjectNoun::Drawing);
        push(
            ObjectKind::Drawing,
            name,
            RowValue::Empty,
            item,
            drawing_at(d),
        );
    }
    for d in &sheet.directives {
        let item = SelectedItem::new(d.uuid(), SelectedKind::Directive);
        let (noun, at) = match d {
            SchDirective::Blanket { start, end, .. } => {
                (ObjectNoun::Blanket, midpoint(*start, *end))
            }
            SchDirective::CompileMask { start, end, .. } => {
                (ObjectNoun::CompileMask, midpoint(*start, *end))
            }
            SchDirective::DiffPair { position, .. } => (ObjectNoun::DiffPair, *position),
            SchDirective::NetParameters { position, .. } => (ObjectNoun::NetParameters, *position),
        };
        let value = match d {
            SchDirective::Blanket {
                net_class: Some(class),
                ..
            }
            | SchDirective::NetParameters {
                net_class: Some(class),
                ..
            } => RowValue::Text(class.clone()),
            _ => RowValue::Empty,
        };
        push(ObjectKind::Directive, RowName::Noun(noun), value, item, at);
    }
    rows
}

fn length(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// A point on a drawing to centre on.
fn drawing_at(d: &SchDrawing) -> Point {
    match d {
        SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => {
            midpoint(*start, *end)
        }
        SchDrawing::Circle { center, .. } => *center,
        SchDrawing::Arc { mid, .. } => *mid,
        SchDrawing::Polyline { points, .. } => points.first().copied().unwrap_or(Point::ZERO),
        SchDrawing::Bezier(b) => midpoint(b.start, b.end),
        SchDrawing::Dimension(d) => midpoint(d.from, d.to),
        SchDrawing::Table(t) => t.position,
    }
}

/// The rows of `rows` that pass the text filter and the kind chips.
pub fn filtered(
    rows: &[ObjectRow],
    filter: &str,
    kinds: &FilterSet<ObjectKind>,
    i18n: &I18n,
) -> Vec<ObjectRow> {
    let needle = i18n.fold_for_search(filter.trim());
    rows.iter()
        .filter(|r| kinds.matches(r.kind) && r.matches(&needle, i18n))
        .cloned()
        .collect()
}

/// How many rows of each kind pass the text filter (the chips' counts; the
/// kind chips themselves do not narrow them).
pub fn kind_counts(rows: &[ObjectRow], filter: &str, i18n: &I18n) -> KindCounts {
    let needle = i18n.fold_for_search(filter.trim());
    let mut counts = KindCounts::default();
    for row in rows.iter().filter(|r| r.matches(&needle, i18n)) {
        counts.0[row.kind.index()] += 1;
    }
    counts
}

/// The panel's own state, kept in `UiState`: what the user picked, plus the
/// rows of the current scope and what they were read from.
#[derive(Debug, Clone, Default)]
pub struct ObjectListState {
    pub filter: String,
    pub scope: ObjectScope,
    /// The kind chips that are on; none on is All.
    pub kinds: FilterSet<ObjectKind>,
    /// More ▾ is open, showing the other kinds' chips.
    pub more_open: bool,
    /// How many rows are drawn; 0 reads as one page.
    pub shown: usize,
    /// The main window's rows.
    pub source: ObjectRowsSource,
    /// Each undocked document window's rows: its panels list the sheet it
    /// shows (#2630). The picks above are shared.
    pub windows: HashMap<iced::window::Id, ObjectRowsSource>,
}

impl ObjectListState {
    /// How many rows to draw.
    pub fn row_limit(&self) -> usize {
        self.shown.max(ROW_PAGE)
    }

    /// The rows of the main window (`None`) or of an undocked window.
    pub fn source_of(&self, window: Option<iced::window::Id>) -> Option<&ObjectRowsSource> {
        match window {
            None => Some(&self.source),
            Some(window) => self.windows.get(&window),
        }
    }

    /// [`Self::source_of`], made on first use for a window.
    pub fn source_of_mut(&mut self, window: Option<iced::window::Id>) -> &mut ObjectRowsSource {
        match window {
            None => &mut self.source,
            Some(window) => self.windows.entry(window).or_default(),
        }
    }

    /// A stamp of what the user picked.
    pub fn picks_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.filter.hash(&mut h);
        self.scope.hash(&mut h);
        for kind in self.kinds.iter() {
            kind.hash(&mut h);
        }
        (self.more_open, self.shown).hash(&mut h);
        h.finish()
    }
}

/// The rows of the current scope, unfiltered, and the stamp of the documents
/// they were read from.
#[derive(Debug, Clone, Default)]
pub struct ObjectRowsSource {
    /// Stamp of the documents and scope the rows were read for.
    pub stamp: Option<u64>,
    pub rows: Arc<Vec<ObjectRow>>,
    /// The newest project read; an older one landing late is dropped.
    pub generation: u64,
    /// A project read is on its way; the last rows stay listed meanwhile.
    pub reading: bool,
    /// The last project read failed. Its stamp is kept, so it is read again
    /// only when the documents or the scope change, not on every update.
    pub failed: bool,
    /// Stamp of everything the panel view was last built from.
    pub view_stamp: Option<u64>,
    /// What `${TOKEN}`s resolved to on the sheet in view when the rows were
    /// read; the canvas resolving them otherwise reads the rows again.
    pub special: Option<SpecialStrings>,
}

impl ObjectRowsSource {
    /// Forget the rows and what they were read from. The generation moves
    /// on, never back, so a read still on its way is dropped when it lands
    /// and a later read cannot share its number (#2630).
    pub fn reset(&mut self) {
        *self = Self {
            generation: self.generation.wrapping_add(1),
            ..Self::default()
        };
    }
}

/// What happened in the Object List panel.
#[derive(Debug, Clone)]
pub enum ObjectListMsg {
    FilterChanged(String),
    ScopePicked(ObjectScope),
    KindChipToggled(KindChip),
    /// More ▾ was pressed: show or hide the other kinds' chips.
    MoreToggled,
    /// The "… N more" row was pressed: draw another page of rows.
    MoreRowsShown,
    /// A row was clicked: select its object on the canvas.
    RowClicked(ObjectRef),
    /// A row was double-clicked: select its object and zoom to it.
    RowDoubleClicked(ObjectRef),
    /// The project's sheets were read for the Project scope of the main
    /// window (`window: None`) or of an undocked window; the error is a
    /// worker that died.
    ProjectRowsRead {
        window: Option<iced::window::Id>,
        generation: u64,
        rows: Result<Arc<Vec<ObjectRow>>, String>,
    },
}

/// Why the panel lists nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ObjectListStatus {
    /// No schematic in view (none open, or a board tab).
    #[default]
    NoSchematic,
    /// The project's sheets are being read; nothing to show yet.
    Reading,
    Ready,
}

/// What the panel shows; built when the documents, the scope, the picks or
/// the selection change, never per frame.
#[derive(Debug, Clone, Default)]
pub struct ObjectListView {
    pub filter: String,
    pub scope: ObjectScope,
    pub kinds: FilterSet<ObjectKind>,
    pub more_open: bool,
    pub status: ObjectListStatus,
    /// The rows the filter and the chips keep.
    pub rows: Arc<Vec<ObjectRow>>,
    /// How many of `rows` are drawn.
    pub row_limit: usize,
    pub counts: KindCounts,
    /// The sheet in view and the objects selected on it.
    pub active_sheet: Option<PathBuf>,
    pub selected: Arc<std::collections::HashSet<Uuid>>,
    /// Selected objects among the listed ones.
    pub selected_count: usize,
    /// A newer project read is on its way.
    pub reading: bool,
}

impl ObjectListView {
    /// The view of `source`'s rows by `state`'s picks: filtered, counted and
    /// marked against the canvas selection `selected` on `active_sheet`; the
    /// filter folds by `i18n`'s rules.
    pub fn build(
        state: &ObjectListState,
        source: &ObjectRowsSource,
        status: ObjectListStatus,
        active_sheet: Option<PathBuf>,
        selected: &[SelectedItem],
        i18n: &I18n,
    ) -> Self {
        let all = &source.rows;
        let mut view = Self {
            filter: state.filter.clone(),
            scope: state.scope,
            kinds: state.kinds.clone(),
            more_open: state.more_open,
            status,
            rows: Arc::new(filtered(all, &state.filter, &state.kinds, i18n)),
            row_limit: state.row_limit(),
            counts: kind_counts(all, &state.filter, i18n),
            active_sheet,
            selected: Arc::new(selected.iter().map(|s| s.uuid).collect()),
            selected_count: 0,
            reading: source.reading,
        };
        view.selected_count = view.rows.iter().filter(|r| view.is_selected(r)).count();
        view
    }

    /// Whether `row` is selected on the canvas.
    pub fn is_selected(&self, row: &ObjectRow) -> bool {
        self.active_sheet.as_deref() == Some(row.target.sheet.as_path())
            && self.selected.contains(&row.target.item.uuid)
    }
}
