//! #2937: a part's footprint set, and a board footprint swapped, from the
//! command line or a running Signex, and the footprint of every part read
//! back.
//!
//! `footprint=<library>:<footprint>` names a footprint of an open library:
//! the library by id, path or name, the footprint by id or name. Its pads
//! are read off the UI thread first (`edit_reads`), then the edit runs on
//! them through its GUI path: `assign_footprint` through Assign Footprints'
//! Apply (the part's own library row must offer the footprint), and
//! `change_footprint` through the footprint picker's swap, which keeps each
//! footprint's position, rotation, side and pad nets. A footprint with no
//! pad for some pin is refused with the pins, and nothing changes. Each
//! edit is one undo step and answers with the parts it changed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{ids, t};
use signex_library::PrimitiveRef;
use signex_types::pcb::{Footprint, PadType};
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::document_operands::named_sheet;
use super::footprint_place_operands::{footprint_answer, footprint_refused, named_footprints};
use super::queries::pin_id;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::assign_footprints::{PinMap, pin_map};
use crate::app::document::DocumentKey;
use crate::app::handlers::pcb_component::SwapRefusal;
use crate::app::{Message, Signex};
use crate::library::place::lib_id_of;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (footprint_refused(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The copper pad numbers of `footprint`, each once.
fn pad_numbers(footprint: &Footprint) -> Vec<String> {
    let mut numbers: Vec<String> = Vec::new();
    for pad in &footprint.pads {
        let number = pad.number.trim();
        if pad.pad_type == PadType::NpThru {
            continue;
        }
        if !number.is_empty() && !numbers.iter().any(|n| n == number) {
            numbers.push(number.to_string());
        }
    }
    numbers
}

/// The pin ids of every unit of the part `symbol` places on `sheet`.
fn pin_numbers(sheet: &SchematicSheet, symbol: &Symbol) -> Vec<String> {
    sheet
        .resolve_lib_symbol(symbol)
        .map(|lib| {
            lib.pins
                .iter()
                .map(|lp| pin_id(&lp.pin).trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

impl Signex {
    /// The footprint `footprint=<library>:<footprint>` names in an open
    /// library: the library by id, path or name, the footprint by id or
    /// name, found in the library's own listing.
    pub(super) fn footprint_operand(&self, args: &CommandArgs) -> Result<PrimitiveRef, String> {
        let named = text(args, "footprint")
            .ok_or("needs the operand `footprint`, written <library>:<footprint>")?;
        let (library, footprint) = named
            .rsplit_once(':')
            .ok_or_else(|| format!("`footprint` {named} is not <library>:<footprint>"))?;
        let library = self.open_library_named(library).or_else(|why| {
            let mut by_name = self.library.open_libraries.iter();
            by_name
                .find(|lib| lib.display_name == library.trim())
                .ok_or(why)
        })?;
        let footprint = footprint.trim();
        let id = Uuid::parse_str(footprint).ok();
        let mut found = library
            .cached_footprints
            .iter()
            .filter(|s| Some(s.uuid) == id || s.name == footprint);
        match (found.next(), found.next()) {
            (Some(one), None) => Ok(PrimitiveRef::new(library.library_id, one.uuid)),
            (None, _) => Err(format!(
                "library {} has no footprint {footprint}",
                library.display_name
            )),
            (Some(_), Some(_)) => Err(format!(
                "library {} has several footprints named {footprint}; name it by its id",
                library.display_name
            )),
        }
    }

    /// The parts on `sheet` an assignment names: `ids` (placed parts by
    /// uuid), `id` or `designator`, exactly one of them.
    fn named_parts(&self, sheet: &Path, args: &CommandArgs) -> Result<Vec<Uuid>, String> {
        let given = ["ids", "id", "designator"]
            .into_iter()
            .filter(|name| args.operand(name).is_some())
            .count();
        if given != 1 {
            return Err("name the parts once: `id`, `designator` or `ids`".into());
        }
        let doc = (self.document_state.engines.get(sheet))
            .map(|engine| engine.document())
            .ok_or("the sheet is not open")?;
        let wanted: Vec<Uuid> = if let Some(list) = args.targets("ids") {
            list.to_vec()
        } else if let Some(id) = text(args, "id") {
            vec![Uuid::parse_str(id.trim()).map_err(|_| format!("`id` {id} is not a uuid"))?]
        } else {
            let typed = text(args, "designator").unwrap_or_default().trim();
            let named = |s: &&Symbol| s.reference == typed && !s.is_power;
            let unit = (doc.symbols.iter().find(named))
                .ok_or_else(|| format!("no part on the sheet is named {typed}"))?;
            vec![unit.uuid]
        };
        for id in &wanted {
            if !doc.symbols.iter().any(|s| s.uuid == *id && !s.is_power) {
                return Err(format!("{id} is not a part on the sheet"));
            }
        }
        Ok(wanted)
    }

    /// `query_footprint_assignments document=<project or sheet>`: every
    /// part of the open sheets, with its designator, value, footprint, the
    /// library footprint it is built with and whether that footprint is in
    /// an open library. A project's sheets that are not open are listed.
    pub(super) fn query_footprint_assignments(&self, args: &CommandArgs) -> Result<Value, String> {
        let entry = match args.operand("document") {
            Some(OperandValue::Document(target)) => self.resolve_document_target(target)?,
            _ => return Err("`document` is needed".to_string()),
        };
        let docs = &self.document_state;
        let (sheets, closed): (Vec<PathBuf>, Vec<PathBuf>) = match &entry.key {
            Some(DocumentKey::Schematic(path)) => (vec![path.clone()], Vec::new()),
            Some(DocumentKey::Project(id)) => {
                let project = docs.projects.iter().find(|p| p.id == *id);
                let pages = crate::app::project_sheets::project_pages(project);
                let mut open: Vec<PathBuf> = (docs.engines.keys())
                    .filter(|path| docs.document_project(path).map(|p| p.id) == Some(*id))
                    .cloned()
                    .collect();
                open.sort();
                let closed = pages.into_iter().filter(|p| !open.contains(p)).collect();
                (open, closed)
            }
            _ => return Err("names a project or a schematic sheet".to_string()),
        };
        let mut parts = Vec::new();
        let mut seen = BTreeSet::new();
        for path in &sheets {
            let Some(doc) = docs.engines.get(path).map(|engine| engine.document()) else {
                continue;
            };
            for symbol in doc.symbols.iter().filter(|s| !s.is_power) {
                if !seen.insert(symbol.reference.clone()) && !symbol.reference.ends_with('?') {
                    continue;
                }
                parts.push(self.assignment_answer(path, symbol));
            }
        }
        Ok(json!({
            "document": entry.name,
            "parts": parts,
            "sheets_not_open": closed.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        }))
    }

    /// One part as the assignment query answers it.
    fn assignment_answer(&self, sheet: &Path, symbol: &Symbol) -> Value {
        let (_, binding) = self.footprint_offers(symbol);
        let resolves = binding
            .as_deref()
            .and_then(PrimitiveRef::parse_key)
            .is_some_and(|address| self.library.footprint_name(&address).is_some());
        json!({
            "id": symbol.uuid,
            "sheet": sheet.display().to_string(),
            "designator": symbol.reference,
            "value": symbol.value,
            "footprint": symbol.footprint,
            "binding": binding,
            "resolves": resolves,
        })
    }
}

/// `assign_footprint document=<sheet> id=<part> footprint=<library>:<fp>`
/// (or `designator=`, or `ids=`): the footprint every named part is built
/// with, through Assign Footprints' Apply, as one undo step.
pub(super) fn assign_footprint(app: &mut Signex, args: &CommandArgs) -> Ran {
    let sheet = match named_sheet(app, args, "`assign_footprint`") {
        Ok(sheet) => sheet,
        Err(reason) => return refused(reason),
    };
    let parts = match app.named_parts(&sheet, args) {
        Ok(parts) => parts,
        Err(reason) => return refused(reason),
    };
    let address = match app.footprint_operand(args) {
        Ok(address) => address,
        Err(reason) => return refused(reason),
    };
    let template = match app.prefetched_footprint(&address) {
        Ok(template) => template,
        Err(reason) => return refused(reason),
    };
    let Some(doc) = app.document_state.engines.get(&sheet).map(|e| e.document()) else {
        return refused("the sheet is not open");
    };
    let pads = pad_numbers(&template);
    let mut picks = Vec::with_capacity(parts.len());
    let mut mismatched = Vec::new();
    for id in &parts {
        let Some(symbol) = doc.symbols.iter().find(|s| s.uuid == *id) else {
            return refused(format!("{id} is not a part on the sheet"));
        };
        if let PinMap::Missing(pins) = pin_map(&pin_numbers(doc, symbol), &pads) {
            mismatched.push(format!("{} pins {}", symbol.reference, pins.join(", ")));
        }
        picks.push((*id, symbol.reference.clone()));
    }
    if !mismatched.is_empty() {
        return refused(format!(
            "the footprint has no pad for {}; nothing was changed",
            mismatched.join("; ")
        ));
    }
    let binding = lib_id_of(&address);
    let before = app.document_state.engines.get(&sheet).map(|e| e.revision());
    if let Err(problem) = app.assign_footprint_on_sheet(&sheet, &picks, &binding) {
        return refused(problem.english());
    }
    let Some(doc) = app.document_state.engines.get(&sheet).map(|e| e.document()) else {
        return refused("the sheet is not open");
    };
    let changed = app.document_state.engines.get(&sheet).map(|e| e.revision()) != before;
    let answered: Vec<Value> = (doc.symbols.iter())
        .filter(|s| parts.contains(&s.uuid))
        .map(|s| json!({"id": s.uuid, "designator": s.reference, "footprint": s.footprint}))
        .collect();
    let answer = json!({
        "document": sheet.display().to_string(),
        "footprint": binding,
        "changed": changed,
        "parts": answered,
    });
    (CommandOutcome::Answered(answer), Task::none())
}

/// `change_footprint document=<board> id=<uuid> footprint=<library>:<fp>`
/// (or `designator=`, or `ids=`): each named footprint swapped for the
/// library one as the footprint picker swaps it, one undo step.
pub(super) fn change_footprint(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.editable_named_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let footprints = match named_footprints(app, &board, args) {
        Ok(footprints) => footprints,
        Err(reason) => return refused(reason),
    };
    let address = match app.footprint_operand(args) {
        Ok(address) => address,
        Err(reason) => return refused(reason),
    };
    let template = match app.prefetched_footprint(&address) {
        Ok(template) => template,
        Err(reason) => return refused(reason),
    };
    let what = t!(app.ui_state.i18n, ids::PCB_COMPONENT_FOOTPRINT_SWAP).into_string();
    let done = match app.run_footprint_swap(&board, &footprints, &template, &what, true) {
        Ok(done) => done,
        Err(SwapRefusal::BoardGone) => return refused("the board is not open"),
        Err(SwapRefusal::Refused(why) | SwapRefusal::NotRun(why)) => return refused(why),
    };
    app.sync_pcb_board_summary();
    let Some(open) = app.document_state.pcb_board(&board) else {
        return refused("the board is not open");
    };
    let answered: Vec<Value> = (footprints.iter())
        .map(|id| footprint_answer(open, *id))
        .collect();
    let kept: Vec<&str> = done
        .as_ref()
        .map(|done| done.not_updated.iter().map(|(d, _)| d.as_str()).collect())
        .unwrap_or_default();
    let answer = json!({
        "document": board.display().to_string(),
        "footprint": lib_id_of(&address),
        "changed": done.is_some(),
        "footprints": answered,
        "parts_not_updated": kept,
    });
    (CommandOutcome::Answered(answer), Task::none())
}
