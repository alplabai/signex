//! The "Grid and snap" illustration of Document Options ▸ General (#2531):
//! the visible-grid dots, the snap ticks with a snapped cursor and a pin end
//! with its hotspot ring. The two pitches are callouts named after their
//! fields; a grid that is switched off is not drawn.

use iced::{Point, Rectangle, Size};
use signex_i18n::{I18n, ids, t};
use signex_types::coord::Unit;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, Tint,
    Weight,
};

/// The Visible Grid row and its on/off box.
pub const VISIBLE_GRID: FieldId = FieldId::new("doc-visible-grid");
/// The Snap Grid row and its on/off box.
pub const SNAP_GRID: FieldId = FieldId::new("doc-snap-grid");
/// The Snap to Hotspots row.
pub const HOTSPOTS: FieldId = FieldId::new("doc-snap-hotspots");

/// View box of the drawing.
const VIEW: Size = Size::new(264.0, 140.0);
/// Spacing of the visible-grid dots, in view units.
const DOT_STEP: f32 = 24.0;
const GRID_LEFT: f32 = 16.0;
const GRID_TOP: f32 = 32.0;
const DOT_COLUMNS: usize = 6;
const DOT_ROWS: usize = 3;
const DOT_RADIUS: f32 = 1.4;
/// Snap ticks are kept between these spacings so they stay readable.
const MIN_TICK_STEP: f32 = 6.0;
const MAX_TICK_STEP: f32 = 48.0;
const TICK_BASE: f32 = 108.0;
const TICK_LENGTH: f32 = 6.0;
/// The two pitch dimension lines, and the bottoms of their labels, which
/// sit clear of the line on the side away from the grid.
const VISIBLE_LINE_Y: f32 = 20.0;
const VISIBLE_LABEL_BOTTOM: f32 = 15.0;
const SNAP_LINE_Y: f32 = 116.0;
const SNAP_LABEL_BOTTOM: f32 = 134.0;
const CURSOR_ARM: f32 = 7.0;
const PIN_END: Point = Point::new(176.0, 78.0);
const PIN_LENGTH: f32 = 24.0;
const BODY_SIZE: Size = Size::new(40.0, 40.0);
const HOTSPOT_RADIUS: f32 = 6.0;

/// What the illustration shows: the General tab's grid settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridFacts {
    pub unit: Unit,
    pub visible_grid_mm: f32,
    pub snap_grid_mm: f32,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub snap_hotspots: bool,
}

/// A pitch as the grid pick lists name it ("2.54 mm", "100 mil"), else the
/// value in the unit.
pub fn pitch_text(mm: f32, unit: Unit) -> String {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};
    let labels = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };
    let named = GRID_SIZES_MM
        .iter()
        .zip(labels.iter())
        .find(|(size, _)| (**size - mm).abs() < 1e-4)
        .map(|(_, label)| (*label).to_string());
    named.unwrap_or_else(|| match unit {
        Unit::Mil => format!("{:.0} mil", mm / 0.0254),
        _ => format!("{mm:.3} mm"),
    })
}

/// Spacing of the snap ticks: the snap pitch drawn against the dot spacing.
fn tick_step(facts: &GridFacts) -> f32 {
    if facts.visible_grid_mm <= 0.0 || facts.snap_grid_mm <= 0.0 {
        return DOT_STEP;
    }
    (DOT_STEP * facts.snap_grid_mm / facts.visible_grid_mm).clamp(MIN_TICK_STEP, MAX_TICK_STEP)
}

fn dots() -> impl Iterator<Item = Part> {
    (0..DOT_ROWS).flat_map(|row| {
        (0..DOT_COLUMNS).map(move |column| {
            let center = Point::new(
                GRID_LEFT + DOT_STEP * column as f32,
                GRID_TOP + DOT_STEP * row as f32,
            );
            Part::new(
                Primitive::Circle {
                    center,
                    radius: DOT_RADIUS,
                },
                Style::filled(Paint::Ink(Ink::Tertiary), Tint::Solid),
            )
            .linked(VISIBLE_GRID)
            .requires(VISIBLE_GRID)
        })
    })
}

fn ticks(step: f32) -> impl Iterator<Item = Part> {
    let right = GRID_LEFT + DOT_STEP * (DOT_COLUMNS - 1) as f32;
    let count = ((right - GRID_LEFT) / step).floor() as usize;
    (0..=count).map(move |i| {
        let x = GRID_LEFT + step * i as f32;
        Part::new(
            Primitive::Line {
                from: Point::new(x, TICK_BASE - TICK_LENGTH),
                to: Point::new(x, TICK_BASE),
            },
            Style::outline(Ink::Secondary, Weight::Thin),
        )
        .linked(SNAP_GRID)
        .requires(SNAP_GRID)
    })
}

/// The cursor, on the snap point nearest the middle of the dots.
fn cursor(step: f32) -> Vec<Part> {
    let k = ((DOT_STEP * 2.5) / step).round();
    let at = Point::new(GRID_LEFT + step * k, GRID_TOP + DOT_STEP);
    let arm = |dx: f32, dy: f32| {
        Part::new(
            Primitive::Line {
                from: Point::new(at.x - dx, at.y - dy),
                to: Point::new(at.x + dx, at.y + dy),
            },
            Style::outline(Ink::Secondary, Weight::Regular),
        )
        .linked(SNAP_GRID)
    };
    vec![arm(CURSOR_ARM, 0.0), arm(0.0, CURSOR_ARM)]
}

/// A part body with one pin; its end carries the hotspot ring.
fn pin(i18n: &I18n) -> Vec<Part> {
    let body = Rectangle::new(
        Point::new(PIN_END.x + PIN_LENGTH, PIN_END.y - BODY_SIZE.height / 2.0),
        BODY_SIZE,
    );
    vec![
        Part::new(
            Primitive::Rect { rect: body },
            Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Wash),
        ),
        Part::new(
            Primitive::Line {
                from: PIN_END,
                to: Point::new(PIN_END.x + PIN_LENGTH, PIN_END.y),
            },
            Style::edge(),
        ),
        Part::new(
            Primitive::Circle {
                center: PIN_END,
                radius: HOTSPOT_RADIUS,
            },
            Style::outline(Ink::Accent, Weight::Regular),
        )
        .linked(HOTSPOTS)
        .requires(HOTSPOTS),
        Part::leader(Leader {
            at: Point::new(PIN_END.x, PIN_END.y + HOTSPOT_RADIUS),
            label_at: Point::new(PIN_END.x, PIN_END.y + 30.0),
            label: t!(i18n, ids::PROPERTIES_ILLUSTRATION_HOTSPOT).into(),
            anchor: Anchor::Middle,
        })
        .linked(HOTSPOTS)
        .requires(HOTSPOTS),
    ]
}

/// The Grid and snap illustration for `facts`.
pub fn grid_illustration(facts: &GridFacts, i18n: &I18n) -> Illustration {
    let step = tick_step(facts);
    let visible = t!(
        i18n,
        ids::PROPERTIES_ILLUSTRATION_VISIBLE_GRID,
        "pitch" => pitch_text(facts.visible_grid_mm, facts.unit)
    );
    let snap = t!(
        i18n,
        ids::PROPERTIES_ILLUSTRATION_SNAP_GRID,
        "pitch" => pitch_text(facts.snap_grid_mm, facts.unit)
    );
    let visible_callout = Callout::new(
        Point::new(GRID_LEFT, VISIBLE_LINE_Y),
        Point::new(GRID_LEFT + DOT_STEP, VISIBLE_LINE_Y),
    )
    .label(visible)
    .label_at(Point::new(GRID_LEFT, VISIBLE_LABEL_BOTTOM), Anchor::Start)
    .extension(
        Point::new(GRID_LEFT, GRID_TOP - 3.0),
        Point::new(GRID_LEFT, VISIBLE_LINE_Y - 3.0),
    )
    .extension(
        Point::new(GRID_LEFT + DOT_STEP, GRID_TOP - 3.0),
        Point::new(GRID_LEFT + DOT_STEP, VISIBLE_LINE_Y - 3.0),
    );
    let snap_callout = Callout::new(
        Point::new(GRID_LEFT, SNAP_LINE_Y),
        Point::new(GRID_LEFT + step, SNAP_LINE_Y),
    )
    .label(snap)
    .label_at(Point::new(GRID_LEFT, SNAP_LABEL_BOTTOM), Anchor::Start);
    Illustration::new(VIEW.width, VIEW.height)
        .parts(dots())
        .parts(ticks(step))
        .parts(cursor(step))
        .parts(pin(i18n))
        .part(
            Part::callout(visible_callout)
                .linked(VISIBLE_GRID)
                .requires(VISIBLE_GRID),
        )
        .part(
            Part::callout(snap_callout)
                .linked(SNAP_GRID)
                .requires(SNAP_GRID),
        )
        .feature(VISIBLE_GRID, facts.grid_visible)
        .feature(SNAP_GRID, facts.snap_enabled)
        .feature(HOTSPOTS, facts.snap_hotspots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> GridFacts {
        GridFacts {
            unit: Unit::Mm,
            visible_grid_mm: 2.54,
            snap_grid_mm: 1.27,
            grid_visible: true,
            snap_enabled: true,
            snap_hotspots: true,
        }
    }

    fn shows(illustration: &Illustration, field: &FieldId) -> bool {
        illustration.visible_parts().any(|p| p.is_linked_to(field))
    }

    fn callout_labels(illustration: &Illustration) -> Vec<String> {
        illustration
            .visible_parts()
            .filter_map(|p| match &p.primitive {
                Primitive::Callout(c) => c.label.as_ref().map(|l| l.as_str().to_string()),
                _ => None,
            })
            .collect()
    }

    /// Both pitches are callouts named after their fields.
    #[test]
    fn the_pitches_are_callouts_named_after_their_fields() {
        let i18n = I18n::en_us();
        let labels = callout_labels(&grid_illustration(&facts(), &i18n));
        let plain: Vec<String> = labels
            .iter()
            .map(|l| l.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert!(
            plain.contains(&"Visible Grid 2.54 mm".to_string()),
            "{plain:?}"
        );
        assert!(
            plain.contains(&"Snap Grid 1.27 mm".to_string()),
            "{plain:?}"
        );
    }

    /// A switched-off grid drops out of the drawing; the other stays.
    #[test]
    fn an_off_grid_is_not_drawn() {
        let i18n = I18n::en_us();
        let on = grid_illustration(&facts(), &i18n);
        assert!(shows(&on, &VISIBLE_GRID) && shows(&on, &SNAP_GRID));
        let off = grid_illustration(
            &GridFacts {
                grid_visible: false,
                ..facts()
            },
            &i18n,
        );
        assert!(!shows(&off, &VISIBLE_GRID), "the dots are gone");
        assert!(shows(&off, &SNAP_GRID), "the snap ticks stay");
        let no_snap = grid_illustration(
            &GridFacts {
                snap_enabled: false,
                snap_hotspots: false,
                ..facts()
            },
            &i18n,
        );
        let ticks = no_snap
            .visible_parts()
            .filter(|p| p.requires.as_ref() == Some(&SNAP_GRID))
            .count();
        assert_eq!(ticks, 0, "no snap ticks or snap callout");
        assert!(!shows(&no_snap, &HOTSPOTS));
    }

    #[test]
    fn a_pitch_reads_as_the_pick_list_names_it() {
        assert_eq!(pitch_text(2.54, Unit::Mm), "2.54 mm");
        assert_eq!(pitch_text(2.54, Unit::Mil), "100 mil");
        assert_eq!(pitch_text(0.5, Unit::Mm), "0.500 mm");
    }
}
