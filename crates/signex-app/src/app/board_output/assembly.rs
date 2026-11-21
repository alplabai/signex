//! Assembly drawings as SVG files (#2486): Board Print / PDF's Export
//! Assembly SVG and `export_assembly_svg` write the top and bottom drawings
//! of `signex_output::assembly_drawing` into a folder, and
//! `board_assembly_drawing` answers one as SVG text. The PDF pages are the
//! print path's (`AssemblyTop` / `AssemblyBottom` sets).

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_i18n::ids;
use signex_output::{
    AssemblySvgOptions, BoardLayerSet, BoardPrintOptions, assembly_drawing, assembly_svg,
};
use signex_types::pcb_layer::BoardSide;

use super::operands::{Ran, file, flag, refused, text};
use super::{BoardOutputDialog, BoardOutputMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};

/// One drawing made for a file: its side, where it goes and the SVG.
pub(super) struct AssemblySvgFile {
    pub side: BoardSide,
    pub path: PathBuf,
    pub mirrored: bool,
    pub svg: String,
}

fn side_id(side: BoardSide) -> &'static str {
    match side {
        BoardSide::Top => "top",
        BoardSide::Bottom => "bottom",
    }
}

fn side_from_id(id: &str) -> Option<BoardSide> {
    match id.trim() {
        "top" => Some(BoardSide::Top),
        "bottom" => Some(BoardSide::Bottom),
        _ => None,
    }
}

/// The `side` operand of `board_assembly_drawing`; `None` when it is
/// missing or not `top` / `bottom`.
pub(crate) fn side_operand(args: &CommandArgs) -> Option<BoardSide> {
    text(args, "side").and_then(side_from_id)
}

/// The sides an export writes: the assembly sets chosen to print, else
/// both, top first.
pub(super) fn svg_sides(options: &BoardPrintOptions) -> Vec<BoardSide> {
    let chosen: Vec<BoardSide> = BoardLayerSet::ALL
        .into_iter()
        .filter(|set| options.sets.contains(set))
        .filter_map(BoardLayerSet::assembly_side)
        .collect();
    if chosen.is_empty() {
        vec![BoardSide::Top, BoardSide::Bottom]
    } else {
        chosen
    }
}

/// `<board>-assembly-top.svg` and `<board>-assembly-bottom.svg`.
pub(super) fn svg_file_name(stem: &str, side: BoardSide) -> String {
    let stem = if stem.trim().is_empty() {
        "board"
    } else {
        stem
    };
    format!("{stem}-assembly-{}.svg", side_id(side))
}

/// The files named `names` that are already in `folder`. Runs in the
/// folder dialog's task, off the UI thread.
pub(super) fn existing_files(folder: &Path, names: &[String]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|name| folder.join(name))
        .filter(|path| path.is_file())
        .collect()
}

/// The Messages line of a written drawing.
fn svg_written_log(file: &AssemblySvgFile, replaced: bool) -> String {
    let verb = if replaced { "Replaced" } else { "Wrote" };
    format!("{verb} {} ({} bytes)", file.path.display(), file.svg.len())
}

impl Signex {
    /// The drawings of the open board at `board` for `sides`, each named
    /// for its file in `folder`; `None` when the board is not open.
    pub(super) fn assembly_svgs_of(
        &self,
        board: &Path,
        sides: &[BoardSide],
        options: AssemblySvgOptions,
        folder: &Path,
    ) -> Option<Vec<AssemblySvgFile>> {
        let pcb = self.document_state.pcb_board_shown(board)?;
        let stem = super::file_title(board);
        let files = sides
            .iter()
            .map(|&side| {
                let mirrored = options.mirrored && side == BoardSide::Bottom;
                let drawing = assembly_drawing(&pcb, side);
                let look = AssemblySvgOptions {
                    mirrored,
                    ..options
                };
                AssemblySvgFile {
                    side,
                    path: folder.join(svg_file_name(&stem, side)),
                    mirrored,
                    svg: assembly_svg(&drawing, &look),
                }
            })
            .collect();
        Some(files)
    }

    /// What an Assembly SVG export writes: Board Print's board and options;
    /// with Board Reports open, its board with the options last used; else
    /// the board in view with them.
    pub(crate) fn assembly_svg_source(&self) -> Option<(PathBuf, BoardPrintOptions)> {
        let last_used = || self.ui_state.board_output.print_options.clone();
        match self.ui_state.board_output.dialog.as_deref() {
            Some(BoardOutputDialog::Print(state)) => {
                Some((state.board.clone(), state.options.clone()))
            }
            Some(BoardOutputDialog::Reports(state)) => Some((state.board.clone(), last_used())),
            None => self.board_in_view().map(|(board, _)| (board, last_used())),
        }
    }

    /// Export Assembly SVG: the folder dialog for
    /// [`Self::assembly_svg_source`].
    pub(super) fn request_assembly_svg(&mut self) -> Task<Message> {
        let Some((board, options)) = self.assembly_svg_source() else {
            self.explain_no_board();
            return Task::none();
        };
        let target = self.save_target(Some(&board));
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        let stem = super::file_title(&board);
        let names: Vec<String> = svg_sides(&options)
            .into_iter()
            .map(|side| svg_file_name(&stem, side))
            .collect();
        let options = Box::new(options);
        Task::perform(
            async move {
                let folder = target
                    .dialog(picker.dialog().title_in(&i18n, ids::BOARD_PRINT_SVG_FOLDER))
                    .await
                    .pick_folder()
                    .await?;
                let replaced = existing_files(&folder, &names);
                Some((folder, replaced))
            },
            move |chosen| match chosen {
                Some((folder, replaced)) => Message::from(BoardOutputMsg::SvgFolderChosen {
                    board,
                    options,
                    folder,
                    replaced,
                }),
                None => Message::noop(),
            },
        )
    }

    /// Draw the assembly drawings of `board`, as it is now, into `folder`;
    /// `replaced` are the files there the folder answer found.
    pub(super) fn write_assembly_svgs(
        &mut self,
        board: &Path,
        options: &BoardPrintOptions,
        folder: &Path,
        replaced: &[PathBuf],
    ) -> Task<Message> {
        let look = AssemblySvgOptions {
            mirrored: options.mirror_bottom,
            monochrome: options.monochrome,
        };
        match self.assembly_svgs_of(board, &svg_sides(options), look, folder) {
            Some(files) => self.write_svg_files(files, replaced),
            None => {
                self.refuse_board_gone(board);
                Task::none()
            }
        }
    }

    /// Write made drawings off the UI thread, each crash-safe; the Messages
    /// line of a file in `replaced` says it was replaced.
    pub(super) fn write_svg_files(
        &mut self,
        files: Vec<AssemblySvgFile>,
        replaced: &[PathBuf],
    ) -> Task<Message> {
        let tasks: Vec<Task<Message>> = files
            .into_iter()
            .map(|file| {
                let log = svg_written_log(&file, replaced.contains(&file.path));
                self.write_export(
                    file.path,
                    file.svg.into_bytes(),
                    "SVG",
                    log,
                    None,
                    Vec::new(),
                )
            })
            .collect();
        Task::batch(tasks)
    }
}

/// The SVG look the operands ask for: the bottom mirrored unless
/// `mirror_bottom=false`, colour unless `monochrome`.
fn look(args: &CommandArgs) -> AssemblySvgOptions {
    AssemblySvgOptions {
        mirrored: flag(args, "mirror_bottom").unwrap_or(true),
        monochrome: flag(args, "monochrome").unwrap_or(false),
    }
}

/// `export_assembly_svg document=<board> path=<folder> [sides] [...]`: the
/// drawings written off the UI thread; the answer lists the files.
pub(crate) fn export_assembly_svg(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let Some(folder) = file(args) else {
        return refused("needs the operand `path`");
    };
    let sides = match text(args, "sides") {
        None => vec![BoardSide::Top, BoardSide::Bottom],
        Some(list) => {
            let mut sides = Vec::new();
            for id in list.split(',').filter(|id| !id.trim().is_empty()) {
                let Some(side) = side_from_id(id) else {
                    return refused(format!("`{}` is not a side (top or bottom)", id.trim()));
                };
                if !sides.contains(&side) {
                    sides.push(side);
                }
            }
            if sides.is_empty() {
                return refused("`sides` names no side");
            }
            sides
        }
    };
    let Some(files) = app.assembly_svgs_of(&board, &sides, look(args), &folder) else {
        return refused("the board is not open");
    };
    let listed: Vec<_> = files
        .iter()
        .map(|f| {
            json!({
                "side": side_id(f.side),
                "path": f.path.display().to_string(),
                "mirrored": f.mirrored,
            })
        })
        .collect();
    let answer = json!({
        "document": board.display().to_string(),
        "files": listed,
    });
    let task = app.write_svg_files(files, &[]);
    (CommandOutcome::Answered(answer), task)
}

/// `board_assembly_drawing document=<board> side=<top|bottom>`: the drawing
/// as SVG text. Nothing is written.
pub(crate) fn board_assembly_drawing(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let Some(side) = side_operand(args) else {
        return refused("`side` is top or bottom");
    };
    let Some(pcb) = app.document_state.pcb_board_shown(&board) else {
        return refused("the board is not open");
    };
    let mut options = look(args);
    options.mirrored &= side == BoardSide::Bottom;
    let drawing = assembly_drawing(&pcb, side);
    let answer = json!({
        "document": board.display().to_string(),
        "side": side_id(side),
        "mirrored": options.mirrored,
        "extent_mm": {
            "min": [drawing.min.x, drawing.min.y],
            "max": [drawing.max.x, drawing.max.y],
        },
        "svg": assembly_svg(&drawing, &options),
    });
    (CommandOutcome::Answered(answer), Task::none())
}
