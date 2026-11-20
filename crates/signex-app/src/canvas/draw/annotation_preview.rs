//! The Bezier and dimension tools' live preview (#1267): the clicked points,
//! the curve or the dimension through the cursor, the measured value in an
//! accent tag set off above the dimension line (so it clears text beside
//! the line; DECISIONS 2026-11-19), and the hint bar at the canvas's bottom-left.

use super::super::*;
use signex_types::schematic::{
    DimensionArrows, DimensionDirection, DimensionUnits, Point, SchBezier, SchDimension,
};

/// Which tool the draft belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationDraftKind {
    Bezier,
    LinearDimension,
    AlignedDimension,
}

/// The colours of the hint bar, from the UI theme tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HintInk {
    pub background: Color,
    pub border: Color,
    pub title: Color,
    pub caption: Color,
    pub accent: Color,
    pub on_accent: Color,
}

/// The annotation being placed, mirrored from the app for the overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationDraft {
    pub kind: AnnotationDraftKind,
    /// The points clicked so far.
    pub points: Vec<Point>,
    /// The hint bar's bold tool name, already in the user's language.
    pub title: String,
    /// The hint bar's caption: what the next click does.
    pub hint: String,
    /// The text the dimension will be placed with.
    pub units: DimensionUnits,
    pub precision: u8,
    pub arrows: DimensionArrows,
    pub ink: HintInk,
}

/// Default distance of a dimension line from its points before the third
/// click sets it (mm, above the points).
const DRAFT_OFFSET_MM: f64 = -2.54;
/// Gap between the dimension line and the live tag (px).
const TAG_GAP_PX: f32 = 8.0;
const TAG_HEIGHT_PX: f32 = 16.0;
const TAG_CHAR_PX: f32 = 6.6;
const TAG_PAD_PX: f32 = 8.0;
const HINT_MARGIN_PX: f32 = 10.0;
const HINT_PAD_PX: f32 = 8.0;
const HINT_HEIGHT_PX: f32 = 26.0;
const HINT_FONT_PX: f32 = 12.0;
const HINT_CHAR_PX: f32 = 6.8;

impl AnnotationDraft {
    /// The dimension the cursor at `cursor` would place, for a dimension
    /// draft with at least one point.
    pub fn dimension_at(&self, cursor: Point) -> Option<SchDimension> {
        let from = *self.points.first()?;
        let to = self.points.get(1).copied().unwrap_or(cursor);
        let direction = match self.kind {
            AnnotationDraftKind::Bezier => return None,
            AnnotationDraftKind::LinearDimension => SchDimension::linear_direction(from, to),
            AnnotationDraftKind::AlignedDimension => DimensionDirection::Aligned,
        };
        let mut d = SchDimension::new(uuid::Uuid::nil(), from, to, DRAFT_OFFSET_MM, direction);
        d.units = self.units;
        d.precision = self.precision;
        d.arrows = self.arrows;
        if self.points.len() >= 2 {
            d.offset = d.offset_to(cursor);
        }
        Some(d)
    }

    /// The curve the cursor at `cursor` would close, for a Bezier draft:
    /// the clicked points, then the cursor for every point still missing.
    pub fn bezier_at(&self, cursor: Point) -> Option<SchBezier> {
        if self.kind != AnnotationDraftKind::Bezier || self.points.is_empty() {
            return None;
        }
        let at = |i: usize| self.points.get(i).copied().unwrap_or(cursor);
        Some(SchBezier::new(
            uuid::Uuid::nil(),
            at(0),
            at(1),
            at(2),
            cursor,
        ))
    }
}

impl SchematicCanvas<'_> {
    /// The draft's preview, drawn while its tool is armed.
    pub(in crate::canvas) fn draw_annotation_preview(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        let Some(draft) = &self.annotation_draft else {
            return;
        };
        let cam = self.camera();
        let world = cam.screen_to_world(cursor_pos, bounds);
        let cursor = Point::new(f64::from(world.x), f64::from(world.y));
        let to_screen =
            |p: Point| cam.world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
        let accent = draft.ink.accent;
        let stroke = canvas::Stroke::default().with_color(accent).with_width(1.4);
        let guide = canvas::Stroke::default()
            .with_color(Color { a: 0.5, ..accent })
            .with_width(1.0);
        for p in &draft.points {
            frame.stroke(&canvas::Path::circle(to_screen(*p), 4.0), stroke);
        }
        if let Some(b) = draft.bezier_at(cursor) {
            let [s, c1, c2, e] = b.points().map(to_screen);
            frame.stroke(&canvas::Path::line(s, c1), guide);
            frame.stroke(&canvas::Path::line(e, c2), guide);
            let curve = canvas::Path::new(|path| {
                path.move_to(s);
                path.bezier_curve_to(c1, c2, e);
            });
            frame.stroke(&curve, stroke);
        } else if let Some(d) = draft.dimension_at(cursor) {
            self.draw_dimension_draft(frame, &d, &to_screen, draft);
        }
        draw_hint_bar(frame, bounds, draft);
    }

    fn draw_dimension_draft(
        &self,
        frame: &mut canvas::Frame,
        d: &SchDimension,
        to_screen: &dyn Fn(Point) -> iced::Point,
        draft: &AnnotationDraft,
    ) {
        let accent = draft.ink.accent;
        let stroke = canvas::Stroke::default().with_color(accent).with_width(1.4);
        let g = d.geometry();
        for [a, b] in [g.line, g.extensions[0], g.extensions[1]] {
            frame.stroke(&canvas::Path::line(to_screen(a), to_screen(b)), stroke);
        }
        for head in &g.arrows {
            let tri = canvas::Path::new(|path| {
                path.move_to(to_screen(head[0]));
                path.line_to(to_screen(head[1]));
                path.line_to(to_screen(head[2]));
                path.close();
            });
            frame.fill(&tri, accent);
        }
        // The tag sits off the line on its outer side, never on it.
        let (a, b) = (to_screen(g.line[0]), to_screen(g.line[1]));
        let mid = iced::Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let at = to_screen(g.text_at);
        let (dx, dy) = (at.x - mid.x, at.y - mid.y);
        let len = dx.hypot(dy).max(1e-3);
        let reach = TAG_GAP_PX + TAG_HEIGHT_PX / 2.0;
        let centre = iced::Point::new(mid.x + dx / len * reach, mid.y + dy / len * reach);
        let label = d.label();
        let width = label.chars().count() as f32 * TAG_CHAR_PX + 2.0 * TAG_PAD_PX;
        let chip = canvas::Path::rounded_rectangle(
            iced::Point::new(centre.x - width / 2.0, centre.y - TAG_HEIGHT_PX / 2.0),
            iced::Size::new(width, TAG_HEIGHT_PX),
            3.0_f32.into(),
        );
        frame.fill(&chip, accent);
        frame.fill_text(canvas::Text {
            content: label,
            position: centre,
            color: draft.ink.on_accent,
            size: iced::Pixels(10.0),
            font: crate::render_config::IOSEVKA,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    }
}

/// The bar at the canvas's bottom-left: the tool's name, then what the next
/// click does.
fn draw_hint_bar(frame: &mut canvas::Frame, bounds: Rectangle, draft: &AnnotationDraft) {
    let title_w = draft.title.chars().count() as f32 * HINT_CHAR_PX;
    let hint_w = draft.hint.chars().count() as f32 * HINT_CHAR_PX;
    let width = title_w + hint_w + 3.0 * HINT_PAD_PX;
    let top_left = iced::Point::new(
        HINT_MARGIN_PX,
        bounds.height - HINT_MARGIN_PX - HINT_HEIGHT_PX,
    );
    let bar = canvas::Path::rounded_rectangle(
        top_left,
        iced::Size::new(width, HINT_HEIGHT_PX),
        4.0_f32.into(),
    );
    frame.fill(&bar, draft.ink.background);
    frame.stroke(
        &bar,
        canvas::Stroke::default()
            .with_color(draft.ink.border)
            .with_width(1.0),
    );
    let y = top_left.y + HINT_HEIGHT_PX / 2.0;
    frame.fill_text(canvas::Text {
        content: draft.title.clone(),
        position: iced::Point::new(top_left.x + HINT_PAD_PX, y),
        color: draft.ink.title,
        size: iced::Pixels(HINT_FONT_PX),
        font: iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::DEFAULT
        },
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
    frame.fill_text(canvas::Text {
        content: draft.hint.clone(),
        position: iced::Point::new(top_left.x + 2.0 * HINT_PAD_PX + title_w, y),
        color: draft.ink.caption,
        size: iced::Pixels(HINT_FONT_PX),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}
