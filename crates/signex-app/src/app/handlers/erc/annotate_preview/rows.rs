//! The Annotate dialog's session model (#75, #76): the change-list view
//! (Changes / All and a filter), the sheet ticks and the reset scope. Pure
//! functions over the cached preview; the view only reads them.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use signex_types::schematic::Symbol;
use uuid::Uuid;

use super::{AnnotatePreviewCache, AnnotatePreviewEntry, PreviewSheet};

/// Which rows the change list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotateListView {
    /// The rows that change, then the locked rows the run keeps.
    #[default]
    Changes,
    /// Every row.
    All,
}

/// The Annotate dialog's choices for one session. Reset when the dialog
/// opens or closes, so a later run from a menu covers every sheet.
#[derive(Debug, Clone)]
pub struct AnnotateSession {
    pub list_view: AnnotateListView,
    /// Text the change list is filtered by (designators and sheet names).
    pub filter: String,
    /// Sheet files the user unticked. Every other project sheet is ticked.
    pub unticked: BTreeSet<PathBuf>,
    /// The "Fixed options" group is expanded.
    pub fixed_options_open: bool,
    /// The reset the confirm runs: `ResetOnly` (Reset All) or
    /// `ResetAndRenumber` (Reset & Renumber).
    pub reset_mode: signex_engine::AnnotateMode,
}

impl Default for AnnotateSession {
    fn default() -> Self {
        Self {
            list_view: AnnotateListView::default(),
            filter: String::new(),
            unticked: BTreeSet::new(),
            fixed_options_open: false,
            reset_mode: signex_engine::AnnotateMode::ResetAndRenumber,
        }
    }
}

impl AnnotateSession {
    /// Whether the reset the confirm runs also renumbers.
    pub(crate) fn resets_and_renumbers(&self) -> bool {
        matches!(
            self.reset_mode,
            signex_engine::AnnotateMode::ResetAndRenumber
        )
    }

    /// The reset confirm's title, in-window and on its own window: it
    /// follows the button pressed (#76).
    pub(crate) fn reset_title(&self) -> signex_i18n::MessageId {
        if self.resets_and_renumbers() {
            signex_i18n::ids::DIALOG_ANNOTATE_RESET_RENUMBER
        } else {
            signex_i18n::ids::DIALOG_TITLE_ANNOTATE_RESET
        }
    }

    /// The reset confirm's danger button.
    pub(crate) fn reset_action(&self) -> signex_i18n::MessageId {
        if self.resets_and_renumbers() {
            signex_i18n::ids::DIALOG_ANNOTATE_RESET_RENUMBER
        } else {
            signex_i18n::ids::DIALOG_ANNOTATE_RESET_ALL
        }
    }
}

/// Whether a row's designator changes.
pub(crate) fn is_changing(entry: &AnnotatePreviewEntry) -> bool {
    entry.current != entry.proposed
}

/// How many listed rows change: the count of the banner and of Accept.
pub(crate) fn change_count(entries: &[AnnotatePreviewEntry]) -> usize {
    entries.iter().filter(|e| is_changing(e)).count()
}

/// The sheet files the changing rows sit on, each once, in list order: the
/// banner names the one, or counts them. A never-saved sheet goes by its
/// title.
pub(crate) fn changing_sheets(entries: &[AnnotatePreviewEntry]) -> Vec<String> {
    let mut sheets: Vec<String> = Vec::new();
    for entry in entries.iter().filter(|e| is_changing(e)) {
        let name = entry
            .sheet_path
            .file_name()
            .map_or_else(|| entry.sheet.clone(), |n| n.to_string_lossy().into_owned());
        if !sheets.contains(&name) {
            sheets.push(name);
        }
    }
    sheets
}

/// The rows the change list draws, in list order.
#[derive(Debug, Default)]
pub(crate) struct ListedRows<'a> {
    pub rows: Vec<&'a AnnotatePreviewEntry>,
    /// Locked rows under the "Kept (locked)" subhead (Changes view only).
    pub kept_locked: Vec<&'a AnnotatePreviewEntry>,
}

impl ListedRows<'_> {
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.kept_locked.is_empty()
    }
}

/// Whether `entry` matches the filter text: a case-insensitive substring of
/// its current or proposed designator or its sheet. Blank matches all.
fn matches_filter(entry: &AnnotatePreviewEntry, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    [&entry.current, &entry.proposed, &entry.sheet]
        .iter()
        .any(|field| field.to_lowercase().contains(needle))
}

/// The change list's rows for `view` and `filter`. Changes lists the rows
/// that change, then the locked ones the run keeps; All lists every row.
pub(crate) fn listed_rows<'a>(
    entries: &'a [AnnotatePreviewEntry],
    locked: &HashSet<Uuid>,
    view: AnnotateListView,
    filter: &str,
) -> ListedRows<'a> {
    let needle = filter.trim().to_lowercase();
    let shown = entries.iter().filter(|e| matches_filter(e, &needle));
    match view {
        AnnotateListView::All => ListedRows {
            rows: shown.collect(),
            kept_locked: Vec::new(),
        },
        AnnotateListView::Changes => {
            let mut listed = ListedRows::default();
            for entry in shown {
                if is_changing(entry) {
                    listed.rows.push(entry);
                } else if locked.contains(&entry.uuid) {
                    listed.kept_locked.push(entry);
                }
            }
            listed
        }
    }
}

/// `locked` plus every symbol of an unticked sheet: such a sheet keeps its
/// designators, and they still hold their numbers in the counter.
pub(crate) fn with_unticked_locked<'s>(
    locked: &HashSet<Uuid>,
    unticked: &BTreeSet<PathBuf>,
    sheets: impl IntoIterator<Item = (&'s Path, &'s [Symbol])>,
) -> HashSet<Uuid> {
    let mut all = locked.clone();
    if unticked.is_empty() {
        return all;
    }
    for (path, symbols) in sheets {
        if unticked.contains(path) {
            all.extend(symbols.iter().map(|s| s.uuid));
        }
    }
    all
}

/// How a placement row names its placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlacementLabel {
    /// The name of the sheet symbol that places it.
    Named(String),
    /// Its ordinal (1-based) when the sheet symbol has no name.
    Ordinal(usize),
}

/// One row of the sheets table: a sheet file, or one placement of a file
/// the project places more than once (#1835).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SheetTickRow {
    pub path: PathBuf,
    /// The file name ("power.snxsch").
    pub name: String,
    pub placement: Option<PlacementLabel>,
    pub ticked: bool,
    /// The sheet the active tab shows.
    pub is_current: bool,
    /// The first number of its page block under a page numbering scheme
    /// (#1292); `None` numbers on from the project, shown as 1.
    pub start: Option<u32>,
}

/// The tick of the sheets table's header box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickState {
    All,
    Some,
    None,
}

/// The header tick over `rows`: an empty table is not ticked.
pub(crate) fn tick_state(rows: &[SheetTickRow]) -> TickState {
    let ticked = rows.iter().filter(|r| r.ticked).count();
    match ticked {
        0 => TickState::None,
        n if n == rows.len() => TickState::All,
        _ => TickState::Some,
    }
}

fn file_name(sheet: &PreviewSheet) -> String {
    sheet
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| sheet.title.clone())
}

/// Sheet-symbol uuid -> its name, over every loaded sheet.
fn sheet_symbol_names(sheets: &[PreviewSheet]) -> HashMap<Uuid, String> {
    sheets
        .iter()
        .flat_map(|s| s.sheet.child_sheets.iter())
        .filter(|child| !child.name.trim().is_empty())
        .map(|child| (child.uuid, child.name.clone()))
        .collect()
}

/// The reset confirm's scope: ticked sheet files, and the symbols on them a
/// reset renumbers (locked ones keep theirs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResetScope {
    pub sheets: usize,
    pub symbols: usize,
}

impl AnnotatePreviewCache {
    /// Every project sheet of the loaded preview as table rows, a file the
    /// project places more than once as one row per placement. Rows are the
    /// numbering units in walk order, so row n shows unit n's start.
    pub(crate) fn sheet_rows(
        &self,
        unticked: &BTreeSet<PathBuf>,
        active: Option<&Path>,
    ) -> Vec<SheetTickRow> {
        let names = sheet_symbol_names(&self.sheets.sheets);
        let mut rows: Vec<SheetTickRow> = Vec::new();
        for sheet in &self.sheets.sheets {
            let row = |placement: Option<PlacementLabel>, unit: usize| SheetTickRow {
                path: sheet.path.clone(),
                name: file_name(sheet),
                placement,
                ticked: !unticked.contains(&sheet.path),
                is_current: active.map_or(sheet.path.as_os_str().is_empty(), |a| {
                    a == sheet.path.as_path()
                }),
                start: self.starts.get(unit).copied().flatten(),
            };
            let places = self
                .sheets
                .multi
                .get(&sheet.path)
                .filter(|places| places.len() > 1);
            let Some(places) = places else {
                let unit = rows.len();
                rows.push(row(None, unit));
                continue;
            };
            for (n, place) in places.iter().enumerate() {
                let label = place
                    .chain()
                    .last()
                    .and_then(|uuid| names.get(uuid))
                    .map_or(PlacementLabel::Ordinal(n + 1), |name| {
                        PlacementLabel::Named(name.clone())
                    });
                let unit = rows.len();
                rows.push(row(Some(label), unit));
            }
        }
        rows
    }

    /// Every sheet file of the loaded preview.
    pub(crate) fn sheet_paths(&self) -> Vec<PathBuf> {
        self.sheets.sheets.iter().map(|s| s.path.clone()).collect()
    }

    /// What a reset of the ticked sheets covers; `None` until the preview
    /// has loaded.
    pub(crate) fn reset_scope(
        &self,
        unticked: &BTreeSet<PathBuf>,
        locked: &HashSet<Uuid>,
    ) -> Option<ResetScope> {
        if self.loading || self.fingerprint.is_none() {
            return None;
        }
        let sheets = (self.sheets.sheets.iter())
            .filter(|s| !unticked.contains(&s.path))
            .count();
        let symbols = (self.entries.iter())
            .filter(|e| !locked.contains(&e.uuid))
            .count();
        Some(ResetScope { sheets, symbols })
    }
}

#[cfg(test)]
mod tests {
    use super::super::PreviewSheets;
    use super::*;
    use signex_types::schematic::{ChildSheet, FillType, InstancePath, Point, SchematicSheet};
    use std::sync::Arc;

    fn entry(current: &str, proposed: &str, sheet: &str) -> AnnotatePreviewEntry {
        AnnotatePreviewEntry {
            sheet: sheet.to_string(),
            sheet_path: PathBuf::from(format!("{sheet}.snxsch")),
            current: current.to_string(),
            proposed: proposed.to_string(),
            uuid: Uuid::now_v7(),
            location: (0.0, 0.0),
        }
    }

    fn currents(rows: &[&AnnotatePreviewEntry]) -> Vec<String> {
        rows.iter().map(|e| e.current.clone()).collect()
    }

    #[test]
    fn changes_lists_the_changing_rows_then_the_locked_ones_kept() {
        let entries = vec![
            entry("R?", "R12", "power"),
            entry("U1", "U1", "power"),
            entry("R1", "R1", "power"),
            entry("C?", "C21", "mcu"),
        ];
        let locked: HashSet<Uuid> = [entries[1].uuid].into();
        let listed = listed_rows(&entries, &locked, AnnotateListView::Changes, "");
        assert_eq!(currents(&listed.rows), ["R?", "C?"]);
        assert_eq!(currents(&listed.kept_locked), ["U1"]);
        assert_eq!(change_count(&entries), 2);
    }

    #[test]
    fn the_banner_names_the_sheets_that_change_each_once() {
        let entries = vec![
            entry("R?", "R12", "power"),
            entry("U1", "U1", "mcu"),
            entry("C?", "C21", "power"),
        ];
        assert_eq!(changing_sheets(&entries), ["power.snxsch"]);
        let mut unsaved = entry("L?", "L1", "Untitled");
        unsaved.sheet_path = PathBuf::new();
        assert_eq!(
            changing_sheets(&[entries[0].clone(), unsaved]),
            ["power.snxsch", "Untitled"]
        );
    }

    #[test]
    fn all_lists_every_row_with_no_kept_group() {
        let entries = vec![entry("R?", "R12", "power"), entry("U1", "U1", "power")];
        let locked: HashSet<Uuid> = [entries[1].uuid].into();
        let listed = listed_rows(&entries, &locked, AnnotateListView::All, "");
        assert_eq!(currents(&listed.rows), ["R?", "U1"]);
        assert!(listed.kept_locked.is_empty());
    }

    #[test]
    fn the_filter_matches_designators_and_sheets_ignoring_case() {
        let entries = vec![
            entry("R?", "R12", "power"),
            entry("C?", "C21", "mcu"),
            entry("R1", "R1", "mcu"),
        ];
        let none = HashSet::new();
        let by_new = listed_rows(&entries, &none, AnnotateListView::Changes, " r12 ");
        assert_eq!(currents(&by_new.rows), ["R?"]);
        let by_sheet = listed_rows(&entries, &none, AnnotateListView::All, "MCU");
        assert_eq!(currents(&by_sheet.rows), ["C?", "R1"]);
        let nothing = listed_rows(&entries, &none, AnnotateListView::All, "zz");
        assert!(nothing.is_empty());
    }

    fn symbol() -> Symbol {
        Symbol {
            uuid: Uuid::now_v7(),
            reference: "R?".to_string(),
            position: Point::new(0.0, 0.0),
            ..Default::default()
        }
    }

    #[test]
    fn an_unticked_sheet_locks_its_symbols_and_nothing_else() {
        let (a, b) = (vec![symbol()], vec![symbol(), symbol()]);
        let (pa, pb) = (PathBuf::from("a.snxsch"), PathBuf::from("b.snxsch"));
        let own = HashSet::from([a[0].uuid]);
        let unticked = BTreeSet::from([pb.clone()]);
        let all = with_unticked_locked(
            &own,
            &unticked,
            [(pa.as_path(), a.as_slice()), (pb.as_path(), b.as_slice())],
        );
        assert_eq!(all, HashSet::from([a[0].uuid, b[0].uuid, b[1].uuid]));
        let none = with_unticked_locked(&own, &BTreeSet::new(), [(pb.as_path(), b.as_slice())]);
        assert_eq!(none, own);
    }

    fn child(uuid: Uuid, name: &str) -> ChildSheet {
        ChildSheet {
            uuid,
            name: name.to_string(),
            filename: "sensors.snxsch".to_string(),
            position: Point::new(0.0, 0.0),
            size: (0.0, 0.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn preview(path: &str, sheet: SchematicSheet) -> PreviewSheet {
        PreviewSheet {
            title: path.trim_end_matches(".snxsch").to_string(),
            path: PathBuf::from(path),
            sheet,
        }
    }

    /// `top` places `sensors` twice (U_S1 and an unnamed one); `power` once.
    fn cache() -> (AnnotatePreviewCache, PathBuf) {
        let (s1, s2) = (Uuid::now_v7(), Uuid::now_v7());
        let top = SchematicSheet {
            child_sheets: vec![child(s1, "U_S1"), child(s2, "")],
            ..Default::default()
        };
        let sensors = PathBuf::from("sensors.snxsch");
        let multi = HashMap::from([(
            sensors.clone(),
            vec![
                InstancePath::from_chain(vec![s1]),
                InstancePath::from_chain(vec![s2]),
            ],
        )]);
        let sheets = PreviewSheets {
            sheets: vec![
                preview("top.snxsch", top),
                preview("power.snxsch", SchematicSheet::default()),
                preview("sensors.snxsch", SchematicSheet::default()),
            ],
            multi,
            ..Default::default()
        };
        let cache = AnnotatePreviewCache {
            fingerprint: Some(1),
            sheets: Arc::new(sheets),
            ..Default::default()
        };
        (cache, sensors)
    }

    #[test]
    fn every_project_sheet_is_a_row_and_each_placement_its_own() {
        let (cache, sensors) = cache();
        let active = PathBuf::from("power.snxsch");
        let rows = cache.sheet_rows(&BTreeSet::new(), Some(&active));
        let names: Vec<(&str, Option<PlacementLabel>)> = rows
            .iter()
            .map(|r| (r.name.as_str(), r.placement.clone()))
            .collect();
        assert_eq!(
            names,
            [
                ("top.snxsch", None),
                ("power.snxsch", None),
                (
                    "sensors.snxsch",
                    Some(PlacementLabel::Named("U_S1".to_string()))
                ),
                ("sensors.snxsch", Some(PlacementLabel::Ordinal(2))),
            ]
        );
        assert!(rows.iter().all(|r| r.ticked));
        assert_eq!(tick_state(&rows), TickState::All);
        assert_eq!(
            rows.iter().filter(|r| r.is_current).count(),
            1,
            "only the active sheet is current"
        );
        assert!(rows[1].is_current);

        let unticked = BTreeSet::from([sensors]);
        let rows = cache.sheet_rows(&unticked, Some(&active));
        assert!(
            !rows[2].ticked && !rows[3].ticked,
            "placements tick together"
        );
        assert_eq!(tick_state(&rows), TickState::Some);
        assert_eq!(tick_state(&[]), TickState::None);
    }

    #[test]
    fn the_reset_scope_counts_ticked_sheets_and_unlocked_symbols() {
        let (mut cache, sensors) = cache();
        cache.entries = vec![
            entry("R?", "R1", "top"),
            entry("U1", "U1", "power"),
            entry("C?", "C1", "power"),
        ];
        let locked = HashSet::from([cache.entries[1].uuid]);
        let unticked = BTreeSet::from([sensors]);
        assert_eq!(
            cache.reset_scope(&unticked, &locked),
            Some(ResetScope {
                sheets: 2,
                symbols: 2
            })
        );
        cache.loading = true;
        assert_eq!(
            cache.reset_scope(&unticked, &locked),
            None,
            "not loaded yet"
        );
    }

    #[test]
    fn the_reset_confirm_is_titled_after_the_reset_asked_for() {
        use signex_i18n::ids;
        let mut session = AnnotateSession {
            reset_mode: signex_engine::AnnotateMode::ResetOnly,
            ..Default::default()
        };
        assert!(!session.resets_and_renumbers());
        assert_eq!(session.reset_title(), ids::DIALOG_TITLE_ANNOTATE_RESET);
        assert_eq!(session.reset_action(), ids::DIALOG_ANNOTATE_RESET_ALL);
        session.reset_mode = signex_engine::AnnotateMode::ResetAndRenumber;
        assert_eq!(session.reset_title(), ids::DIALOG_ANNOTATE_RESET_RENUMBER);
        assert_eq!(session.reset_action(), ids::DIALOG_ANNOTATE_RESET_RENUMBER);
    }
}
