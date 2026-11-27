//! `edit_symbol_pins` (#2817, mockup row 122b): the Pin table tab's bulk
//! edit, Delete selected and Paste rows for an agent or the CLI, on the open
//! `.snxsym` that `document` names. `pins` sets fields by pin number or
//! removes a pin; `paste` reads rows as Apply paste does, with `fields` as
//! the column map and `mode` as the paste mode. Exactly one of the two. One
//! undo step on the symbol file; `dry_run=true` answers with what the edit
//! would leave and changes nothing. Rules: docs/CLI.md, "Symbol pins".

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_library::{Symbol, SymbolPin};

use super::document_operands::named;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::{Message, Signex};
use crate::library::editor::symbol::{
    PartView, PasteMode, apply_paste, apply_rows, drop_pin_selection, electrical_id,
    orientation_id, parse_paste_fields, parse_paste_mode, parse_pin_rows, paste_preview,
    read_mapped, unescape_paste,
};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// Whether a pin length can be stored, by the Properties field's rule, as
/// the Pin table checks it.
fn pin_length_ok(value: f64) -> bool {
    crate::panels::NumericFieldKey::PinLength(0).accepts(value)
}

/// What the edit did, for the answer.
#[derive(Debug, Default)]
struct Edited {
    updated: usize,
    added: usize,
    /// The pins `pins` removed, by their index before the edit.
    removed: BTreeSet<usize>,
    /// The pins a `replace_all` paste replaced.
    replaced: usize,
    pasted: bool,
    /// Pasted lengths a pin cannot have; each such pin keeps its length.
    lengths_refused: usize,
}

/// The open symbol file `args` names.
fn named_symbol_file(app: &Signex, args: &CommandArgs) -> Result<PathBuf, String> {
    let entry = named(app, args)?;
    match entry.key {
        Some(DocumentKey::Symbol(path)) => Ok(path),
        Some(_) => Err(format!(
            "`edit_symbol_pins` acts on a symbol file (.snxsym), and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!("{} is not loaded; open it first", entry.name)),
    }
}

fn symbol_names(symbols: &[Symbol]) -> String {
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    names.join(", ")
}

/// The symbol `wanted` names in the file, or the file's only symbol.
fn symbol_index(symbols: &[Symbol], wanted: Option<&str>) -> Result<usize, String> {
    match wanted.map(str::trim) {
        Some(name) => symbols.iter().position(|s| s.name == name).ok_or_else(|| {
            format!(
                "the file has no symbol named `{name}` (it holds {})",
                symbol_names(symbols)
            )
        }),
        None if symbols.len() == 1 => Ok(0),
        None => Err(format!(
            "the file holds {} symbols; name one with `symbol` ({})",
            symbols.len(),
            symbol_names(symbols)
        )),
    }
}

/// Apply `args` to `sym`; on a refusal `sym` may be part-edited, so the
/// caller edits a copy.
fn edit(sym: &mut Symbol, args: &CommandArgs) -> Result<Edited, String> {
    match (text(args, "pins"), text(args, "paste")) {
        (Some(_), Some(_)) => Err("give `pins` or `paste`, not both".into()),
        (None, None) => {
            Err("needs `pins` (the fields to set by pin number) or `paste` (pin rows)".into())
        }
        (Some(pins), None) => {
            if text(args, "fields").is_some() || text(args, "mode").is_some() {
                return Err("`fields` and `mode` go with `paste`, not `pins`".into());
            }
            let rows = parse_pin_rows(pins, pin_length_ok)?;
            let applied = apply_rows(sym, &rows)?;
            Ok(Edited {
                updated: applied.updated,
                removed: applied.removed,
                ..Edited::default()
            })
        }
        (None, Some(paste)) => edit_by_paste(sym, args, &unescape_paste(paste)),
    }
}

/// Apply pasted rows as Apply paste does: new pins on the normal body of
/// part 1, a length a pin cannot have kept as the pin's own.
fn edit_by_paste(sym: &mut Symbol, args: &CommandArgs, pasted: &str) -> Result<Edited, String> {
    let columns = match text(args, "fields") {
        Some(fields) => parse_paste_fields(fields)?,
        None => Vec::new(),
    };
    let mode = match text(args, "mode") {
        Some(mode) => parse_paste_mode(mode)?,
        None => PasteMode::default(),
    };
    let rows = read_mapped(pasted, &columns);
    if rows.is_empty() {
        return Err("`paste` holds no row".into());
    }
    let numbers: Vec<&str> = sym.pins.iter().map(|p| p.number.as_str()).collect();
    let preview = paste_preview(&rows, mode, &numbers);
    let lengths_refused = rows
        .iter()
        .filter(|row| row.length.is_some_and(|l| !pin_length_ok(l)))
        .count();
    apply_paste(sym, &rows, mode, PartView::normal(1), pin_length_ok);
    Ok(Edited {
        updated: preview.updated,
        added: preview.added,
        replaced: preview.replaced,
        pasted: true,
        lengths_refused,
        ..Edited::default()
    })
}

fn pin_answer(pin: &SymbolPin) -> Value {
    json!({
        "number": pin.number,
        "name": pin.name,
        "electrical": electrical_id(pin.electrical),
        "part": pin.part_number,
        "length": pin.length,
        "orientation": orientation_id(pin.orientation),
        "hidden": pin.hidden,
        "description": pin.description,
    })
}

/// The numbers more than one pin of `sym` has, sorted.
fn duplicate_numbers(sym: &Symbol) -> Vec<&str> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for pin in &sym.pins {
        *counts.entry(pin.number.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(number, _)| number)
        .collect()
}

fn answer(path: &Path, sym: &Symbol, edited: &Edited, changed: bool, dry_run: bool) -> Value {
    json!({
        "document": path.display().to_string(),
        "symbol": sym.name,
        "changed": changed,
        "dry_run": dry_run,
        "updated": edited.updated,
        "added": edited.added,
        "removed": edited.removed.len(),
        "replaced": edited.replaced,
        "lengths_refused": edited.lengths_refused,
        "duplicate_numbers": duplicate_numbers(sym),
        "pins": sym.pins.iter().map(pin_answer).collect::<Vec<_>>(),
    })
}

/// `edit_symbol_pins document=<.snxsym> [symbol=<name>] pins=<json> |
/// paste=<rows> [fields=..] [mode=..] [dry_run=true]`: one undo step on the
/// symbol file; nothing when the symbol already reads so.
pub(super) fn edit_symbol_pins(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_symbol_file(app, args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    if let Some(reason) = app.editor_edit_refusal(&path) {
        return refused(reason);
    }
    let Some(editor) = app.document_state.symbol_editors.get(&path) else {
        return refused("the symbol file is not open");
    };
    let idx = match symbol_index(&editor.draft.file.symbols, text(args, "symbol")) {
        Ok(idx) => idx,
        Err(reason) => return refused(reason),
    };
    let before = &editor.draft.file.symbols[idx];
    let mut after = before.clone();
    let edited = match edit(&mut after, args) {
        Ok(edited) => edited,
        Err(reason) => return refused(reason),
    };
    let changed = after != *before;
    let dry_run = matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)));
    let answered = answer(&path, &after, &edited, changed, dry_run);
    if changed && !dry_run {
        commit(app, &path, idx, after, &edited);
    }
    (CommandOutcome::Answered(answered), Task::none())
}

/// Write `symbol` over the file's symbol `idx` as one undo step, then keep
/// the Pin table's ticks and the editor's selection on the pins they named,
/// as the tab's own Delete selected and Apply paste do.
fn commit(app: &mut Signex, path: &Path, idx: usize, symbol: Symbol, edited: &Edited) {
    let Some(editor) = app.document_state.symbol_editors.get_mut(path) else {
        return;
    };
    let shown = idx
        == editor
            .draft
            .active_idx
            .min(editor.draft.file.symbols.len().saturating_sub(1));
    let moves_pins = edited.pasted || !edited.removed.is_empty();
    editor.record_edit(None, |editor| {
        editor.draft.file.symbols[idx] = symbol;
        if shown && moves_pins {
            drop_pin_selection(&mut editor.gui.selected);
        }
        editor.mark_dirty();
        editor.gui.canvas_cache.clear();
        true
    });
    let pins = editor.primitive().pins.len();
    app.document_state.mark_document_dirty(path);
    if shown && let Some(props) = app.library.symbol_props.get_mut(path) {
        if edited.pasted {
            props.table.checked.clear();
        } else {
            props.table.forget_removed(&edited.removed);
        }
        props.table.keep_checks_below(pins);
    }
    app.refresh_panel_ctx();
}
