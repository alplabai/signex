//! The non-pad fields a sketch bakes into a footprint, and which of them
//! an empty bake may clear. Ownership rule: `signex_sketch::baked_kinds`.

use std::collections::HashMap;

use signex_library::primitive::footprint::{Footprint, FpGraphic, Polygon};
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::{SketchData, SketchError};

/// A baked field that can be told empty.
trait BakeField: Default {
    fn is_unset(&self) -> bool;
}

impl<T> BakeField for Vec<T> {
    fn is_unset(&self) -> bool {
        self.is_empty()
    }
}

impl BakeField for Polygon {
    fn is_unset(&self) -> bool {
        self.points.is_empty()
    }
}

/// One solve's bake of every non-pad kind.
#[derive(Default)]
pub(super) struct BakedFields {
    silk_f: Vec<FpGraphic>,
    silk_b: Vec<FpGraphic>,
    courtyard: Polygon,
    mask_openings: Vec<signex_library::primitive::footprint::FpMaskOpening>,
    mask_excludes: Vec<signex_library::primitive::footprint::FpMaskExclude>,
    paste_apertures: Vec<signex_library::primitive::footprint::FpPasteAperture>,
    pours: Vec<signex_library::primitive::footprint::FpPour>,
    keepouts: Vec<signex_library::primitive::footprint::FpKeepout>,
    cutouts: Vec<signex_library::primitive::footprint::FpCutout>,
    v_scores: Vec<signex_library::primitive::footprint::FpVScore>,
}

impl BakedFields {
    pub(super) fn bake(
        sketch: &SketchData,
        out: &FullSolveOutput,
        resolved: &HashMap<String, f64>,
        w: &mut Vec<String>,
    ) -> Result<Self, SketchError> {
        use signex_bake as b;
        let mut f = Self::default();
        b::bake_silk(sketch, out, &mut f.silk_f, &mut f.silk_b, w)?;
        b::bake_courtyard(sketch, out, &mut f.courtyard, w)?;
        b::bake_mask_openings(sketch, out, &mut f.mask_openings, w)?;
        b::bake_mask_excludes(sketch, out, &mut f.mask_excludes, w)?;
        b::bake_paste_apertures(sketch, out, &mut f.paste_apertures, w)?;
        b::bake_pours(sketch, out, resolved, &mut f.pours, w)?;
        b::bake_keepouts(sketch, out, &mut f.keepouts, w)?;
        b::bake_cutouts(sketch, out, resolved, &mut f.cutouts, w)?;
        b::bake_v_scores(sketch, out, resolved, &mut f.v_scores, w)?;
        Ok(f)
    }

    /// The bake of a sketch with no entities: every owned kind, pads
    /// included, clears. A sketch that never baked owns nothing, so a
    /// literal-pad footprint entering Sketch mode is untouched.
    pub(super) fn clear_owned(fp: &mut Footprint, warnings: &mut Vec<String>) {
        if let Some(sketch) = fp.sketch.as_mut()
            && sketch.was_baked(BakedKind::Pads)
        {
            if !fp.pads.is_empty() {
                warnings.push("cleared the footprint's pads: the sketch bakes none".into());
            }
            fp.pads.clear();
            sketch.record_baked(BakedKind::Pads, false);
        }
        Self::default().apply(fp, warnings);
    }

    /// Write each kind into `fp`. A non-empty bake replaces the field and
    /// records the kind as baked. An empty bake clears the field only when
    /// the sketch owns the kind, and says so in `warnings`.
    pub(super) fn apply(self, fp: &mut Footprint, warnings: &mut Vec<String>) {
        let Some(mut sketch) = fp.sketch.take() else {
            debug_assert!(false, "apply is only reached with a sketch");
            return;
        };
        macro_rules! settle {
            ($($kind:ident => $field:ident),* $(,)?) => {$(
                settle(BakedKind::$kind, &mut fp.$field, self.$field, &mut sketch, warnings);
            )*};
        }
        settle! {
            SilkF => silk_f, SilkB => silk_b, Courtyard => courtyard,
            MaskOpenings => mask_openings, MaskExcludes => mask_excludes,
            PasteApertures => paste_apertures, Pours => pours, Keepouts => keepouts,
            Cutouts => cutouts, VScores => v_scores,
        }
        fp.sketch = Some(sketch);
    }
}

fn settle<T: BakeField>(
    kind: BakedKind,
    dst: &mut T,
    baked: T,
    sketch: &mut SketchData,
    warnings: &mut Vec<String>,
) {
    if !baked.is_unset() {
        *dst = baked;
        sketch.record_baked(kind, true);
        return;
    }
    if !sketch.owns_kind(kind) {
        return;
    }
    if !dst.is_unset() {
        warnings.push(format!(
            "cleared the footprint's {}: the sketch bakes none",
            kind.label()
        ));
    }
    *dst = T::default();
    sketch.record_baked(kind, false);
}
