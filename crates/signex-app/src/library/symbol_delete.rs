//! Who still uses a library symbol that is about to be deleted (#1845).
//!
//! The SCH Library panel's Delete asks first when a component row binds the
//! symbol or an open sheet has parts placed from it. The scan reads only
//! state already in memory (the cached rows of the open libraries and the
//! open sheets), so it is safe on the `update` path.

use std::path::{Path, PathBuf};

use signex_library::{PrimitiveRef, RowId};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::closed_sheets::ClosedSheetScan;
use super::drift::split_lib_id;
use super::state::{EditorAddress, OpenLibrary};
use crate::panels::{PanelMsg, SymbolDeleteWarning, SymbolUsageEntry, SymbolUsageKey};

/// Names listed per cell before the rest is counted or behind "Show all".
const MAX_LISTED: usize = 6;

/// A Delete waiting for the user's answer.
#[derive(Debug, Clone)]
pub struct PendingSymbolDelete {
    pub path: PathBuf,
    pub symbol: Uuid,
    /// The editor's `history_generation` when the warning was raised; any
    /// edit, undo or redo since then makes the warning stale.
    pub history: u64,
    /// Names the scan this warning started; a result carrying another serial
    /// belongs to a warning that was replaced or closed and is dropped.
    pub serial: u64,
    /// How many rows, open sheets and closed sheets the warning named.
    pub rows: usize,
    pub sheets: usize,
    pub closed: usize,
    /// Tables (or whole table lists) that could not be read.
    pub unchecked: usize,
    /// The rows that bind the symbol, in the order the warning lists them:
    /// what a Re-bind writes.
    pub affected: Vec<EditorAddress>,
    /// What the closed-sheet scan found, kept for a recount after a re-bind.
    pub closed_placed: Vec<String>,
    pub closed_unreadable: Vec<String>,
    /// A Re-bind was started from this warning: it stays up for the user's
    /// answer even once nothing uses the symbol (no delete without asking).
    pub rebound: bool,
    pub warning: SymbolDeleteWarning,
}

/// Who the warning is about, before any usage is counted.
pub struct PendingSubject {
    pub path: PathBuf,
    pub symbol: Uuid,
    pub history: u64,
    pub serial: u64,
    pub idx: usize,
    pub name: String,
}

impl PendingSymbolDelete {
    /// The warning for `subject` from the in-memory `usage`, with the closed
    /// sheets still to read when `checking`.
    pub fn new(subject: PendingSubject, usage: SymbolUsage, checking: bool) -> Self {
        let mut pending = Self {
            path: subject.path,
            symbol: subject.symbol,
            history: subject.history,
            serial: subject.serial,
            rows: 0,
            sheets: 0,
            closed: 0,
            unchecked: 0,
            affected: Vec::new(),
            closed_placed: Vec::new(),
            closed_unreadable: Vec::new(),
            rebound: false,
            warning: SymbolDeleteWarning {
                idx: subject.idx,
                name: subject.name,
                only_unknown: false,
                checking,
                entries: Vec::new(),
                rows: 0,
                show_all_rows: false,
            },
        };
        pending.recount(usage);
        pending
    }

    /// The warning still describes `editor`: same container, same history,
    /// and the symbol it names is still at its slot.
    pub fn is_current(&self, editor: &crate::app::SymbolDocument) -> bool {
        editor.path == self.path
            && editor.draft.history_generation == self.history
            && editor
                .draft
                .file
                .symbols
                .get(self.warning.idx)
                .is_some_and(|s| s.uuid == self.symbol)
    }

    /// Fold the closed-sheet scan into the warning, together with the
    /// in-memory usage read again now: rows and open sheets may have changed
    /// while the scan ran. Nothing found and nothing unchecked leaves
    /// `warning.entries` empty: the delete is then safe to run.
    pub fn apply_closed_scan(&mut self, usage: SymbolUsage, scan: &ClosedSheetScan) {
        self.closed_placed = scan.placed.clone();
        self.closed_unreadable = scan.unreadable.clone();
        self.warning.checking = false;
        self.recount(usage);
    }

    /// Rebuild the counts and the in-use box from `usage` and the closed
    /// sheets already read. A recount after a re-bind or a library reload
    /// keeps a running scan's `checking`.
    pub fn recount(&mut self, usage: SymbolUsage) {
        self.rows = usage.rows.len();
        self.sheets = usage.placed.len();
        self.closed = self.closed_placed.len();
        self.unchecked = usage.unchecked.len() + self.closed_unreadable.len();
        let (affected, labels): (Vec<_>, Vec<_>) = usage
            .rows
            .into_iter()
            .map(|row| (row.address, row.label))
            .unzip();
        self.affected = affected;
        let not_checked = usage
            .unchecked
            .into_iter()
            .chain(self.closed_unreadable.iter().cloned())
            .collect();
        let entries = [
            (SymbolUsageKey::ComponentRows, labels),
            (SymbolUsageKey::OpenSheets, usage.placed),
            (SymbolUsageKey::ClosedSheets, self.closed_placed.clone()),
            (SymbolUsageKey::NotChecked, not_checked),
        ];
        self.warning.entries = entries
            .into_iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(key, items)| SymbolUsageEntry { key, items })
            .collect();
        self.warning.rows = self.rows;
        self.warning.only_unknown = self.rows == 0 && self.sheets == 0 && self.closed == 0;
    }
}

/// A component row that binds the symbol.
#[derive(Debug, Clone)]
pub struct UsingRow {
    pub address: EditorAddress,
    /// `PN (library / table)`, as the warning lists it.
    pub label: String,
}

/// What the scan found for one symbol.
#[derive(Debug, Default)]
pub struct SymbolUsage {
    pub rows: Vec<UsingRow>,
    /// Open sheets with parts placed from the symbol, with their references.
    pub placed: Vec<String>,
    /// Unreadable tables, libraries whose table list failed, or a container
    /// whose rows cannot be checked at all.
    pub unchecked: Vec<String>,
}

impl SymbolUsage {
    /// Nothing uses the symbol and nothing was left unchecked.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.placed.is_empty() && self.unchecked.is_empty()
    }
}

/// Rows and open sheets that use `symbol`, plus an entry for every table of
/// a mounted library that could not be read: what binds into it is unknown.
pub fn usage_lines<'a>(
    libs: &[OpenLibrary],
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    symbol: &PrimitiveRef,
) -> SymbolUsage {
    let mut rows = Vec::new();
    let mut unchecked = Vec::new();
    for lib in libs {
        if lib.dangling.tables_not_listed {
            unchecked.push(format!(
                "Tables of {} could not be listed; its rows were not checked",
                lib.display_name
            ));
        }
        for table in &lib.dangling.unreadable_tables {
            unchecked.push(format!(
                "Table {table} ({}) could not be read; its rows were not checked",
                lib.display_name
            ));
        }
        let mut tables: Vec<_> = lib.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        for (table, list) in tables {
            for row in list.iter().filter(|r| r.binds_symbol(symbol)) {
                rows.push(UsingRow {
                    address: EditorAddress::new(
                        lib.root.clone(),
                        table.clone(),
                        RowId::from_uuid(row.row_id),
                    ),
                    label: format!(
                        "{} ({} / {})",
                        row.internal_pn.as_str(),
                        lib.display_name,
                        table
                    ),
                });
            }
        }
    }
    let target = (symbol.library_id, symbol.uuid);
    let placed = placed_on_sheets(sheets, |id| split_lib_id(id) == Some(target));
    SymbolUsage {
        rows,
        placed,
        unchecked,
    }
}

/// One entry per open sheet that has parts whose `lib_id` satisfies
/// `is_symbol`: the sheet file and the references placed from it.
pub(super) fn placed_on_sheets<'a>(
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    is_symbol: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut placed = Vec::new();
    for (path, sheet) in sheets {
        let refs: Vec<&str> = sheet
            .symbols
            .iter()
            .filter(|s| is_symbol(&s.lib_id))
            .map(|s| s.reference.as_str())
            .collect();
        if !refs.is_empty() {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            placed.push(format!("{name}: {}", refs.join(", ")));
        }
    }
    placed.sort();
    placed
}

/// The scan for a container whose library is unknown: it is outside every
/// mounted library, or (`shared_folder`) several share its folder. Its
/// component rows cannot be checked (usage is unknown), but open sheets are
/// still matched by the symbol's uuid, whatever library the part names.
pub fn usage_outside_library<'a>(
    container: &Path,
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    symbol: Uuid,
    shared_folder: bool,
) -> SymbolUsage {
    let placed = placed_on_sheets(sheets, |id| {
        split_lib_id(id).is_some_and(|(_, sym)| sym == symbol)
    });
    let why = if shared_folder {
        "its library cannot be told apart (several libraries share its folder)"
    } else {
        "it is not inside a mounted library"
    };
    SymbolUsage {
        rows: Vec::new(),
        placed,
        unchecked: vec![format!(
            "{}: {why}; its component rows were not checked",
            container.display()
        )],
    }
}

fn capped(items: &[String]) -> String {
    let mut text = items
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if items.len() > MAX_LISTED {
        text.push_str(&format!("; and {} more", items.len() - MAX_LISTED));
    }
    text
}

/// The "Delete Symbol?" confirm (mockup row 137): an amber warning intro,
/// what deleting does to the rows, the in-use details as key / value rows,
/// then Re-bind all to… on the left and Cancel and Delete anyway on the
/// right (#2572). The header's close runs the same Cancel as the footer.
pub fn view<'a>(
    warning: &'a SymbolDeleteWarning,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> iced::Element<'a, PanelMsg> {
    use super::modal_chrome::{SYMBOL_DELETE_W, cancel_btn, danger_btn, footer_strip, header};
    use crate::panels::SymbolDeleteRebind;
    use crate::styles::{
        DialogButtonKind, DialogIcon, Spacing, TypeScale, dialog_body, dialog_button, dialog_intro,
        ti,
    };
    use iced::widget::{Column, column, container};
    use iced::{Length, Widget as _};
    use signex_i18n::{ids, t};
    use signex_widgets::text;
    const SP: Spacing = Spacing::DEFAULT;
    const TS: TypeScale = TypeScale::DEFAULT;

    let cancel = PanelMsg::SchLibraryDeleteCancelled;
    let title = t!(i18n, ids::LIBRARY_SYMBOL_DELETE_TITLE).into_string();
    let intro = dialog_intro(
        DialogIcon::Warn,
        warning.heading(i18n).into_string(),
        Some(t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_WARNING_NOTE).into_string()),
        tokens,
    );
    let mut content: Column<iced::Element<'a, PanelMsg>> =
        column![intro].spacing(SP.m).width(Length::Fill);
    if warning.rows > 0 {
        content = content.push(
            text(t!(i18n, ids::LIBRARY_SYMBOL_DELETE_CONSEQUENCE, "count" => warning.rows))
                .size(TS.body)
                .color(ti(tokens.text))
                .boxed(),
        );
    }
    if !warning.entries.is_empty() || warning.checking {
        content = content.push(usage_box(warning, tokens, i18n));
    }
    let extras = if warning.rows > 0 {
        vec![dialog_button(
            t!(i18n, ids::LIBRARY_SYMBOL_DELETE_REBIND_ALL),
            DialogButtonKind::Secondary,
            Some(PanelMsg::SchLibraryDeleteRebind(
                SymbolDeleteRebind::AllRows,
            )),
            tokens,
        )]
    } else {
        Vec::new()
    };
    // Once a re-bind left nothing using the symbol, it is a plain Delete.
    let confirm = if warning.is_clear() {
        ids::PANELS_SCH_LIBRARY_DELETE
    } else {
        ids::PANELS_SCH_LIBRARY_DELETE_ANYWAY
    };
    let footer = footer_strip(
        extras,
        vec![
            cancel_btn(
                t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_CANCEL),
                cancel.clone(),
                tokens,
            ),
            danger_btn(
                t!(i18n, confirm),
                PanelMsg::SchLibraryDeleteConfirmed(warning.idx),
                tokens,
            ),
        ],
        tokens,
    );
    container(
        column![
            header(&title, cancel, false, tokens),
            dialog_body(content),
            footer
        ]
        .width(Length::Fixed(SYMBOL_DELETE_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The in-use details, one key / value row per kind of user; the closed
/// sheets row says they are being read while the scan runs.
fn usage_box<'a>(
    warning: &'a SymbolDeleteWarning,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> iced::Element<'a, PanelMsg> {
    use crate::styles::{TypeScale, ti};
    use iced::Widget as _;
    use signex_i18n::{ids, t};
    use signex_widgets::{raw, text};
    const TS: TypeScale = TypeScale::DEFAULT;

    let ink = ti(tokens.text);
    let mut rows: Vec<(signex_i18n::Localized, iced::Element<'a, PanelMsg>)> = Vec::new();
    for entry in &warning.entries {
        let value = match entry.key {
            SymbolUsageKey::ComponentRows => {
                rows_cell(&entry.items, warning.show_all_rows, tokens, i18n)
            }
            // raw: the names are worded in English by the delete check
            // above (#2060).
            _ => text(raw(capped(&entry.items)))
                .size(TS.body)
                .color(ink)
                .boxed(),
        };
        rows.push((key_label(entry.key, i18n), value));
    }
    if warning.checking {
        let at = warning
            .entries
            .iter()
            .take_while(|e| {
                matches!(
                    e.key,
                    SymbolUsageKey::ComponentRows | SymbolUsageKey::OpenSheets
                )
            })
            .count();
        let value = text(t!(i18n, ids::LIBRARY_SYMBOL_DELETE_CHECKING_CLOSED))
            .size(TS.body)
            .color(ink)
            .boxed();
        rows.insert(at, (key_label(SymbolUsageKey::ClosedSheets, i18n), value));
    }
    super::modal_chrome::key_value_box(rows, tokens)
}

fn key_label(key: SymbolUsageKey, i18n: &signex_i18n::I18n) -> signex_i18n::Localized {
    use signex_i18n::{ids, t};
    let id = match key {
        SymbolUsageKey::ComponentRows => ids::LIBRARY_SYMBOL_DELETE_KEY_ROWS,
        SymbolUsageKey::OpenSheets => ids::LIBRARY_SYMBOL_DELETE_KEY_OPEN_SHEETS,
        SymbolUsageKey::ClosedSheets => ids::LIBRARY_SYMBOL_DELETE_KEY_CLOSED_SHEETS,
        SymbolUsageKey::NotChecked => ids::LIBRARY_SYMBOL_DELETE_KEY_NOT_CHECKED,
    };
    t!(i18n, id)
}

/// Tallest the expanded row list grows before it scrolls.
const ROWS_MAX_H: f32 = 220.0;

/// The affected rows, one per line with its own Re-bind…: the first
/// [`MAX_LISTED`], then Show all, which lists every row in a scrolled list.
fn rows_cell<'a>(
    items: &'a [String],
    show_all: bool,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> iced::Element<'a, PanelMsg> {
    use crate::panels::SymbolDeleteRebind;
    use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button, ti};
    use iced::widget::{Column, container, row, scrollable};
    use iced::{Alignment, Length, Widget as _};
    use signex_i18n::{ids, t};
    use signex_widgets::text;
    const SP: Spacing = Spacing::DEFAULT;
    const TS: TypeScale = TypeScale::DEFAULT;

    let ink = ti(tokens.text);
    let shown = if show_all { items.len() } else { MAX_LISTED };
    let mut list: Column<iced::Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).width(Length::Fill);
    for (i, label) in items.iter().enumerate().take(shown) {
        list = list.push(
            row![
                // The row's part number, library and table: the user's names.
                text(signex_i18n::UserText::new(label.clone()))
                    .size(TS.body)
                    .color(ink)
                    .width(Length::Fill),
                dialog_button(
                    t!(i18n, ids::LIBRARY_SYMBOL_DELETE_REBIND_ROW),
                    DialogButtonKind::Link,
                    Some(PanelMsg::SchLibraryDeleteRebind(SymbolDeleteRebind::Row(i))),
                    tokens,
                ),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    if show_all {
        return container(scrollable(list))
            .max_height(ROWS_MAX_H)
            .width(Length::Fill)
            .boxed();
    }
    if items.len() > MAX_LISTED {
        list = list.push(dialog_button(
            t!(i18n, ids::LIBRARY_SYMBOL_DELETE_SHOW_ALL, "count" => items.len()),
            DialogButtonKind::Link,
            Some(PanelMsg::SchLibraryDeleteShowAllRows),
            tokens,
        ));
    }
    list.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_folder_is_not_called_unmounted() {
        let file = Path::new("/w/symbols/s.snxsym");
        let lines = |shared| {
            usage_outside_library(file, std::iter::empty(), Uuid::nil(), shared)
                .unchecked
                .join("\n")
        };
        assert!(lines(false).contains("is not inside a mounted library"));
        let shared = lines(true);
        assert!(
            shared.contains("several libraries share its folder"),
            "{shared}"
        );
        assert!(!shared.contains("not inside a mounted library"), "{shared}");
    }

    use crate::library::state::LibraryDisplaySettings;
    use crate::library::test_adapters::{FailingListingAdapter, FailingTableListAdapter};
    use std::collections::HashMap;

    fn library() -> OpenLibrary {
        OpenLibrary {
            root: PathBuf::from("/tmp/a.snxlib"),
            display_name: "Lib".into(),
            library_id: Uuid::now_v7(),
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            alternate_supply: Default::default(),
            own_edit_stamp: None,
            display: LibraryDisplaySettings::default(),
            provenance: Default::default(),
        }
    }

    fn scan(lib: &OpenLibrary) -> SymbolUsage {
        let symbol = PrimitiveRef::new(lib.library_id, Uuid::now_v7());
        usage_lines(std::slice::from_ref(lib), std::iter::empty(), &symbol)
    }

    #[test]
    fn a_library_whose_table_list_failed_is_unchecked_until_it_reloads() {
        let mut lib = library();
        assert!(
            lib.reload_tables(&FailingTableListAdapter::new(lib.library_id))
                .is_err()
        );

        let usage = scan(&lib);
        assert_eq!(usage.unchecked.len(), 1);
        assert!(
            usage.unchecked[0].contains("could not be listed"),
            "{:?}",
            usage.unchecked
        );

        lib.reload_tables(&FailingListingAdapter::new(lib.library_id))
            .unwrap();
        assert!(scan(&lib).is_empty(), "a good load clears the flag");
    }

    fn sheet_placing(lib_id: &str, reference: &str) -> SchematicSheet {
        let mut sheet = SchematicSheet::default();
        sheet.symbols.push(signex_types::schematic::Symbol {
            lib_id: lib_id.into(),
            reference: reference.into(),
            ..Default::default()
        });
        sheet
    }

    use crate::app::SymbolDocument;
    use signex_library::{Symbol, SymbolFile};

    fn subject_of(editor: &SymbolDocument) -> PendingSubject {
        PendingSubject {
            path: editor.path.clone(),
            symbol: editor.draft.file.symbols[0].uuid,
            history: editor.draft.history_generation,
            serial: 1,
            idx: 0,
            name: "A".into(),
        }
    }

    fn pending_for(editor: &SymbolDocument) -> PendingSymbolDelete {
        PendingSymbolDelete::new(subject_of(editor), SymbolUsage::default(), false)
    }

    /// A row of library `Lib`, table `Resistors`, labelled `pn`.
    fn using_row(pn: &str) -> UsingRow {
        UsingRow {
            address: EditorAddress::new(
                PathBuf::from("lib.snxlib"),
                "Resistors".into(),
                RowId::from_uuid(Uuid::now_v7()),
            ),
            label: format!("{pn} (Lib / Resistors)"),
        }
    }

    fn edit(editor: &mut SymbolDocument) {
        assert!(editor.record_edit(None, |e| {
            e.draft.file.symbols[0].name.push('x');
            true
        }));
    }

    fn editor() -> SymbolDocument {
        SymbolDocument::new(
            PathBuf::from("a.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("A")),
        )
    }

    #[test]
    fn undo_then_a_new_edit_leaves_the_warning_stale() {
        // Undo depth is 1 again after undo + edit; only the generation differs.
        let mut ed = editor();
        edit(&mut ed);
        let pending = pending_for(&ed);
        assert!(pending.is_current(&ed));

        assert!(ed.undo());
        assert!(!pending.is_current(&ed), "undo");
        edit(&mut ed);
        assert_eq!(ed.draft.history.undo_len(), 1);
        assert!(!pending.is_current(&ed), "undo then edit");
    }

    #[test]
    fn redo_leaves_the_warning_stale() {
        let mut ed = editor();
        edit(&mut ed);
        assert!(ed.undo());
        let pending = pending_for(&ed);
        assert!(pending.is_current(&ed));

        assert!(ed.redo());
        assert!(!pending.is_current(&ed));
    }

    #[test]
    fn a_reopened_editor_does_not_revive_the_warning() {
        let (mut app, _task) = crate::app::Signex::new();
        let path = PathBuf::from("a.snxsym");
        let file = SymbolFile::from_symbol(Symbol::empty("A"));

        app.open_symbol_editor_tab(path.clone(), file.clone());
        edit(app.document_state.symbol_editors.get_mut(&path).unwrap());
        let pending = pending_for(&app.document_state.symbol_editors[&path]);
        assert!(pending.is_current(&app.document_state.symbol_editors[&path]));

        // Close, reopen and make the same single edit.
        app.document_state.symbol_editors.remove(&path);
        app.document_state.tabs.clear();
        app.open_symbol_editor_tab(path.clone(), file);
        edit(app.document_state.symbol_editors.get_mut(&path).unwrap());
        assert!(!pending.is_current(&app.document_state.symbol_editors[&path]));
    }

    #[test]
    fn an_upper_case_lib_id_on_a_sheet_is_still_named() {
        let lib = library();
        let symbol = PrimitiveRef::new(lib.library_id, Uuid::now_v7());
        let upper = super::super::place::lib_id_of(&symbol).to_uppercase();
        let sheet = sheet_placing(&upper, "U7");
        let path = PathBuf::from("/tmp/a.snxsch");

        let usage = usage_lines(
            std::slice::from_ref(&lib),
            std::iter::once((path.as_path(), &sheet)),
            &symbol,
        );
        assert_eq!(usage.placed.len(), 1);
        assert!(usage.placed.join("\n").contains("U7"), "{:?}", usage.placed);

        let outside = usage_outside_library(
            Path::new("/tmp/x.snxsym"),
            std::iter::once((path.as_path(), &sheet)),
            symbol.uuid,
            false,
        );
        assert_eq!(outside.placed.len(), 1);
    }

    #[test]
    fn a_closed_scan_replaces_the_checking_line_and_counts_unchecked_sheets() {
        let mut pending =
            PendingSymbolDelete::new(subject_of(&editor()), SymbolUsage::default(), true);
        assert!(pending.warning.only_unknown && pending.warning.entries.is_empty());
        assert_eq!(heading(&pending), "Checking whether symbol \"A\" is in use");

        pending.apply_closed_scan(
            SymbolUsage::default(),
            &ClosedSheetScan {
                unreadable: vec!["x.snxsch could not be read".into()],
                ..Default::default()
            },
        );

        assert_eq!(pending.unchecked, 1);
        assert_eq!(
            pending.warning.entries,
            [SymbolUsageEntry {
                key: SymbolUsageKey::NotChecked,
                items: vec!["x.snxsch could not be read".into()],
            }]
        );
        assert!(pending.warning.only_unknown && !pending.warning.checking);
        assert_eq!(heading(&pending), "Symbol \"A\" may still be in use");

        pending.apply_closed_scan(
            SymbolUsage::default(),
            &ClosedSheetScan {
                placed: vec!["y.snxsch: U1".into()],
                ..Default::default()
            },
        );
        assert!(!pending.warning.only_unknown);
        assert_eq!(heading(&pending), "Symbol \"A\" is still in use");
    }

    const WINDOW: iced::Size = iced::Size::new(900.0, 700.0);

    /// A warning for `rows` rows of the symbol, placed on one open sheet.
    fn in_use(rows: usize) -> PendingSymbolDelete {
        let usage = SymbolUsage {
            rows: (0..rows).map(|i| using_row(&format!("R-{i}"))).collect(),
            placed: vec!["dcdc.snxsch: R1, R2".into()],
            unchecked: Vec::new(),
        };
        PendingSymbolDelete::new(subject_of(&editor()), usage, false)
    }

    fn texts_of(warning: &SymbolDeleteWarning) -> Vec<crate::layout_probe::LaidOutText> {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = signex_i18n::I18n::en_us();
        crate::layout_probe::lay_out_texts(view(warning, &tokens, &i18n), WINDOW)
    }

    fn shown(texts: &[crate::layout_probe::LaidOutText]) -> Vec<String> {
        texts
            .iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// What clicking the text that reads `needle` sends.
    fn click(warning: &SymbolDeleteWarning, needle: &str) -> Vec<PanelMsg> {
        let texts = texts_of(warning);
        let at = texts
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == needle)
            .unwrap_or_else(|| panic!("no {needle:?} in {:?}", shown(&texts)))
            .bounds
            .center();
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = signex_i18n::I18n::en_us();
        crate::layout_probe::messages_on_click_at(view(warning, &tokens, &i18n), WINDOW, None, at)
    }

    /// Mockup row 137: title, the warning intro, the consequence, the in-use
    /// key / value rows, then the footer, all inside the card's width, in
    /// both themes.
    #[test]
    fn the_modal_stacks_intro_consequence_box_and_footer_inside_its_width() {
        use crate::layout_probe::{LaidOutText, lay_out_texts};
        use signex_types::theme::{ThemeId, theme_tokens};
        let i18n = signex_i18n::I18n::en_us();
        let pending = in_use(1);
        let at = |texts: &[LaidOutText], needle: &str| {
            texts
                .iter()
                .find(|t| {
                    t.text
                        .replace(['\u{2068}', '\u{2069}'], "")
                        .contains(needle)
                })
                .unwrap_or_else(|| panic!("no {needle:?} in {texts:#?}"))
                .bounds
        };
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let texts = lay_out_texts(view(&pending.warning, &tokens, &i18n), WINDOW);
            let title = at(&texts, "Delete Symbol?");
            let heading = at(&texts, "Symbol \"A\" is still in use");
            let note = at(&texts, "the library symbol will be missing");
            let consequence = at(&texts, "1 component row will show a broken symbol binding.");
            let rows_key = at(&texts, "Component rows");
            let row = at(&texts, "R-0 (Lib / Resistors)");
            let sheets_key = at(&texts, "Open sheets");
            let sheets = at(&texts, "dcdc.snxsch: R1, R2");
            let rebind_all = at(&texts, "Re-bind all to…");
            let confirm = at(&texts, "Delete anyway");
            let cancel = at(&texts, "Cancel");
            assert!(title.y + title.height <= heading.y, "{id:?}");
            assert!(heading.y + heading.height <= note.y + 0.5, "{id:?}");
            assert!(note.y + note.height <= consequence.y, "{id:?}");
            assert!(consequence.y + consequence.height <= rows_key.y, "{id:?}");
            assert!(rows_key.x + rows_key.width <= row.x, "key left of value");
            assert!(
                sheets_key.x + sheets_key.width <= sheets.x,
                "key left of value"
            );
            assert!(row.y + row.height <= sheets.y + 0.5, "{id:?}");
            assert!(sheets.y + sheets.height <= confirm.y, "{id:?}");
            assert!(
                rebind_all.x + rebind_all.width <= cancel.x,
                "Re-bind all on the left"
            );
            assert!(
                cancel.x + cancel.width <= confirm.x,
                "Cancel before Delete anyway"
            );
            assert!(
                confirm.x + confirm.width <= super::super::modal_chrome::SYMBOL_DELETE_W,
                "{id:?}"
            );
        }
    }

    #[test]
    fn the_consequence_counts_the_rows_that_bind_the_symbol() {
        let shown_two = shown(&texts_of(&in_use(2).warning));
        assert!(
            shown_two
                .iter()
                .any(|t| t == "2 component rows will show a broken symbol binding."),
            "{shown_two:?}"
        );
        // Placed parts only: no row loses its binding, so no count and no Re-bind.
        let sheets_only = shown(&texts_of(&in_use(0).warning));
        assert!(
            !sheets_only
                .iter()
                .any(|t| t.contains("broken symbol binding") || t.starts_with("Re-bind")),
            "{sheets_only:?}"
        );
    }

    /// Decision 3 (#2572): the first six rows, each with Re-bind…, then Show
    /// all; Show all lists every row.
    #[test]
    fn the_rows_cell_lists_six_rows_then_show_all() {
        let mut pending = in_use(8);
        let texts = shown(&texts_of(&pending.warning));
        assert_eq!(
            texts.iter().filter(|t| *t == "Re-bind…").count(),
            6,
            "{texts:?}"
        );
        assert!(texts.iter().any(|t| t == "R-5 (Lib / Resistors)"));
        assert!(!texts.iter().any(|t| t == "R-6 (Lib / Resistors)"));
        assert!(matches!(
            click(&pending.warning, "Show all 8").as_slice(),
            [PanelMsg::SchLibraryDeleteShowAllRows]
        ));

        pending.warning.show_all_rows = true;
        let texts = shown(&texts_of(&pending.warning));
        assert_eq!(
            texts.iter().filter(|t| *t == "Re-bind…").count(),
            8,
            "{texts:?}"
        );
        assert!(texts.iter().any(|t| t == "R-7 (Lib / Resistors)"));
        assert!(!texts.iter().any(|t| t.starts_with("Show all")));
    }

    #[test]
    fn re_bind_buttons_name_all_rows_or_their_own_row() {
        use crate::panels::SymbolDeleteRebind;
        let pending = in_use(2);
        assert!(matches!(
            click(&pending.warning, "Re-bind all to…").as_slice(),
            [PanelMsg::SchLibraryDeleteRebind(
                SymbolDeleteRebind::AllRows
            )]
        ));
        assert!(matches!(
            click(&pending.warning, "Re-bind…").as_slice(),
            [PanelMsg::SchLibraryDeleteRebind(SymbolDeleteRebind::Row(0))]
        ));
    }

    /// Decision 2 (#2572): a recount after a re-bind updates the count; once
    /// nothing uses the symbol the confirm is a plain Delete.
    #[test]
    fn a_recount_with_no_users_left_offers_a_plain_delete() {
        let mut pending = in_use(2);
        pending.recount(SymbolUsage {
            rows: vec![using_row("R-1")],
            ..Default::default()
        });
        assert_eq!(
            (pending.rows, pending.sheets, pending.affected.len()),
            (1, 0, 1)
        );
        assert_eq!(
            pending.warning.items(SymbolUsageKey::ComponentRows),
            ["R-1 (Lib / Resistors)"]
        );
        assert!(pending.warning.items(SymbolUsageKey::OpenSheets).is_empty());

        pending.recount(SymbolUsage::default());
        assert!(pending.warning.is_clear());
        assert_eq!(heading(&pending), "Symbol \"A\" is no longer in use");
        let texts = shown(&texts_of(&pending.warning));
        assert!(texts.iter().any(|t| t == "Delete"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Delete anyway"), "{texts:?}");
    }

    /// A recount keeps what the closed-sheet scan found and a running scan.
    #[test]
    fn a_recount_keeps_the_closed_scan_and_its_checking_state() {
        let mut pending = in_use(1);
        pending.apply_closed_scan(
            SymbolUsage::default(),
            &ClosedSheetScan {
                placed: vec!["closed.snxsch: U9".into()],
                ..Default::default()
            },
        );
        pending.recount(SymbolUsage::default());
        assert_eq!(pending.closed, 1);
        assert_eq!(
            pending.warning.items(SymbolUsageKey::ClosedSheets),
            ["closed.snxsch: U9"]
        );

        let mut checking =
            PendingSymbolDelete::new(subject_of(&editor()), SymbolUsage::default(), true);
        checking.recount(SymbolUsage::default());
        assert!(checking.warning.checking && !checking.warning.is_clear());
        let texts = shown(&texts_of(&checking.warning));
        assert!(
            texts
                .iter()
                .any(|t| t == "Checking closed sheets of the open projects…"),
            "{texts:?}"
        );
    }

    /// The warning heading in English, without Fluent's isolation marks.
    fn heading(pending: &PendingSymbolDelete) -> String {
        pending
            .warning
            .heading(&signex_i18n::I18n::en_us())
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }
}
