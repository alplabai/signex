//! The Import DXF dialog of a board sketch (#2794, mockup row 179c): the file
//! as read, with the units, scale and layer targets the user picks. Nothing
//! here touches the sketch: Import builds the geometry from these choices and
//! Cancel drops them.

use std::sync::Arc;

use signex_engine::pcb::dxf::{
    DxfEdge, DxfEdgeShape, DxfLayerRow, DxfSource, DxfUnits, dxf_edges, dxf_layer_rows,
};
use signex_sketch::import::{ImportCheck, ImportPiece, ImportShape, SketchImport, build_import};
use signex_sketch::plane::PlaneId;
use signex_types::pcb::Point;
use signex_types::pcb_origin::BoardOrigin;
use signex_types::sketch::SketchRole;

/// What a DXF layer becomes; `None` is Skip.
pub type DxfLayerTarget = Option<SketchRole>;

/// The targets a layer can take, in the order the dialog lists them.
pub static DXF_LAYER_TARGETS: [DxfLayerTarget; 6] = [
    Some(SketchRole::BoardOutline),
    Some(SketchRole::Cutout),
    Some(SketchRole::Keepout),
    Some(SketchRole::MountingHole),
    Some(SketchRole::Unassigned),
    None,
];

/// An event of the dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum SketchDxfMsg {
    UnitsPicked(DxfUnits),
    ScaleEdited(String),
    TargetPicked { row: usize, target: DxfLayerTarget },
    JoinToggled(bool),
    Confirmed,
    Cancelled,
}

/// Why the dialog's choices cannot be imported as they stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchDxfProblem {
    /// The scale is not a positive number.
    Scale,
    /// The drawing would leave the range a board file holds.
    Range,
    /// Every layer is on Skip.
    NothingPicked,
}

/// The open dialog.
#[derive(Debug, Clone)]
pub struct SketchDxfDialog {
    /// The file's name, for the File row.
    pub file: String,
    pub units: DxfUnits,
    /// The scale as typed.
    pub scale: String,
    pub rows: Vec<DxfLayerRow>,
    /// The target of each of `rows`.
    pub targets: Vec<DxfLayerTarget>,
    /// Join ends under the heal tolerance and add H / V.
    pub join: bool,
    /// What Import would make with these choices, or why it cannot.
    pub check: Result<ImportCheck, SketchDxfProblem>,
    source: Arc<DxfSource>,
    origin: BoardOrigin,
}

impl SketchDxfDialog {
    /// The dialog for `source`. `has_outline`: the sketch already has an
    /// outline, so no layer starts as a second one.
    pub fn open(
        file: String,
        source: Arc<DxfSource>,
        origin: BoardOrigin,
        has_outline: bool,
    ) -> Self {
        let rows = dxf_layer_rows(&source.drawing);
        let targets = default_targets(&rows, has_outline);
        let mut dialog = Self {
            file,
            units: DxfUnits::FromFile,
            scale: "1".to_string(),
            rows,
            targets,
            join: true,
            check: Ok(ImportCheck::default()),
            source,
            origin,
        };
        dialog.refresh_check();
        dialog
    }

    /// Apply a choice. `Confirmed` and `Cancelled` are the handler's.
    pub fn apply(&mut self, event: SketchDxfMsg) {
        match event {
            SketchDxfMsg::UnitsPicked(units) => self.units = units,
            SketchDxfMsg::ScaleEdited(scale) => self.scale = scale,
            SketchDxfMsg::TargetPicked { row, target } => {
                if let Some(slot) = self.targets.get_mut(row) {
                    *slot = target;
                }
            }
            SketchDxfMsg::JoinToggled(join) => self.join = join,
            SketchDxfMsg::Confirmed | SketchDxfMsg::Cancelled => return,
        }
        self.refresh_check();
    }

    fn refresh_check(&mut self) {
        self.check = self.build(PlaneId::new()).map(|import| import.check);
    }

    /// The sketch geometry of the current choices, on `plane`.
    pub fn build(&self, plane: PlaneId) -> Result<SketchImport, SketchDxfProblem> {
        let scale = self
            .scale
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|s| s.is_finite() && *s > 0.0)
            .ok_or(SketchDxfProblem::Scale)?;
        let factor = self.units.to_mm(self.source.file_to_mm) * scale;
        let edges = dxf_edges(&self.source.drawing, self.origin, factor)
            .map_err(|_| SketchDxfProblem::Range)?;
        let pieces: Vec<ImportPiece> = edges
            .iter()
            .filter(|edge| self.targets.get(edge.layer).copied().flatten().is_some())
            .map(piece)
            .collect();
        if pieces.is_empty() {
            return Err(SketchDxfProblem::NothingPicked);
        }
        Ok(build_import(&pieces, plane, self.join))
    }

    /// The role of an edge from layer row `tag`.
    pub fn role_of(&self, tag: usize) -> SketchRole {
        self.targets
            .get(tag)
            .copied()
            .flatten()
            .unwrap_or(SketchRole::Unassigned)
    }
}

/// A layer of circles starts as mounting holes, the first other drawn layer
/// as the outline (unless the sketch has one), the rest as plain geometry;
/// a layer with nothing to draw starts on Skip.
fn default_targets(rows: &[DxfLayerRow], has_outline: bool) -> Vec<DxfLayerTarget> {
    let mut outline_free = !has_outline;
    rows.iter()
        .map(|row| {
            if row.drawable == 0 {
                None
            } else if row.circles == row.drawable {
                Some(SketchRole::MountingHole)
            } else if std::mem::take(&mut outline_free) {
                Some(SketchRole::BoardOutline)
            } else {
                Some(SketchRole::Unassigned)
            }
        })
        .collect()
}

fn piece(edge: &DxfEdge) -> ImportPiece {
    let xy = |p: Point| (p.x, p.y);
    let shape = match edge.shape {
        DxfEdgeShape::Line { a, b } => ImportShape::Line { a: xy(a), b: xy(b) },
        DxfEdgeShape::Arc { center, start, end } => ImportShape::Arc {
            center: xy(center),
            start: xy(start),
            end: xy(end),
        },
        DxfEdgeShape::Circle { center, radius } => ImportShape::Circle {
            center: xy(center),
            radius,
        },
    };
    ImportPiece {
        shape,
        tag: edge.layer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 10 x 10 mm square on BOARD, a hole on HOLES and a text on NOTES.
    const FILE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\nBOARD\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nCIRCLE\n8\nHOLES\n10\n5\n20\n5\n40\n1\n\
0\nTEXT\n8\nNOTES\n10\n0\n20\n0\n\
0\nENDSEC\n0\nEOF\n";

    fn dialog(has_outline: bool) -> SketchDxfDialog {
        let source = Arc::new(DxfSource::read(FILE).expect("the file reads"));
        SketchDxfDialog::open(
            "lid.dxf".into(),
            source,
            BoardOrigin::default(),
            has_outline,
        )
    }

    #[test]
    fn layers_start_on_outline_holes_and_skip() {
        let d = dialog(false);
        assert_eq!(
            d.targets,
            [
                Some(SketchRole::BoardOutline),
                Some(SketchRole::MountingHole),
                None
            ]
        );
        let check = d.check.expect("it can import");
        assert_eq!((check.lines, check.circles, check.open_ends), (4, 1, 0));
    }

    #[test]
    fn a_sketch_with_an_outline_gets_no_second_one_by_default() {
        assert_eq!(dialog(true).targets[0], Some(SketchRole::Unassigned));
    }

    #[test]
    fn a_skipped_layer_is_left_out() {
        let mut d = dialog(false);
        d.apply(SketchDxfMsg::TargetPicked {
            row: 1,
            target: None,
        });
        assert_eq!(d.check.expect("still imports").circles, 0);
    }

    #[test]
    fn a_bad_scale_or_all_layers_skipped_blocks_the_import() {
        let mut d = dialog(false);
        for bad in ["", "0", "-2", "abc", "inf"] {
            d.apply(SketchDxfMsg::ScaleEdited(bad.into()));
            assert_eq!(d.check, Err(SketchDxfProblem::Scale), "{bad:?}");
        }
        d.apply(SketchDxfMsg::ScaleEdited("2".into()));
        assert!(d.check.is_ok());
        for row in 0..d.rows.len() {
            d.apply(SketchDxfMsg::TargetPicked { row, target: None });
        }
        assert_eq!(d.check, Err(SketchDxfProblem::NothingPicked));
    }

    #[test]
    fn units_and_scale_size_the_geometry() {
        let mut d = dialog(false);
        d.apply(SketchDxfMsg::UnitsPicked(DxfUnits::Inch));
        d.apply(SketchDxfMsg::ScaleEdited("2".into()));
        let import = d.build(PlaneId::new()).expect("it builds");
        let far = import
            .entities
            .iter()
            .filter_map(|e| match e.kind {
                signex_sketch::entity::EntityKind::Point { x, .. } => Some(x),
                _ => None,
            })
            .fold(f64::MIN, f64::max);
        assert!((far - 508.0).abs() < 1e-9, "10 in x 2 = 508 mm: {far}");
    }
}
