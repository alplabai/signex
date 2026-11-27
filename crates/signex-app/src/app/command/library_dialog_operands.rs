//! The library dialogs' actions as catalog commands (#3013, mockup rows
//! 132-134): `set_library_document_options`, `copy_stock_to_library` and
//! `new_library`. Each fills the dialog the person would see and presses its
//! button through `Signex::update`, so the command and the dialog have one
//! behaviour. A dialog the person has open is never taken over: the command
//! is refused until it is closed. See docs/CLI.md, "Library dialogs".

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_types::coord::Unit;

use super::operand_handlers::refused;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::stock_copy::{StockCopyMsg, StockCopyState, StockPrimitiveKind};
use crate::panels::SheetColor;

type Ran = (CommandOutcome, Task<Message>);

/// A refusal in the catalog's words.
pub(super) fn refuse(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

/// `command` was run without the operand `operand`.
pub(super) fn needs(command: &str, operand: &str) -> Ran {
    refuse(
        Translatable::new(ids::LIBRARY_COMMAND_NEEDS)
            .arg("command", command)
            .arg("operand", operand),
    )
}

pub(super) fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()),
        _ => None,
    }
}

pub(super) fn path(args: &CommandArgs, name: &str) -> Option<PathBuf> {
    match args.operand(name) {
        Some(OperandValue::Path(path)) => Some(PathBuf::from(path)),
        _ => None,
    }
}

fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

/// Send `msg` as the dialog's control does.
pub(super) fn send(app: &mut Signex, msg: LibraryMessage) -> Task<Message> {
    app.update(Message::Library(msg))
}

const SHEET_COLORS: [(&str, SheetColor); 5] = [
    ("black", SheetColor::Black),
    ("white", SheetColor::White),
    ("dark_gray", SheetColor::DarkGray),
    ("light_gray", SheetColor::LightGray),
    ("cream", SheetColor::Cream),
];

const UNITS: [(&str, Unit); 4] = [
    ("mm", Unit::Mm),
    ("mil", Unit::Mil),
    ("inch", Unit::Inch),
    ("um", Unit::Micrometer),
];

/// The value `name` stands for in `table`, or the name of `value`.
fn named<T: Copy + PartialEq>(table: &[(&'static str, T)], name: &str) -> Option<T> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

fn name_of<T: Copy + PartialEq>(table: &[(&'static str, T)], value: T) -> &'static str {
    table
        .iter()
        .find(|(_, v)| *v == value)
        .map_or("", |(n, _)| *n)
}

/// The `.snxlib` of the open library `named` names: its id, else its path.
fn open_library_root(app: &Signex, named: &str) -> Option<PathBuf> {
    let libraries = &app.library.open_libraries;
    if let Ok(id) = named.parse::<uuid::Uuid>() {
        return libraries
            .iter()
            .find(|lib| lib.library_id == id)
            .map(|lib| lib.root.clone());
    }
    app.library
        .library_at(Path::new(named))
        .map(|lib| lib.root.clone())
}

/// The display settings of the library at `root`, as the answer gives them.
fn options_answer(app: &Signex, root: &Path) -> Value {
    let Some(lib) = app.library.library_at(root) else {
        return json!({ "library": root.display().to_string() });
    };
    let display = lib.display;
    json!({
        "library": root.display().to_string(),
        "name": lib.display_name,
        "sheet_color": name_of(&SHEET_COLORS, display.sheet_color),
        "grid": display.grid_visible,
        "grid_spacing": crate::canvas::grid::pitch_mm_exact(display.grid_size_mm),
        "unit": name_of(&UNITS, display.unit),
    })
}

/// `set_library_document_options library=<id or .snxlib> [sheet_color=..]
/// [grid=..] [grid_spacing=<mm>] [unit=..]`: Document Options' Apply with
/// the settings given; one left out keeps its value. The settings live in
/// memory for the session, as the dialog's do.
pub(super) fn set_library_document_options(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(named_library) = text(args, "library") else {
        return needs("set_library_document_options", "library");
    };
    let Some(root) = open_library_root(app, named_library) else {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_LIBRARY_UNKNOWN)
                .arg("library", named_library.to_string()),
        );
    };
    if let Some(open) = &app.library.document_options {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_OPTIONS_OPEN)
                .arg("name", open.library_name.clone()),
        );
    }
    let sheet_color = match text(args, "sheet_color") {
        None => None,
        Some(name) => match named(&SHEET_COLORS, name) {
            Some(color) => Some(color),
            None => return refuse(Translatable::new(ids::LIBRARY_COMMAND_SHEET_COLOR)),
        },
    };
    let unit = match text(args, "unit") {
        None => None,
        Some(name) => match named(&UNITS, name) {
            Some(unit) => Some(unit),
            None => return refuse(Translatable::new(ids::LIBRARY_COMMAND_UNIT)),
        },
    };
    let spacing = match args.operand("grid_spacing") {
        Some(OperandValue::Length(mm)) => Some(*mm as f32),
        _ => None,
    };
    let mut tasks = vec![send(
        app,
        LibraryMessage::OpenDocumentOptions {
            library_path: root.clone(),
        },
    )];
    if let Some(color) = sheet_color {
        tasks.push(send(
            app,
            LibraryMessage::DocumentOptionsSetSheetColor(color),
        ));
    }
    let shown = app
        .library
        .document_options
        .as_ref()
        .map(|state| state.draft.grid_visible);
    if let Some(visible) = flag(args, "grid")
        && shown.is_some_and(|shown| shown != visible)
    {
        tasks.push(send(app, LibraryMessage::DocumentOptionsToggleGrid));
    }
    if let Some(mm) = spacing {
        tasks.push(send(
            app,
            LibraryMessage::DocumentOptionsGridSpacingPicked(mm),
        ));
    }
    if let Some(unit) = unit {
        tasks.push(send(app, LibraryMessage::DocumentOptionsUnitPicked(unit)));
    }
    tasks.push(send(app, LibraryMessage::DocumentOptionsApply));
    let answer = options_answer(app, &root);
    (CommandOutcome::Answered(answer), Task::batch(tasks))
}

/// The file of the stock primitive of `kind` the stock library lists as
/// `name`.
fn stock_file(app: &Signex, name: &str, kind: StockPrimitiveKind) -> Result<PathBuf, Translatable> {
    let stock = app
        .library
        .stock_library_root
        .as_deref()
        .and_then(|root| app.library.library_at(root))
        .ok_or_else(|| Translatable::new(ids::LIBRARY_COMMAND_STOCK_NOT_OPEN))?;
    let listed = match kind {
        StockPrimitiveKind::Symbol => &stock.cached_symbols,
        StockPrimitiveKind::Footprint => &stock.cached_footprints,
    };
    let found: Vec<&PathBuf> = listed
        .iter()
        .filter(|primitive| primitive.name == name)
        .filter_map(|primitive| primitive.file.as_ref())
        .collect();
    match found.as_slice() {
        [file] => Ok((*file).clone()),
        [] => Err(Translatable::new(ids::LIBRARY_COMMAND_STOCK_UNKNOWN)
            .arg("kind", kind.noun())
            .arg("name", name.to_string())),
        many => Err(Translatable::new(ids::LIBRARY_COMMAND_STOCK_MANY)
            .arg("count", many.len())
            .arg("name", name.to_string())),
    }
}

/// `copy_stock_to_library name=<stock primitive> kind=symbol|footprint
/// path=<.snxlib>`: Copy to Library's "Copy to <library>" with that library
/// selected. The copy is written off the UI thread, as the dialog's is, and
/// opens in its editor when it lands; the answer names what was copied where.
pub(super) fn copy_stock_to_library(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text(args, "name").unwrap_or_default().to_string();
    let kind = match text(args, "kind") {
        Some("symbol") => StockPrimitiveKind::Symbol,
        Some("footprint") => StockPrimitiveKind::Footprint,
        _ => return refuse(Translatable::new(ids::LIBRARY_COMMAND_STOCK_KIND)),
    };
    let Some(target) = path(args, "path") else {
        return needs("copy_stock_to_library", "path");
    };
    let source = match stock_file(app, &name, kind) {
        Ok(source) => source,
        Err(reason) => return refuse(reason),
    };
    if let Some(open) = &app.library.stock_copy {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_STOCK_BUSY).arg("name", open.label.clone()),
        );
    }
    if app.library.stock_copies_in_flight.contains(&source) {
        return refuse(Translatable::new(ids::LIBRARY_COMMAND_STOCK_RUNNING).arg("name", name));
    }
    // The dialog an open of the stock primitive raises, `path` selected.
    app.library.stock_copy = Some(StockCopyState {
        source: source.clone(),
        kind,
        label: name.clone(),
        previous_unavailable: None,
        error: None,
        selected: Some(target.clone()),
    });
    let task = send(
        app,
        LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(target.clone())),
    );
    // Still open: the library cannot take the copy, and the dialog says so.
    if app.library.stock_copy.take().is_some() {
        return refuse(
            Translatable::new(ids::LIBRARY_STOCK_COPY_NOT_WRITABLE)
                .arg("path", target.display().to_string()),
        );
    }
    let answer = json!({
        "name": name,
        "kind": kind.noun(),
        "source": source.display().to_string(),
        "library": target.display().to_string(),
    });
    (CommandOutcome::Answered(answer), task)
}

/// The path of the open project `id`.
fn path_of_project(app: &Signex, id: crate::app::state::ProjectId) -> Option<PathBuf> {
    app.document_state
        .project_by_id(id)
        .map(|project| project.path.clone())
}

/// `new_library document=<project> path=<.snxlib> [git=..] [lfs=..]`: the
/// New Library dialog's Create Library with those options. As there, the
/// library is registered with the project and written when the project is
/// saved; the answer is the library waiting to be created.
pub(super) fn new_library(app: &mut Signex, args: &CommandArgs) -> Ran {
    let project = match app.rules_project_named(args) {
        Ok(project) => project,
        Err(reason) => return refused(reason),
    };
    let Some(lib_path) = path(args, "path") else {
        return needs("new_library", "path");
    };
    let (git, lfs) = (
        flag(args, "git").unwrap_or(false),
        flag(args, "lfs").unwrap_or(false),
    );
    if lfs && !git {
        return refuse(Translatable::new(ids::LIBRARY_COMMAND_NEW_LFS));
    }
    if let Some(open) = &app.library.create_options {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_NEW_OPEN)
                .arg("path", open.lib_path.display().to_string()),
        );
    }
    let Some(project_path) = path_of_project(app, project) else {
        return needs("new_library", "document");
    };
    let mut tasks = vec![send(
        app,
        LibraryMessage::CreateLibraryAtPath {
            project_path: project_path.clone(),
            lib_path: lib_path.clone(),
        },
    )];
    if git {
        tasks.push(send(app, LibraryMessage::LibraryCreateOptionsToggleGit));
    }
    if lfs {
        tasks.push(send(app, LibraryMessage::LibraryCreateOptionsToggleLfs));
    }
    tasks.push(send(app, LibraryMessage::LibraryCreateOptionsConfirm));
    // Still open: Create Library was refused, and the dialog says why.
    if let Some(state) = app.library.create_options.take() {
        return refused(state.refusal.unwrap_or_default());
    }
    let pending = app
        .document_state
        .project_by_id(project)
        .and_then(|loaded| {
            loaded
                .pending_libraries
                .iter()
                .find(|(_, spec)| spec.lib_path == lib_path)
                .map(|(id, spec)| (*id, spec.clone()))
        });
    let Some((id, spec)) = pending else {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_NEW_NOT_REGISTERED)
                .arg("path", lib_path.display().to_string()),
        );
    };
    let answer = json!({
        "project": project_path.display().to_string(),
        "library": spec.lib_path.display().to_string(),
        "library_id": id.to_string(),
        "name": spec.display_name,
        "git": spec.enable_git,
        "lfs": spec.use_lfs,
        "pending": true,
    });
    (CommandOutcome::Answered(answer), Task::batch(tasks))
}
