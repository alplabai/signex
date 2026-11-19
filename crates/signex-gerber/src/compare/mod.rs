//! Compare with board (#2539, #193d): a package read back against the
//! board it should come from. The board side is the Signex exporter's own
//! output for the board, read through the same reader, so the frame, the
//! shapes and the layer functions are decided once (`FabFrame`, the
//! exporter) and never re-derived here. Rules: `docs/GERBER_VIEWER.md`,
//! "Compare with board".

mod matching;
mod objects;
mod overlay;
mod reference;

use pcb_geom::{PointNm, Shape};
use signex_types::pcb::PcbBoard;

use crate::package::Package;
use crate::{FileFunction, SourceLine};

/// How far two objects may differ and still be the same object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompareOptions {
    /// Per axis, in nm, for positions, extents and hole sizes.
    pub tolerance_nm: i64,
}

/// The default tolerance: 0.010 mm.
pub const DEFAULT_TOLERANCE_NM: i64 = 10_000;

impl Default for CompareOptions {
    fn default() -> Self {
        CompareOptions {
            tolerance_nm: DEFAULT_TOLERANCE_NM,
        }
    }
}

/// What kind of difference a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DifferenceKind {
    /// On the board, not in the package's file. On a mask, paste or legend
    /// layer it is that layer's material; the viewer filters by layer.
    MissingCopper,
    /// In the package's file, not on the board.
    ExtraCopper,
    /// A hole at the same place with another size.
    DrillSizeMismatch,
    /// A file with no board layer: drawn, not compared.
    UnmatchedLayer,
    /// An object the reader could not build. Never counted as missing.
    NotReadable,
}

/// One difference between the package and the board.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    pub kind: DifferenceKind,
    /// The package layer, an index into [`Package::layers`].
    pub layer: usize,
    /// The board layer it was compared with; `None` for an unmatched one.
    pub board_layer: Option<FileFunction>,
    /// Where to zoom to, in the file frame.
    pub at: PointNm,
    /// The box of the object (min, max); `None` when it has no shape.
    pub extent: Option<(PointNm, PointNm)>,
    /// The X2 net and pin of the object that carries them.
    pub net: Option<String>,
    pub pin: Option<(String, String)>,
    pub size_in_file_nm: Option<i64>,
    pub size_on_board_nm: Option<i64>,
    /// Extra copper that meets a board keep-out of its layer.
    pub inside_keepout: bool,
    /// The package line the object was read from; `None` for what only the
    /// board has.
    pub source: Option<SourceLine>,
}

/// Which board layer a package layer is compared with.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerMatch {
    pub layer: usize,
    /// `None`: drawn, not compared.
    pub board: Option<FileFunction>,
    /// Chosen by the user rather than by the layer's function.
    pub by_hand: bool,
}

/// The board drawn over the package, in the file frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardOverlay {
    /// The board outline, then each cut-out.
    pub outline: Vec<Vec<PointNm>>,
    /// Every keep-out that keeps copper out, drawn and baked from the
    /// board sketch.
    pub keepouts: Vec<Vec<PointNm>>,
    /// The board's copper as the exporter draws it, per copper layer.
    pub copper: Vec<(FileFunction, Vec<Shape>)>,
}

/// The result of one comparison.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Comparison {
    /// One per package layer, in layer order.
    pub matching: Vec<LayerMatch>,
    /// By layer, kind, then position.
    pub differences: Vec<Difference>,
    pub overlay: BoardOverlay,
}

/// Compare `package` with `board`. `by_hand` overrides the match of a
/// package layer: `Some` compares it with that board layer, `None` leaves
/// it as a drawing; the last entry for a layer wins. A board the exporter
/// refuses (no outline) gives every layer unmatched and no other
/// difference.
pub fn compare_with_board(
    package: &Package,
    board: &PcbBoard,
    options: &CompareOptions,
    by_hand: &[(usize, Option<FileFunction>)],
) -> Comparison {
    let tolerance = options.tolerance_nm.max(0);
    let keepouts = overlay::keepouts(board);
    let mut overlay = BoardOverlay {
        outline: overlay::outline(board),
        keepouts: keepouts.iter().map(|k| k.ring.clone()).collect(),
        copper: Vec::new(),
    };
    let Some(reference) = reference::of_board(board) else {
        let matching: Vec<LayerMatch> = (0..package.layers.len())
            .map(|layer| LayerMatch {
                layer,
                board: None,
                by_hand: false,
            })
            .collect();
        let differences = matching
            .iter()
            .map(|m| objects::unmatched(m.layer, &package.layers[m.layer].image))
            .collect();
        return Comparison {
            matching,
            differences,
            overlay,
        };
    };
    let copper_count = reference::copper_count(&reference);
    let matching = matching::match_layers(package, &reference, copper_count, by_hand);
    let pins = objects::pins_of(&reference);
    let mut differences = Vec::new();
    for found in &matching {
        let image = &package.layers[found.layer].image;
        let Some(board_layer) = &found.board else {
            differences.push(objects::unmatched(found.layer, image));
            continue;
        };
        let layer_keepouts = overlay::keepouts_on(&keepouts, board_layer, copper_count);
        let input = objects::LayerInput {
            layer: found.layer,
            board: board_layer,
            package: image,
            reference: matching::reference_image(&reference, board_layer, copper_count),
            tolerance,
            keepouts: &layer_keepouts,
            pins: &pins,
        };
        objects::compare_layer(&input, &mut differences);
    }
    differences.sort_by_key(|d| (d.layer, d.kind, d.at));
    overlay.copper = reference::copper_shapes(&reference);
    Comparison {
        matching,
        differences,
        overlay,
    }
}
