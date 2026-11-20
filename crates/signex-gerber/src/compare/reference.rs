//! The board side of a comparison: the board's own Gerber, drill and job
//! files as the Signex exporter writes them, read back through the
//! package reader. One exporter, one frame, one reader.

use pcb_geom::Shape;
use signex_output::fab::{FabExtras, FabFileKind, FabOptions, generate};
use signex_types::pcb::PcbBoard;

use crate::package::{Package, assemble};
use crate::{FileFunction, Polarity, read_file};

/// The file-name stem of the reference files; never written anywhere.
const REFERENCE_NAME: &str = "compare";

/// The board's images read back, or `None` when the exporter refuses the
/// board (it has no outline).
pub(super) fn of_board(board: &PcbBoard) -> Option<Package> {
    // Every output off but the images and the drill files. Starting from
    // `all` keeps this compiling when an output kind is added.
    let mut options = FabOptions::all(REFERENCE_NAME);
    options.centroid = false;
    options.test_netlist = false;
    options.ipc2581 = false;
    options.odb = None;
    let set = generate(board, &FabExtras::of_board(board), &options).ok()?;
    let files = set
        .files
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                FabFileKind::Gerber { .. } | FabFileKind::Drill { .. } | FabFileKind::Job
            )
        })
        .map(|file| read_file(&file.name, file.contents.as_bytes()))
        .collect();
    Some(assemble(files))
}

/// How many copper layers the board has: the highest copper number of its
/// images (the exporter always writes L1 and the bottom).
pub(super) fn copper_count(reference: &Package) -> u32 {
    reference
        .layers
        .iter()
        .filter_map(|layer| match layer.function {
            Some(FileFunction::Copper { number, .. }) => Some(number),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// The board's copper shapes per copper layer, top first; cut-outs (clear
/// objects) are left to the drawing of the package.
pub(super) fn copper_shapes(reference: &Package) -> Vec<(FileFunction, Vec<Shape>)> {
    let mut copper: Vec<(FileFunction, Vec<Shape>)> = reference
        .layers
        .iter()
        .filter_map(|layer| {
            let function = layer.function.clone().filter(FileFunction::is_copper)?;
            let shapes = layer
                .image
                .objects
                .iter()
                .filter(|object| object.polarity == Polarity::Dark)
                .flat_map(|object| object.shapes.iter().cloned())
                .collect();
            Some((function, shapes))
        })
        .collect();
    copper.sort_by(|a, b| a.0.cmp(&b.0));
    copper
}
