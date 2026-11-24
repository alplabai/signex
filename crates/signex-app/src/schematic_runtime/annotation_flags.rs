//! Annotation flags on the sheet (mockup row 165): every part that Design
//! Notebook annotations name carries one flag per kind at its top-right
//! corner, in the kind's colour, with how many annotations of that kind name
//! it. Drawn in the selection layer, so the flags follow the camera, are
//! editor-only and never reach a print or an export. The geometry is
//! `signex_renderer::schematic::annotation_flags`; the rules are in
//! `docs/NOTEBOOK_ANNOTATIONS.md`, "Annotation flags".

use super::*;
use signex_renderer::schematic::annotation_flags::{annotation_flag_snapshot, flag_feet};
use signex_renderer::schematic::{AnnotationFlagInput, AnnotationFlagMetrics};
use signex_widgets::theme_ext::{
    AnnotationTone, TypeScale, annotation_glyph_colour, annotation_kind_colour,
};

const TS: TypeScale = TypeScale::DEFAULT;
/// The number on a flag; the flag is sized from it (mockup: a 10 px number
/// on a 16 px pennant, 28 px wide on a 28 px pole, poles 40 px apart).
const TEXT_PX: f32 = TS.caption;
const PENNANT_HEIGHT_PX: f32 = TEXT_PX * 1.6;
const PENNANT_WIDTH_PX: f32 = TEXT_PX * 2.8;
const POLE_PX: f32 = TEXT_PX * 2.8;
const NOTCH_PX: f32 = TEXT_PX * 0.6;
const STEP_PX: f32 = TEXT_PX * 4.0;
const TEXT_INSET_PX: f32 = TEXT_PX * 0.4;
const STROKE_PX: f32 = TEXT_PX / 10.0;

/// One kind's flag on a part: its colour key and how many annotations of
/// the kind name the part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagMark {
    pub tone: AnnotationTone,
    pub count: u32,
}

/// The flags of a design, by designator; each part's marks in kind order.
pub type PartFlags = HashMap<String, Vec<FlagMark>>;

/// The colours flags are drawn in, from the theme tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AnnotationFlagInk {
    pub note: Color,
    pub calculation: Color,
    pub measurement: Color,
    pub issue: Color,
    pub decision: Color,
    /// The number on the pennant.
    pub count: Color,
}

impl AnnotationFlagInk {
    #[must_use]
    pub fn from_tokens(tokens: &signex_types::theme::ThemeTokens) -> Self {
        let kind = |tone| annotation_kind_colour(tone, tokens);
        Self {
            note: kind(AnnotationTone::Note),
            calculation: kind(AnnotationTone::Calculation),
            measurement: kind(AnnotationTone::Measurement),
            issue: kind(AnnotationTone::Issue),
            decision: kind(AnnotationTone::Decision),
            count: annotation_glyph_colour(tokens),
        }
    }

    fn of(&self, tone: AnnotationTone) -> Color {
        match tone {
            AnnotationTone::Note => self.note,
            AnnotationTone::Calculation => self.calculation,
            AnnotationTone::Measurement => self.measurement,
            AnnotationTone::Issue => self.issue,
            AnnotationTone::Decision => self.decision,
        }
    }
}

/// What the canvas needs to draw the flags of the sheet in view: the flags
/// of its project's design (`None`: no flags there) and the inks.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnnotationFlagView<'a> {
    pub parts: Option<&'a PartFlags>,
    pub ink: AnnotationFlagInk,
}

impl AnnotationFlagView<'_> {
    /// Whether any part could carry a flag: the layer then has to draw.
    #[must_use]
    pub fn has_flags(&self) -> bool {
        self.parts.is_some_and(|parts| !parts.is_empty())
    }
}

/// The flag sizes in world mm at `scale` screen px per mm.
fn metrics(scale: f32) -> AnnotationFlagMetrics {
    let mm = |px: f32| screen_px_to_world_mm(px, scale) as f32;
    AnnotationFlagMetrics {
        pole_mm: mm(POLE_PX),
        pennant_width_mm: mm(PENNANT_WIDTH_PX),
        pennant_height_mm: mm(PENNANT_HEIGHT_PX),
        notch_mm: mm(NOTCH_PX),
        stroke_mm: mm(STROKE_PX),
        text_mm: mm(TEXT_PX),
        text_inset_mm: mm(TEXT_INSET_PX),
        step_mm: mm(STEP_PX),
    }
}

/// The flags of `snapshot`'s parts: a part is matched by the designator the
/// canvas draws for it (`designators`, else its own reference), so a sheet
/// reused under several placements flags each placement's parts. Power ports
/// carry none.
pub(crate) fn flag_inputs(
    snapshot: &SchematicRenderSnapshot,
    view: &AnnotationFlagView<'_>,
    designators: &Designators,
    m: &AnnotationFlagMetrics,
) -> Vec<AnnotationFlagInput> {
    let Some(parts) = view.parts.filter(|parts| !parts.is_empty()) else {
        return Vec::new();
    };
    let count = to_rgba(view.ink.count);
    let mut flags = Vec::new();
    for symbol in snapshot.symbols.iter().filter(|s| !s.is_power) {
        let shown = designators
            .get(&symbol.uuid)
            .map_or(symbol.reference.as_str(), String::as_str);
        let Some(marks) = parts.get(shown).filter(|marks| !marks.is_empty()) else {
            continue;
        };
        // Top-right corner of the part; world y grows downward.
        let bbox = symbol_bounds(snapshot, symbol);
        let first = [bbox.max_x as f32, bbox.min_y as f32];
        for (mark, foot) in marks.iter().zip(flag_feet(first, marks.len(), m)) {
            flags.push(AnnotationFlagInput {
                foot,
                count: mark.count,
                color: to_rgba(view.ink.of(mark.tone)),
                text_color: count,
            });
        }
    }
    flags
}

/// Draw the flags of `snapshot`'s parts.
pub fn draw_annotation_flags(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    view: &AnnotationFlagView<'_>,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    designators: &Designators,
) {
    let m = metrics(transform.scale);
    let flags = flag_inputs(snapshot, view, designators, &m);
    if flags.is_empty() {
        return;
    }
    draw_renderer_snapshot(
        frame,
        &annotation_flag_snapshot(&flags, &m),
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::OVERLAY | DirtyFlags::TEXT,
        transform,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn part(id: u128, reference: &str, x: f64) -> Symbol {
        Symbol {
            uuid: uuid::Uuid::from_u128(id),
            lib_id: "Device:R".into(),
            reference: reference.into(),
            position: Point::new(x, 0.0),
            ..Symbol::default()
        }
    }

    fn sheet() -> SchematicRenderSnapshot {
        let mut port = part(3, "#PWR1", 40.0);
        port.is_power = true;
        SchematicSheet {
            symbols: vec![part(1, "U3", 0.0), part(2, "R1", 20.0), port],
            ..SchematicSheet::default()
        }
    }

    fn marks(list: &[(AnnotationTone, u32)]) -> Vec<FlagMark> {
        list.iter()
            .map(|&(tone, count)| FlagMark { tone, count })
            .collect()
    }

    fn ink() -> AnnotationFlagInk {
        AnnotationFlagInk::from_tokens(&theme_tokens(ThemeId::GraphiteLight))
    }

    #[test]
    fn a_named_part_carries_one_flag_per_kind_at_its_top_right_corner() {
        let mut parts = PartFlags::new();
        parts.insert(
            "U3".into(),
            marks(&[(AnnotationTone::Issue, 2), (AnnotationTone::Decision, 1)]),
        );
        let view = AnnotationFlagView {
            parts: Some(&parts),
            ink: ink(),
        };
        let snapshot = sheet();
        let m = metrics(4.0);
        let flags = flag_inputs(&snapshot, &view, &Designators::new(), &m);
        assert_eq!(flags.len(), 2);
        let bbox = symbol_bounds(&snapshot, &snapshot.symbols[0]);
        assert_eq!(flags[0].foot, [bbox.max_x as f32, bbox.min_y as f32]);
        assert_eq!(flags[1].foot[0], flags[0].foot[0] + m.step_mm);
        assert_eq!(
            [flags[0].count, flags[1].count],
            [2, 1],
            "the counts, kind order"
        );
        assert_eq!(flags[0].color, to_rgba(view.ink.issue));
        assert_eq!(flags[1].color, to_rgba(view.ink.decision));
        assert_eq!(flags[0].text_color, to_rgba(view.ink.count));
    }

    #[test]
    fn a_part_is_matched_by_the_designator_the_canvas_draws() {
        let mut parts = PartFlags::new();
        parts.insert("U7".into(), marks(&[(AnnotationTone::Note, 1)]));
        parts.insert("U3".into(), marks(&[(AnnotationTone::Issue, 1)]));
        let view = AnnotationFlagView {
            parts: Some(&parts),
            ink: ink(),
        };
        // This placement draws U3 as U7.
        let mut shown = Designators::new();
        shown.insert(uuid::Uuid::from_u128(1), "U7".into());
        let flags = flag_inputs(&sheet(), &view, &shown, &metrics(4.0));
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].color, to_rgba(view.ink.note));
    }

    #[test]
    fn no_flags_without_a_named_part_or_without_annotations() {
        let none = AnnotationFlagView {
            parts: None,
            ink: ink(),
        };
        assert!(!none.has_flags());
        assert!(flag_inputs(&sheet(), &none, &Designators::new(), &metrics(4.0)).is_empty());
        let mut parts = PartFlags::new();
        parts.insert("Q9".into(), marks(&[(AnnotationTone::Issue, 1)]));
        parts.insert("#PWR1".into(), marks(&[(AnnotationTone::Issue, 1)]));
        let elsewhere = AnnotationFlagView {
            parts: Some(&parts),
            ink: ink(),
        };
        assert!(elsewhere.has_flags());
        assert!(
            flag_inputs(&sheet(), &elsewhere, &Designators::new(), &metrics(4.0)).is_empty(),
            "a part on another sheet and a power port carry no flag"
        );
    }

    #[test]
    fn a_flag_keeps_its_screen_size_at_any_zoom() {
        let (near, far) = (metrics(8.0), metrics(2.0));
        assert!((near.pole_mm * 8.0 - POLE_PX).abs() < 1e-3);
        assert!((far.pole_mm * 2.0 - POLE_PX).abs() < 1e-3);
        assert!((near.text_mm * 8.0 - TEXT_PX).abs() < 1e-3);
    }
}
