//! The live drawing of the sync dialog (mockup row 79): the written side as
//! a sheet symbol, after Apply or as it is now. Pure: built from the entries
//! and the size; entry names are not editable here.

use iced::{Color, Point, Rectangle, Size};
use signex_engine::{SheetEntryChange, SheetSyncEntry};
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Illustration, Ink, Paint, Part, Primitive, Style, Text, TextLabel, TextRole, Tint,
    Weight,
};

use crate::app::sheet_sync::SheetSyncPreviewMode;

/// Least height of the drawing in captions: about 200 px, taller expanded.
const HEIGHT_EM: f32 = 18.0;
const EXPANDED_HEIGHT_EM: f32 = 30.0;
/// Vertical pitch of the entries, and the room above and below the box.
const PITCH_EM: f32 = 2.6;
const MARGIN_EM: f32 = 2.6;
/// Half the height of an entry flag, and how far its point reaches.
const FLAG_HALF_EM: f32 = 0.75;
const FLAG_TIP_EM: f32 = 0.7;
/// Width of a character of a mono entry name, and the flag's side padding.
const CHAR_EM: f32 = 0.62;
const FLAG_PAD_EM: f32 = 1.8;
/// Gap between the two columns of flags inside the box.
const COLUMN_GAP_EM: f32 = 2.0;
/// Length of the wire stub on an entry that exists today.
const STUB_EM: f32 = 4.0;
/// Gap between the box and the title above it or the file under it.
const TITLE_GAP_EM: f32 = 0.9;

/// The drawing's size in px and the text size its metrics derive from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreviewSize {
    pub width: f32,
    pub height: f32,
    pub text_size: f32,
}

impl PreviewSize {
    /// `width` wide and tall enough for `rows` rows of entries.
    pub(super) fn new(width: f32, expanded: bool, rows: usize, text_size: f32) -> Self {
        let least = if expanded {
            EXPANDED_HEIGHT_EM
        } else {
            HEIGHT_EM
        };
        let needed = 2.0 * MARGIN_EM + PITCH_EM * (rows.max(1) + 1) as f32;
        Self {
            width,
            height: least.max(needed) * text_size,
            text_size,
        }
    }

    fn em(&self, ems: f32) -> f32 {
        ems * self.text_size
    }
}

/// Which way an entry flag points, from its I/O type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Points {
    inward: bool,
    outward: bool,
}

fn points_of(direction: &str) -> Points {
    match direction {
        "input" => Points {
            inward: true,
            outward: false,
        },
        "output" => Points {
            inward: false,
            outward: true,
        },
        "passive" => Points {
            inward: false,
            outward: false,
        },
        _ => Points {
            inward: true,
            outward: true,
        },
    }
}

/// How one entry is drawn in the current mode.
struct Shown<'e> {
    entry: &'e SheetSyncEntry,
    name: &'e str,
    /// The current name over a renamed entry, after Apply.
    was: Option<&'e str>,
}

/// The entries the mode draws: Before leaves out what Apply would add and
/// shows a rename under its current name.
fn shown(entries: &[SheetSyncEntry], mode: SheetSyncPreviewMode) -> Vec<Shown<'_>> {
    entries
        .iter()
        .filter_map(|entry| match (&entry.change, mode) {
            (SheetEntryChange::Added, SheetSyncPreviewMode::Before) => None,
            (SheetEntryChange::Renamed { from }, SheetSyncPreviewMode::Before) => Some(Shown {
                entry,
                name: from.as_str(),
                was: None,
            }),
            (SheetEntryChange::Renamed { from }, SheetSyncPreviewMode::After) => Some(Shown {
                entry,
                name: entry.name.as_str(),
                was: Some(from.as_str()),
            }),
            _ => Some(Shown {
                entry,
                name: entry.name.as_str(),
                was: None,
            }),
        })
        .collect()
}

/// Rows the drawing needs for `entries`: the taller of its two columns.
pub(super) fn rows_of(entries: &[SheetSyncEntry], mode: SheetSyncPreviewMode) -> usize {
    let drawn = shown(entries, mode);
    let right = drawn
        .iter()
        .filter(|s| s.entry.direction == "output")
        .count();
    right.max(drawn.len() - right)
}

/// Stroke and fill of a flag, from its change. Before draws every flag
/// plain: nothing has changed yet.
fn flag_style(change: &SheetEntryChange, mode: SheetSyncPreviewMode, added: Color) -> Style {
    if mode == SheetSyncPreviewMode::Before {
        return Style::edge();
    }
    match change {
        SheetEntryChange::Kept => Style::edge(),
        SheetEntryChange::Added => Style::outline(Ink::Secondary, Weight::Regular)
            .with_stroke_paint(Paint::Document(added))
            .with_fill(Paint::Document(added), Tint::Body),
        SheetEntryChange::Renamed { .. } => Style::outline(Ink::Accent, Weight::Regular)
            .with_fill(Paint::Ink(Ink::Accent), Tint::Body),
        SheetEntryChange::Removed => {
            let mut style = Style::dashed().with_stroke_paint(Paint::Ink(Ink::Error));
            if let Some(stroke) = style.stroke.as_mut() {
                stroke.weight = Weight::Regular;
            }
            style
        }
    }
}

/// The outline of a flag from `edge` (on the box) `width` into the box;
/// `into_box` is +1 on the left side and -1 on the right.
fn flag_outline(
    edge: f32,
    y: f32,
    width: f32,
    into_box: f32,
    points: Points,
    size: PreviewSize,
) -> Vec<Point> {
    let (half, tip) = (size.em(FLAG_HALF_EM), size.em(FLAG_TIP_EM));
    let inner = edge + into_box * width;
    let inner_side = if points.inward {
        inner - into_box * tip
    } else {
        inner
    };
    let outer_side = if points.outward {
        edge + into_box * tip
    } else {
        edge
    };
    let mut outline = vec![
        Point::new(outer_side, y - half),
        Point::new(inner_side, y - half),
    ];
    if points.inward {
        outline.push(Point::new(inner, y));
    }
    outline.push(Point::new(inner_side, y + half));
    outline.push(Point::new(outer_side, y + half));
    if points.outward {
        outline.push(Point::new(edge, y));
    }
    outline
}

fn label(at: Point, text: Text, anchor: Anchor, role: TextRole, ink: Ink) -> Part {
    Part::label(
        TextLabel {
            at,
            text,
            anchor,
            role,
        },
        ink,
    )
}

/// One entry at row `y` on the side whose edge is `edge`.
fn entry_parts(
    parts: &mut Vec<Part>,
    item: &Shown<'_>,
    (edge, into_box, y, width): (f32, f32, f32, f32),
    mode: SheetSyncPreviewMode,
    added: Color,
    size: PreviewSize,
    i18n: &I18n,
) {
    let change = &item.entry.change;
    if matches!(
        change,
        SheetEntryChange::Kept | SheetEntryChange::Renamed { .. }
    ) {
        let line = Primitive::Line {
            from: Point::new(edge, y),
            to: Point::new(edge - into_box * size.em(STUB_EM), y),
        };
        parts.push(Part::new(line, Style::outline(Ink::Tertiary, Weight::Thin)));
    }
    let points = points_of(item.entry.direction);
    let outline = flag_outline(edge, y, width, into_box, points, size);
    parts.push(Part::new(
        Primitive::Polyline {
            points: outline,
            is_closed: true,
        },
        flag_style(change, mode, added),
    ));
    let middle = Point::new(edge + into_box * width / 2.0, y);
    let ink = match (change, mode) {
        (SheetEntryChange::Removed, SheetSyncPreviewMode::After) => Ink::Error,
        (SheetEntryChange::Renamed { .. }, SheetSyncPreviewMode::After) => Ink::Accent,
        _ => Ink::Secondary,
    };
    let name: Text = UserText::new(item.name.to_string()).into();
    parts.push(label(middle, name, Anchor::Middle, TextRole::Mono, ink));
    if mode == SheetSyncPreviewMode::After && matches!(change, SheetEntryChange::Removed) {
        let reach = width / 2.0 - size.em(FLAG_TIP_EM);
        let strike = Primitive::Line {
            from: Point::new(middle.x - reach, y),
            to: Point::new(middle.x + reach, y),
        };
        parts.push(Part::new(strike, Style::outline(Ink::Error, Weight::Thin)));
    }
    if let Some(was) = item.was {
        let over = Point::new(middle.x, y - size.em(FLAG_HALF_EM + 0.6));
        let words: Text = t!(i18n, ids::SHEET_SYNC_PREVIEW_WAS, "name" => was).into();
        parts.push(label(
            over,
            words,
            Anchor::Middle,
            TextRole::Note,
            Ink::Faint,
        ));
    }
}

/// Width of the flags of one column, for its longest name.
fn flag_width(names: impl Iterator<Item = usize>, size: PreviewSize) -> f32 {
    let longest = names.max().unwrap_or(0) as f32;
    size.em(longest * CHAR_EM + FLAG_PAD_EM)
}

/// The symbol's box with `title` above it and `footer` under it.
fn box_parts(body: Rectangle, title: &str, footer: &str, size: PreviewSize) -> Vec<Part> {
    let gap = size.em(TITLE_GAP_EM);
    let name: Text = UserText::new(title).into();
    let file: Text = UserText::new(footer).into();
    vec![
        Part::new(
            Primitive::Rect { rect: body },
            Style::edge().with_fill(Paint::Ink(Ink::Accent), Tint::Wash),
        ),
        label(
            Point::new(body.x, body.y - gap),
            name,
            Anchor::Start,
            TextRole::Mono,
            Ink::Secondary,
        ),
        label(
            Point::new(body.x, body.y + body.height + gap),
            file,
            Anchor::Start,
            TextRole::Note,
            Ink::Faint,
        ),
    ]
}

/// The drawing of `entries` titled `title`, with `footer` under the box.
/// `added` is the theme's success colour.
pub(super) fn illustration(
    entries: &[SheetSyncEntry],
    mode: SheetSyncPreviewMode,
    (title, footer): (&str, &str),
    added: Color,
    size: PreviewSize,
    i18n: &I18n,
) -> Illustration {
    let drawn = shown(entries, mode);
    let (right, left): (Vec<&Shown<'_>>, Vec<&Shown<'_>>) =
        drawn.iter().partition(|s| s.entry.direction == "output");
    let left_w = flag_width(left.iter().map(|s| s.name.chars().count()), size);
    let right_w = flag_width(right.iter().map(|s| s.name.chars().count()), size);
    let room = 2.0 * size.em(STUB_EM);
    let box_w = (left_w + right_w + size.em(COLUMN_GAP_EM))
        .max(size.width * 0.32)
        .min((size.width - room).max(left_w + right_w));
    let rows = left.len().max(right.len()).max(1);
    let pitch = size.em(PITCH_EM);
    let box_h = pitch * (rows + 1) as f32;
    let x = (size.width - box_w) / 2.0;
    let y = (size.height - box_h) / 2.0;
    let body = Rectangle::new(Point::new(x, y), Size::new(box_w, box_h));
    let mut parts = box_parts(body, title, footer, size);
    for (column, edge, into_box, width) in
        [(&left, x, 1.0, left_w), (&right, x + box_w, -1.0, right_w)]
    {
        for (row, item) in column.iter().enumerate() {
            let at = y + pitch * (row + 1) as f32;
            entry_parts(
                &mut parts,
                item,
                (edge, into_box, at, width),
                mode,
                added,
                size,
                i18n,
            );
        }
    }
    Illustration::new(size.width, size.height)
        .height_em(size.height / size.text_size)
        .parts(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, direction: &'static str, change: SheetEntryChange) -> SheetSyncEntry {
        SheetSyncEntry {
            name: name.to_string(),
            direction,
            change,
        }
    }

    fn sample() -> Vec<SheetSyncEntry> {
        vec![
            entry("GND", "passive", SheetEntryChange::Kept),
            entry(
                "VIN_12V",
                "input",
                SheetEntryChange::Renamed {
                    from: "VIN".to_string(),
                },
            ),
            entry("VSENSE", "input", SheetEntryChange::Removed),
            entry("PGOOD", "output", SheetEntryChange::Added),
        ]
    }

    fn texts(mode: SheetSyncPreviewMode) -> Vec<String> {
        let size = PreviewSize::new(540.0, false, 3, 11.0);
        illustration(
            &sample(),
            mode,
            ("U_Power", "power.snxsch"),
            Color::BLACK,
            size,
            I18n::en_us_ref(),
        )
        .visible_parts()
        .filter_map(|part| match &part.primitive {
            Primitive::Label(label) => Some(label.text.as_str().to_string()),
            _ => None,
        })
        .collect()
    }

    /// Mockup row 79: After shows the added entry and the rename with its
    /// old name; Before shows the sheet as it is.
    #[test]
    fn after_and_before_draw_the_two_states() {
        let after = texts(SheetSyncPreviewMode::After);
        for name in ["PGOOD", "VIN_12V", "VSENSE", "GND"] {
            assert!(after.iter().any(|t| t == name), "{name} in {after:?}");
        }
        assert!(after.iter().any(|t| t.contains("was") && t.contains("VIN")));

        let before = texts(SheetSyncPreviewMode::Before);
        assert!(!before.iter().any(|t| t == "PGOOD"), "not there yet");
        assert!(!before.iter().any(|t| t == "VIN_12V"));
        assert!(before.iter().any(|t| t == "VIN"));
        assert!(!before.iter().any(|t| t.contains("was")));
    }

    /// A removed entry is a dashed ghost only in the After drawing.
    #[test]
    fn a_removed_entry_is_a_dashed_ghost_after_apply() {
        let style = |mode| flag_style(&SheetEntryChange::Removed, mode, Color::BLACK);
        let dashed = |s: Style| s.stroke.is_some_and(|stroke| stroke.is_dashed);
        assert!(dashed(style(SheetSyncPreviewMode::After)));
        assert!(!dashed(style(SheetSyncPreviewMode::Before)));
    }

    /// The legend's rule: input in, output out, passive square.
    #[test]
    fn a_flag_points_by_its_io_type() {
        let size = PreviewSize::new(540.0, false, 1, 10.0);
        let corners =
            |direction| flag_outline(0.0, 0.0, 40.0, 1.0, points_of(direction), size).len();
        assert_eq!(corners("passive"), 4);
        assert_eq!(corners("input"), 5);
        assert_eq!(corners("output"), 5);
        assert_eq!(corners("bidirectional"), 6);
        let input = flag_outline(0.0, 0.0, 40.0, 1.0, points_of("input"), size);
        assert!(
            input.contains(&Point::new(40.0, 0.0)),
            "the point is inside the box"
        );
    }

    /// A long list grows the drawing; it never overlaps its rows.
    #[test]
    fn the_drawing_grows_with_its_rows() {
        let short = PreviewSize::new(540.0, false, 2, 11.0);
        let long = PreviewSize::new(540.0, false, 20, 11.0);
        assert!(long.height > short.height);
        assert!(long.height >= 11.0 * PITCH_EM * 21.0);
    }
}
