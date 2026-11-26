//! Notebook badges on the sheet (mockup row 197): every part and net label
//! the Design Notebook writes about carries one badge at its top-right
//! corner with how many blocks name it, coloured by the worst status of
//! those blocks (`signex_notebook::worst_tone`): blue for notes and
//! calculations only, green when every measurement passes, amber for an
//! open issue, red for a failing measurement. Drawn in the selection layer,
//! so the badges follow the camera, are editor-only and never reach a print
//! or an export. The pole-and-pennant geometry is
//! `signex_renderer::schematic::annotation_flags`; the rules are in
//! `docs/NOTEBOOK_ANNOTATIONS.md`, "Notebook badges".

use super::*;
use signex_notebook::{Badge, BadgeTarget, BadgeTone};
use signex_renderer::schematic::annotation_flags::annotation_flag_snapshot;
use signex_renderer::schematic::{AnnotationFlagInput, AnnotationFlagMetrics};
use signex_widgets::theme_ext::{
    TypeScale, accent, annotation_glyph_colour, error_color, success_color, to_color, warning_color,
};

const TS: TypeScale = TypeScale::DEFAULT;
/// The number on a badge; the badge is sized from it (a 10 px number on a
/// 16 px pennant, 28 px wide on a 28 px pole).
const TEXT_PX: f32 = TS.caption;
const PENNANT_HEIGHT_PX: f32 = TEXT_PX * 1.6;
const PENNANT_WIDTH_PX: f32 = TEXT_PX * 2.8;
const POLE_PX: f32 = TEXT_PX * 2.8;
const NOTCH_PX: f32 = TEXT_PX * 0.6;
const STEP_PX: f32 = TEXT_PX * 4.0;
const TEXT_INSET_PX: f32 = TEXT_PX * 0.4;
const STROKE_PX: f32 = TEXT_PX / 10.0;

/// One badge: its colour key and how many blocks name its part or net.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeMark {
    pub tone: BadgeTone,
    pub count: u32,
}

/// The badges of a design: parts by designator, nets by name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NotebookBadges {
    pub parts: HashMap<String, BadgeMark>,
    pub nets: HashMap<String, BadgeMark>,
}

impl NotebookBadges {
    /// The marks of the notebook's `badges`.
    #[must_use]
    pub fn from_badges(badges: &[Badge]) -> Self {
        let mut out = Self::default();
        for badge in badges {
            let mark = BadgeMark {
                tone: badge.tone,
                count: u32::try_from(badge.count()).unwrap_or(u32::MAX),
            };
            let map = match &badge.target {
                BadgeTarget::Part(_) => &mut out.parts,
                BadgeTarget::Net(_) => &mut out.nets,
            };
            map.insert(badge.target.name().to_string(), mark);
        }
        out
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty() && self.nets.is_empty()
    }
}

/// The colours badges are drawn in, from the theme tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NotebookBadgeInk {
    pub notes: Color,
    pub pass: Color,
    pub open_issue: Color,
    pub failing: Color,
    /// The number on the pennant.
    pub count: Color,
}

impl NotebookBadgeInk {
    #[must_use]
    pub fn from_tokens(tokens: &signex_types::theme::ThemeTokens) -> Self {
        let notes = tokens
            .extended
            .info
            .as_ref()
            .map_or_else(|| accent(tokens), to_color);
        Self {
            notes,
            pass: success_color(tokens),
            open_issue: warning_color(tokens),
            failing: error_color(tokens),
            count: annotation_glyph_colour(tokens),
        }
    }

    pub fn of(&self, tone: BadgeTone) -> Color {
        match tone {
            BadgeTone::Notes => self.notes,
            BadgeTone::Pass => self.pass,
            BadgeTone::OpenIssue => self.open_issue,
            BadgeTone::Failing => self.failing,
        }
    }
}

/// What the canvas needs to draw the badges of the sheet in view: the
/// badges of its project's design (`None`: none there, or View ▸ Notebook
/// Badges is off) and the inks.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotebookBadgeView<'a> {
    pub badges: Option<&'a NotebookBadges>,
    pub ink: NotebookBadgeInk,
}

impl NotebookBadgeView<'_> {
    /// Whether any part or net could carry a badge: the layer then draws.
    #[must_use]
    pub fn has_badges(&self) -> bool {
        self.badges.is_some_and(|badges| !badges.is_empty())
    }
}

/// The badge sizes in world mm at `scale` screen px per mm.
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

/// One badge placed on the sheet: what it is on, where its pole stands
/// and its mark.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlacedBadge {
    pub target: BadgeTarget,
    pub foot: [f32; 2],
    pub mark: BadgeMark,
}

/// The badges of `snapshot`'s parts and net labels. A part is matched by
/// the designator the canvas draws for it (`designators`, else its own
/// reference), so a sheet reused under several placements badges each
/// placement's parts; power ports carry none. A net carries its badge on
/// each of its labels.
pub(crate) fn placed_badges(
    snapshot: &SchematicRenderSnapshot,
    badges: &NotebookBadges,
    designators: &Designators,
) -> Vec<PlacedBadge> {
    let mut placed = Vec::new();
    for symbol in snapshot.symbols.iter().filter(|s| !s.is_power) {
        let shown = designators
            .get(&symbol.uuid)
            .map_or(symbol.reference.as_str(), String::as_str);
        let Some(mark) = badges.parts.get(shown) else {
            continue;
        };
        // Top-right corner of the part; world y grows downward.
        let bbox = symbol_bounds(snapshot, symbol);
        placed.push(PlacedBadge {
            target: BadgeTarget::Part(shown.to_string()),
            foot: [bbox.max_x as f32, bbox.min_y as f32],
            mark: *mark,
        });
    }
    let family = snapshot.font_family.as_deref();
    for label in &snapshot.labels {
        let Some(mark) = badges.nets.get(&label.text) else {
            continue;
        };
        let bbox = label_aabb_with(Some(WidthSources::of(snapshot)), label, family);
        placed.push(PlacedBadge {
            target: BadgeTarget::Net(label.text.clone()),
            foot: [bbox.max_x as f32, bbox.min_y as f32],
            mark: *mark,
        });
    }
    placed
}

/// The pennant-and-pole inputs of `placed`.
fn badge_inputs(placed: &[PlacedBadge], ink: &NotebookBadgeInk) -> Vec<AnnotationFlagInput> {
    let count = to_rgba(ink.count);
    placed
        .iter()
        .map(|badge| AnnotationFlagInput {
            foot: badge.foot,
            count: badge.mark.count,
            color: to_rgba(ink.of(badge.mark.tone)),
            text_color: count,
        })
        .collect()
}

/// The badge under the world point `at`, at `scale` screen px per mm: the
/// pole and the pennant both count. The last drawn wins.
pub(crate) fn badge_at(placed: &[PlacedBadge], at: [f32; 2], scale: f32) -> Option<&PlacedBadge> {
    let m = metrics(scale);
    placed.iter().rev().find(|badge| {
        let [x, y] = badge.foot;
        let inside_x = at[0] >= x - m.stroke_mm && at[0] <= x + m.pennant_width_mm;
        let inside_y = at[1] >= y - m.pole_mm && at[1] <= y;
        inside_x && inside_y
    })
}

/// Draw the badges of `snapshot`'s parts and net labels.
pub fn draw_notebook_badges(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    view: &NotebookBadgeView<'_>,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    designators: &Designators,
) {
    let Some(badges) = view.badges.filter(|badges| !badges.is_empty()) else {
        return;
    };
    let placed = placed_badges(snapshot, badges, designators);
    if placed.is_empty() {
        return;
    }
    let m = metrics(transform.scale);
    draw_renderer_snapshot(
        frame,
        &annotation_flag_snapshot(&badge_inputs(&placed, &view.ink), &m),
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
            labels: vec![Label::new("VBUS", Point::new(60.0, 10.0), LabelType::Net)],
            ..SchematicSheet::default()
        }
    }

    fn mark(tone: BadgeTone, count: u32) -> BadgeMark {
        BadgeMark { tone, count }
    }

    fn badges() -> NotebookBadges {
        let mut badges = NotebookBadges::default();
        badges
            .parts
            .insert("U3".into(), mark(BadgeTone::OpenIssue, 3));
        badges.nets.insert("VBUS".into(), mark(BadgeTone::Pass, 1));
        badges
    }

    fn ink() -> NotebookBadgeInk {
        NotebookBadgeInk::from_tokens(&theme_tokens(ThemeId::GraphiteLight))
    }

    #[test]
    fn a_named_part_carries_one_badge_at_its_top_right_corner() {
        let snapshot = sheet();
        let placed = placed_badges(&snapshot, &badges(), &Designators::new());
        let u3 = placed
            .iter()
            .find(|b| b.target == BadgeTarget::Part("U3".into()))
            .expect("U3 has a badge");
        let bbox = symbol_bounds(&snapshot, &snapshot.symbols[0]);
        assert_eq!(u3.foot, [bbox.max_x as f32, bbox.min_y as f32]);
        assert_eq!(
            u3.mark,
            mark(BadgeTone::OpenIssue, 3),
            "one badge, the count"
        );
        let inputs = badge_inputs(&placed, &ink());
        assert_eq!(inputs.len(), 2, "U3 and the VBUS label");
        assert_eq!(inputs[0].color, to_rgba(ink().open_issue));
        assert_eq!(inputs[0].text_color, to_rgba(ink().count));
    }

    #[test]
    fn a_net_label_carries_its_net_badge() {
        let placed = placed_badges(&sheet(), &badges(), &Designators::new());
        let net = placed
            .iter()
            .find(|b| b.target == BadgeTarget::Net("VBUS".into()))
            .expect("the label has a badge");
        assert_eq!(net.mark.tone, BadgeTone::Pass);
        assert!(net.foot[0] >= 60.0, "at the label's right edge");
    }

    #[test]
    fn the_colour_is_the_worst_status_tone() {
        let ink = ink();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        assert_eq!(ink.of(BadgeTone::Pass), success_color(&tokens));
        assert_eq!(ink.of(BadgeTone::OpenIssue), warning_color(&tokens));
        assert_eq!(ink.of(BadgeTone::Failing), error_color(&tokens));
        assert_ne!(ink.of(BadgeTone::Notes), ink.of(BadgeTone::Failing));
    }

    #[test]
    fn a_part_is_matched_by_the_designator_the_canvas_draws() {
        let mut badges = NotebookBadges::default();
        badges.parts.insert("U7".into(), mark(BadgeTone::Notes, 1));
        // This placement draws U3 as U7.
        let mut shown = Designators::new();
        shown.insert(uuid::Uuid::from_u128(1), "U7".into());
        let placed = placed_badges(&sheet(), &badges, &shown);
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].target, BadgeTarget::Part("U7".into()));
    }

    #[test]
    fn no_badges_without_a_named_part_or_while_switched_off() {
        let off = NotebookBadgeView {
            badges: None,
            ink: ink(),
        };
        assert!(!off.has_badges());
        let mut elsewhere = NotebookBadges::default();
        elsewhere
            .parts
            .insert("Q9".into(), mark(BadgeTone::Notes, 1));
        elsewhere
            .parts
            .insert("#PWR1".into(), mark(BadgeTone::Notes, 1));
        assert!(
            placed_badges(&sheet(), &elsewhere, &Designators::new()).is_empty(),
            "a part on another sheet and a power port carry no badge"
        );
    }

    #[test]
    fn the_badge_under_the_pointer_is_found_on_its_pole_or_pennant() {
        let placed = placed_badges(&sheet(), &badges(), &Designators::new());
        let scale = 4.0;
        let m = metrics(scale);
        let u3 = placed[0].foot;
        let on_pennant = [u3[0] + m.pennant_width_mm * 0.5, u3[1] - m.pole_mm + 0.1];
        assert_eq!(
            badge_at(&placed, on_pennant, scale).map(|b| b.target.clone()),
            Some(BadgeTarget::Part("U3".into()))
        );
        let below = [u3[0], u3[1] + 1.0];
        assert!(badge_at(&placed, below, scale).is_none());
    }

    #[test]
    fn marks_come_from_the_notebook_badges() {
        let (blocks, _) = signex_notebook::scan(
            "#annotation(kind: \"issue\", status: \"open\")[#component(\"U3\") on #net(\"VBUS\")]\n",
        );
        let found = signex_notebook::badges([("design.snxnbk", blocks.as_slice())]);
        let marks = NotebookBadges::from_badges(&found);
        assert_eq!(marks.parts.get("U3"), Some(&mark(BadgeTone::OpenIssue, 1)));
        assert_eq!(marks.nets.get("VBUS"), Some(&mark(BadgeTone::OpenIssue, 1)));
        assert!(NotebookBadges::default().is_empty());
    }

    #[test]
    fn a_badge_keeps_its_screen_size_at_any_zoom() {
        let (near, far) = (metrics(8.0), metrics(2.0));
        assert!((near.pole_mm * 8.0 - POLE_PX).abs() < 1e-3);
        assert!((far.pole_mm * 2.0 - POLE_PX).abs() < 1e-3);
        assert!((near.text_mm * 8.0 - TEXT_PX).abs() < 1e-3);
    }
}
