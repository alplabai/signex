//! The Import DXF dialog of a board sketch (#2794, mockup row 179c): the file
//! as read, with the units, scale and layer targets the user picks, then the
//! profile check, then what landed. Nothing here touches the sketch: Heal and
//! merge or Import as is builds the geometry from these choices and Cancel
//! drops them.

use std::collections::HashSet;
use std::sync::Arc;

use signex_engine::pcb::dxf::{
    DxfEdge, DxfEdgeShape, DxfLayerRow, DxfSource, DxfUnits, dxf_edges, dxf_layer_rows,
};
use signex_sketch::import::{
    ImportCheck, ImportPiece, ImportShape, ProfileReport, SketchImport, build_import,
    build_import_as_is, check_profiles, merge_overlaps,
};
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
    TargetPicked {
        row: usize,
        target: DxfLayerTarget,
    },
    JoinToggled(bool),
    /// Next: check profiles.
    CheckRequested,
    /// Heal and merge, or Import as is, on the check.
    Confirmed(DxfImportMode),
    /// Done on the result.
    Dismissed,
    Cancelled,
}

/// How an import treats what the profile check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxfImportMode {
    /// Close gaps under the heal tolerance and fold overlapping lines.
    HealAndMerge,
    /// Keep the drawing as it is.
    AsIs,
}

/// What an import landed, for the result card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SketchDxfResult {
    pub mode: DxfImportMode,
    /// Board outline profiles.
    pub outlines: usize,
    /// Mounting holes.
    pub holes: usize,
    /// Every other profile.
    pub others: usize,
    pub check: ImportCheck,
}

/// Where the dialog is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchDxfStep {
    /// Units, scale and a target per layer.
    Mapping,
    /// The profile check, with Heal and merge and Import as is.
    Checking,
    /// The geometry landed; Done closes the card.
    Imported(SketchDxfResult),
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
    /// What Heal and merge would make with these choices, or why it cannot.
    pub check: Result<ImportCheck, SketchDxfProblem>,
    pub step: SketchDxfStep,
    /// The pieces of these choices as drawn, for the preview; after Heal
    /// and merge, the pieces that landed.
    pub pieces: Vec<ImportPiece>,
    /// The profile check of `pieces` as drawn.
    pub report: ProfileReport,
    pub origin: BoardOrigin,
    source: Arc<DxfSource>,
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
            step: SketchDxfStep::Mapping,
            pieces: Vec::new(),
            report: ProfileReport::default(),
            origin,
            source,
        };
        dialog.refresh_check();
        dialog
    }

    /// Apply a choice. Mapping choices apply only while mapping; Next moves
    /// to the check when the choices can import. `Confirmed`, `Dismissed`
    /// and `Cancelled` are the handler's.
    pub fn apply(&mut self, event: SketchDxfMsg) {
        if self.step != SketchDxfStep::Mapping {
            return;
        }
        match event {
            SketchDxfMsg::UnitsPicked(units) => self.units = units,
            SketchDxfMsg::ScaleEdited(scale) => self.scale = scale,
            SketchDxfMsg::TargetPicked { row, target } => {
                if let Some(slot) = self.targets.get_mut(row) {
                    *slot = target;
                }
            }
            SketchDxfMsg::JoinToggled(join) => self.join = join,
            SketchDxfMsg::CheckRequested => {
                if self.check.is_ok() {
                    self.step = SketchDxfStep::Checking;
                }
                return;
            }
            SketchDxfMsg::Confirmed(_) | SketchDxfMsg::Dismissed | SketchDxfMsg::Cancelled => {
                return;
            }
        }
        self.refresh_check();
    }

    fn refresh_check(&mut self) {
        match self.pieces() {
            Ok(pieces) => {
                self.report = check_profiles(&pieces);
                self.check = Ok(build_import(&pieces, PlaneId::new(), self.join).check);
                self.pieces = pieces;
            }
            Err(problem) => {
                self.report = ProfileReport::default();
                self.pieces.clear();
                self.check = Err(problem);
            }
        }
    }

    /// The sketch geometry of the current choices, on `plane`, healed and
    /// merged.
    #[cfg(test)]
    pub fn build(&self, plane: PlaneId) -> Result<SketchImport, SketchDxfProblem> {
        self.build_as(plane, DxfImportMode::HealAndMerge)
    }

    /// The sketch geometry of the current choices, on `plane`, in `mode`.
    pub fn build_as(
        &self,
        plane: PlaneId,
        mode: DxfImportMode,
    ) -> Result<SketchImport, SketchDxfProblem> {
        let pieces = self.pieces()?;
        Ok(match mode {
            DxfImportMode::HealAndMerge => build_import(&pieces, plane, self.join),
            DxfImportMode::AsIs => build_import_as_is(&pieces, plane, self.join),
        })
    }

    /// What `import`, built in `mode`, makes.
    pub fn result_of(mode: DxfImportMode, import: &SketchImport) -> SketchDxfResult {
        let count = |role| {
            let tag = role_tag(role);
            import.profiles.iter().filter(|t| **t == tag).count()
        };
        let outlines = count(SketchRole::BoardOutline);
        let holes = count(SketchRole::MountingHole);
        SketchDxfResult {
            mode,
            outlines,
            holes,
            others: import.profiles.len() - outlines - holes,
            check: import.check,
        }
    }

    /// The import landed: the card shows `result`, and the preview what
    /// landed.
    pub fn land(&mut self, result: SketchDxfResult) {
        if result.mode == DxfImportMode::HealAndMerge {
            self.pieces = merge_overlaps(&self.pieces).0;
        }
        self.step = SketchDxfStep::Imported(result);
    }

    /// The pieces of the current choices as drawn.
    fn pieces(&self) -> Result<Vec<ImportPiece>, SketchDxfProblem> {
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
        let with_path: HashSet<usize> = edges
            .iter()
            .filter(|edge| !matches!(edge.shape, DxfEdgeShape::Circle { .. }))
            .map(|edge| edge.layer)
            .collect();
        let pieces: Vec<ImportPiece> = edges
            .iter()
            .filter_map(|edge| {
                let role = self.piece_role(edge, &with_path)?;
                Some(piece(edge, role_tag(role)))
            })
            .collect();
        if pieces.is_empty() {
            return Err(SketchDxfProblem::NothingPicked);
        }
        Ok(pieces)
    }

    /// What `edge` becomes; `None` on a skipped layer. A board has one
    /// outline, so a circle beside the lines or arcs of an outline layer is
    /// a hole in it. `with_path`: the layers holding a line or arc.
    fn piece_role(&self, edge: &DxfEdge, with_path: &HashSet<usize>) -> Option<SketchRole> {
        let target = self.targets.get(edge.layer).copied().flatten()?;
        let is_circle = matches!(edge.shape, DxfEdgeShape::Circle { .. });
        let beside_path = is_circle && with_path.contains(&edge.layer);
        Some(if target == SketchRole::BoardOutline && beside_path {
            SketchRole::MountingHole
        } else {
            target
        })
    }

    /// The role of an edge whose piece has `tag` (see [`role_tag`]).
    pub fn role_of(&self, tag: usize) -> SketchRole {
        DXF_LAYER_TARGETS
            .get(tag)
            .copied()
            .flatten()
            .unwrap_or(SketchRole::Unassigned)
    }
}

/// The tag of pieces of `role`, its place in [`DXF_LAYER_TARGETS`]: ends join
/// only within one tag, so only among pieces of one role.
fn role_tag(role: SketchRole) -> usize {
    DXF_LAYER_TARGETS
        .iter()
        .position(|target| *target == Some(role))
        .unwrap_or(DXF_LAYER_TARGETS.len())
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

fn piece(edge: &DxfEdge, tag: usize) -> ImportPiece {
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
    ImportPiece { shape, tag }
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

    /// A square and two holes, all on layer 0.
    const ONE_LAYER: &str = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\n0\n10\n10\n20\n0\n11\n10\n21\n10\n\
0\nLINE\n8\n0\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\n0\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nCIRCLE\n8\n0\n10\n3\n20\n3\n40\n1\n\
0\nCIRCLE\n8\n0\n10\n7\n20\n7\n40\n1\n\
0\nENDSEC\n0\nEOF\n";

    /// One circle alone on its layer.
    const ROUND: &str = "0\nSECTION\n2\nENTITIES\n\
0\nCIRCLE\n8\n0\n10\n5\n20\n5\n40\n5\n\
0\nENDSEC\n0\nEOF\n";

    /// The role each circle and each other edge of `file` lands with when
    /// its one layer is on Board Outline.
    fn roles_on_an_outline_layer(file: &str) -> (Vec<SketchRole>, Vec<SketchRole>) {
        let source = Arc::new(DxfSource::read(file).expect("the file reads"));
        let mut d = SketchDxfDialog::open("f.dxf".into(), source, BoardOrigin::default(), false);
        d.apply(SketchDxfMsg::TargetPicked {
            row: 0,
            target: Some(SketchRole::BoardOutline),
        });
        let import = d.build(PlaneId::new()).expect("it builds");
        let role = |id: signex_sketch::id::SketchEntityId| {
            let tag = import.edges.iter().find(|(edge, _)| *edge == id)?.1;
            Some(d.role_of(tag))
        };
        let (mut circles, mut others) = (Vec::new(), Vec::new());
        for entity in &import.entities {
            let Some(role) = role(entity.id) else {
                continue;
            };
            match entity.kind {
                signex_sketch::entity::EntityKind::Circle { .. } => circles.push(role),
                _ => others.push(role),
            }
        }
        (circles, others)
    }

    #[test]
    fn circles_beside_an_outline_on_one_layer_land_as_holes() {
        let (circles, others) = roles_on_an_outline_layer(ONE_LAYER);
        assert_eq!(circles, [SketchRole::MountingHole; 2]);
        assert_eq!(others, [SketchRole::BoardOutline; 4]);
    }

    #[test]
    fn a_circle_alone_on_an_outline_layer_stays_the_outline() {
        let (circles, others) = roles_on_an_outline_layer(ROUND);
        assert_eq!(circles, [SketchRole::BoardOutline]);
        assert!(others.is_empty());
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

    #[test]
    fn next_moves_to_the_check_only_when_the_choices_import() {
        let mut d = dialog(false);
        d.apply(SketchDxfMsg::ScaleEdited("0".into()));
        d.apply(SketchDxfMsg::CheckRequested);
        assert_eq!(d.step, SketchDxfStep::Mapping);
        d.apply(SketchDxfMsg::ScaleEdited("1".into()));
        d.apply(SketchDxfMsg::CheckRequested);
        assert_eq!(d.step, SketchDxfStep::Checking);
        d.apply(SketchDxfMsg::ScaleEdited("3".into()));
        assert_eq!(d.scale, "1", "the check holds the mapping still");
    }

    #[test]
    fn the_check_reports_the_profiles_as_drawn() {
        let d = dialog(false);
        assert_eq!(d.report.profiles.len(), 2, "the square and the hole");
        assert_eq!(d.report.problems(), 0);
        assert_eq!(d.report.circles, 1);
        assert_eq!(d.pieces.len(), 5);
    }

    #[test]
    fn landing_counts_the_outline_and_the_holes() {
        let mut d = dialog(false);
        let import = d.build(PlaneId::new()).expect("it builds");
        d.land(SketchDxfDialog::result_of(
            DxfImportMode::HealAndMerge,
            &import,
        ));
        let SketchDxfStep::Imported(result) = d.step else {
            panic!("{:?}", d.step);
        };
        assert_eq!((result.outlines, result.holes, result.others), (1, 1, 0));
        assert_eq!(result.check.constraints, 4);
    }
}
