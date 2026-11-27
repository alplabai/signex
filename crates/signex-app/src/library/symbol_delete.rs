//! Who still uses a library symbol that is about to be deleted (#1845).
//!
//! The SCH Library panel's Delete asks first when a component row binds the
//! symbol or an open sheet has parts placed from it. The scan reads only
//! state already in memory (the cached rows of the open libraries and the
//! open sheets), so it is safe on the `update` path.

use std::path::{Path, PathBuf};

use signex_library::PrimitiveRef;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::closed_sheets::ClosedSheetScan;
use super::drift::split_lib_id;
use super::state::OpenLibrary;
use crate::panels::{PanelMsg, SymbolDeleteWarning};

/// Names listed per group before the rest is counted.
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
    /// Warning lines before the closed-sheet scan added its own.
    pub base_lines: usize,
    /// Tables (or whole table lists) that could not be read.
    pub unchecked: usize,
    pub warning: SymbolDeleteWarning,
}

impl PendingSymbolDelete {
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
    /// `warning.lines` empty: the delete is then safe to run.
    pub fn apply_closed_scan(&mut self, usage: SymbolUsage, scan: &ClosedSheetScan) {
        self.rows = usage.rows;
        self.sheets = usage.sheets;
        self.base_lines = usage.lines.len();
        self.warning.lines = usage.lines;
        if !scan.placed.is_empty() {
            self.warning.lines.push(format!(
                "Closed sheets with placed parts: {}",
                capped(&scan.placed)
            ));
        }
        if !scan.unreadable.is_empty() {
            self.warning.lines.push(format!(
                "Closed sheets not checked: {}",
                capped(&scan.unreadable)
            ));
        }
        self.closed = scan.placed.len();
        self.unchecked = usage.unchecked + scan.unreadable.len();
        self.warning.only_unknown = self.rows == 0 && self.sheets == 0 && self.closed == 0;
        self.warning.checking = false;
    }
}

/// What the scan found for one symbol.
#[derive(Debug, Default)]
pub struct SymbolUsage {
    /// One line per kind of finding (rows, open sheets, unreadable tables);
    /// empty when nothing uses the symbol and nothing was left unchecked.
    pub lines: Vec<String>,
    pub rows: usize,
    pub sheets: usize,
    /// Unreadable tables plus libraries whose table list failed.
    pub unchecked: usize,
}

/// Rows and open sheets that use `symbol`, plus a line for every table of a
/// mounted library that could not be read: what binds into it is unknown.
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
                rows.push(format!(
                    "{} ({} / {})",
                    row.internal_pn.as_str(),
                    lib.display_name,
                    table
                ));
            }
        }
    }
    let target = (symbol.library_id, symbol.uuid);
    let placed = placed_on_sheets(sheets, |id| split_lib_id(id) == Some(target));
    let mut lines = Vec::new();
    if !rows.is_empty() {
        lines.push(format!("Component rows: {}", capped(&rows)));
    }
    if !placed.is_empty() {
        lines.push(format!(
            "Open sheets with placed parts: {}",
            capped(&placed)
        ));
    }
    let unchecked_count = unchecked.len();
    lines.extend(unchecked);
    SymbolUsage {
        lines,
        rows: rows.len(),
        sheets: placed.len(),
        unchecked: unchecked_count,
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
    let mut lines = vec![format!(
        "{}: {why}; its component rows were not checked",
        container.display()
    )];
    if !placed.is_empty() {
        lines.push(format!(
            "Open sheets with placed parts: {}",
            capped(&placed)
        ));
    }
    SymbolUsage {
        lines,
        rows: 0,
        sheets: placed.len(),
        unchecked: 1,
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
/// the in-use lines in a box, then Cancel and Delete anyway. The header's
/// close runs the same Cancel as the footer.
pub fn view<'a>(
    warning: &'a SymbolDeleteWarning,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> iced::Element<'a, PanelMsg> {
    use super::modal_chrome::{SYMBOL_DELETE_W, cancel_btn, danger_btn, footer_strip, header};
    use crate::styles::{DialogIcon, Spacing, dialog_body, dialog_intro};
    use iced::widget::{Column, column, container};
    use iced::{Length, Widget as _};
    use signex_i18n::{ids, t};
    const SP: Spacing = Spacing::DEFAULT;

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
    if !warning.lines.is_empty() {
        content = content.push(usage_box(&warning.lines, tokens));
    }
    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_CANCEL),
                cancel.clone(),
                tokens,
            ),
            danger_btn(
                t!(i18n, ids::PANELS_SCH_LIBRARY_DELETE_ANYWAY),
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

/// The in-use lines, one per kind of user, in a bordered box.
fn usage_box<'a>(
    lines: &'a [String],
    tokens: &signex_types::theme::ThemeTokens,
) -> iced::Element<'a, PanelMsg> {
    use crate::styles::{HAIRLINE, Radii, Spacing, TypeScale, ti};
    use iced::widget::{Column, container};
    use iced::{Border, Length, Theme, Widget as _};
    use signex_widgets::{raw, text};
    const SP: Spacing = Spacing::DEFAULT;
    const TS: TypeScale = TypeScale::DEFAULT;

    let ink = ti(tokens.text);
    let line = ti(tokens.border);
    let mut list: Column<iced::Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).width(Length::Fill);
    for entry in lines {
        // raw: the in-use lines are worded in English by the delete check
        // above (#2060).
        list = list.push(text(raw(entry.as_str())).size(TS.body).color(ink).boxed());
    }
    container(list)
        .padding(SP.m)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_folder_is_not_called_unmounted() {
        let file = Path::new("/w/symbols/s.snxsym");
        let lines = |shared| {
            usage_outside_library(file, std::iter::empty(), Uuid::nil(), shared)
                .lines
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
            own_edit_stamp: None,
            display: LibraryDisplaySettings::default(),
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
        assert_eq!(usage.unchecked, 1);
        assert!(
            usage.lines[0].contains("could not be listed"),
            "{:?}",
            usage.lines
        );

        lib.reload_tables(&FailingListingAdapter::new(lib.library_id))
            .unwrap();
        assert!(scan(&lib).lines.is_empty(), "a good load clears the flag");
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

    fn pending_for(editor: &SymbolDocument) -> PendingSymbolDelete {
        PendingSymbolDelete {
            path: editor.path.clone(),
            symbol: editor.draft.file.symbols[0].uuid,
            history: editor.draft.history_generation,
            serial: 1,
            rows: 1,
            sheets: 0,
            closed: 0,
            base_lines: 0,
            unchecked: 0,
            warning: SymbolDeleteWarning {
                idx: 0,
                name: "A".into(),
                only_unknown: false,
                checking: false,
                lines: Vec::new(),
            },
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
        assert_eq!(usage.sheets, 1);
        assert!(usage.lines.join("\n").contains("U7"), "{:?}", usage.lines);

        let outside = usage_outside_library(
            Path::new("/tmp/x.snxsym"),
            std::iter::once((path.as_path(), &sheet)),
            symbol.uuid,
            false,
        );
        assert_eq!(outside.sheets, 1);
    }

    #[test]
    fn a_closed_scan_replaces_the_checking_line_and_counts_unchecked_sheets() {
        let mut pending = pending_for(&editor());
        pending.rows = 0;
        pending.base_lines = 0;
        pending.warning.only_unknown = true;
        pending.warning.checking = true;
        pending.warning.lines = vec!["Checking closed sheets of the open projects…".into()];
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
            pending.warning.lines,
            ["Closed sheets not checked: x.snxsch could not be read"]
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

    /// Mockup row 137: title, then the warning intro, then the in-use lines,
    /// then the footer, all inside the card's width, in both themes.
    #[test]
    fn the_modal_stacks_intro_lines_and_footer_inside_its_width() {
        use crate::layout_probe::{LaidOutText, lay_out_texts};
        use signex_types::theme::{ThemeId, theme_tokens};
        let i18n = signex_i18n::I18n::en_us();
        let mut pending = pending_for(&editor());
        pending.warning.lines = vec![
            "Component rows: R-0402 (Lib / Resistors)".into(),
            "Open sheets with placed parts: dcdc.snxsch: R1, R2".into(),
        ];
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
            let texts = lay_out_texts(
                view(&pending.warning, &tokens, &i18n),
                iced::Size::new(900.0, 700.0),
            );
            let title = at(&texts, "Delete Symbol?");
            let heading = at(&texts, "Symbol \"A\" is still in use");
            let note = at(&texts, "the library symbol will be missing");
            let rows = at(&texts, "Component rows:");
            let sheets = at(&texts, "Open sheets with placed parts:");
            let confirm = at(&texts, "Delete anyway");
            let cancel = at(&texts, "Cancel");
            assert!(title.y + title.height <= heading.y, "{id:?}");
            assert!(heading.y + heading.height <= note.y + 0.5, "{id:?}");
            assert!(note.y + note.height <= rows.y, "{id:?}");
            assert!(rows.y + rows.height <= sheets.y + 0.5, "{id:?}");
            assert!(sheets.y + sheets.height <= confirm.y, "{id:?}");
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

    /// The warning heading in English, without Fluent's isolation marks.
    fn heading(pending: &PendingSymbolDelete) -> String {
        pending
            .warning
            .heading(&signex_i18n::I18n::en_us())
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }
}
