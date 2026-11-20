//! The "Sheet" illustration of Document Options ▸ Page Options (#2531): the
//! paper at its orientation, the border with its zone letters and numbers,
//! the title block and the origin marker. The W / H callouts follow the
//! fields that set the size: Paper, or the typed W / H of a Custom format.

use iced::{Point, Rectangle, Size};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::schematic::PageOrigin;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextLabel,
    TextRole, Tint, Weight,
};

pub const PAPER: FieldId = FieldId::new("doc-paper");
pub const ORIENTATION: FieldId = FieldId::new("doc-orientation");
pub const PAPER_WIDTH: FieldId = FieldId::new("doc-paper-width");
pub const PAPER_HEIGHT: FieldId = FieldId::new("doc-paper-height");
pub const ORIGIN: FieldId = FieldId::new("doc-origin");
pub const BORDER: FieldId = FieldId::new("doc-show-border");
pub const ZONES: FieldId = FieldId::new("doc-zones");
pub const TITLE_BLOCK: FieldId = FieldId::new("doc-show-title-block");

/// View box of the drawing.
const VIEW: Size = Size::new(264.0, 140.0);
/// The box the paper is fitted into, leaving room for the callouts.
const AREA: Rectangle = Rectangle {
    x: 14.0,
    y: 24.0,
    width: 184.0,
    height: 100.0,
};
/// Border inset from the paper edge, in view units.
const BORDER_INSET: f32 = 7.0;
/// Title block size as a share of the space inside the border.
const TITLE_WIDTH_SHARE: f32 = 0.42;
const TITLE_HEIGHT_SHARE: f32 = 0.2;
const CALLOUT_GAP: f32 = 9.0;
const ORIGIN_ARM: f32 = 6.0;

/// What the illustration shows: the sheet's page settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetFacts {
    /// Paper size as shown, already turned to its orientation.
    pub width_mm: f32,
    pub height_mm: f32,
    pub show_border: bool,
    pub show_title_block: bool,
    pub vertical_zones: u8,
    pub horizontal_zones: u8,
    pub origin: PageOrigin,
    /// Format = Custom: the size comes from the typed W / H fields.
    pub is_custom: bool,
}

/// A size in mm as the callout reads it: whole millimetres stay whole.
pub fn mm_text(mm: f32) -> String {
    if (mm - mm.round()).abs() < 0.05 {
        format!("{mm:.0}")
    } else {
        format!("{mm:.1}")
    }
}

/// The paper rectangle, fitted into [`AREA`] at the sheet's proportions.
fn paper_rect(facts: &SheetFacts) -> Rectangle {
    let (w, h) = (facts.width_mm.max(1.0), facts.height_mm.max(1.0));
    let scale = (AREA.width / w).min(AREA.height / h);
    let size = Size::new(w * scale, h * scale);
    Rectangle::new(
        Point::new(
            AREA.x + (AREA.width - size.width) / 2.0,
            AREA.y + (AREA.height - size.height) / 2.0,
        ),
        size,
    )
}

fn inset(rect: Rectangle, by: f32) -> Rectangle {
    Rectangle::new(
        Point::new(rect.x + by, rect.y + by),
        Size::new(
            (rect.width - 2.0 * by).max(1.0),
            (rect.height - 2.0 * by).max(1.0),
        ),
    )
}

fn zone_label(at: Point, text: String) -> Part {
    Part::label(
        TextLabel {
            at,
            text: UserText::new(text).into(),
            anchor: Anchor::Middle,
            role: TextRole::Note,
        },
        Ink::Faint,
    )
    .linked(ZONES)
    .requires(BORDER)
}

/// The letter of vertical zone `i`: A, B, C...
fn zone_letter(i: u8) -> String {
    char::from(b'A' + i % 26).to_string()
}

/// Border, zone dividers and zone names.
fn border(paper: Rectangle, facts: &SheetFacts) -> Vec<Part> {
    let frame = inset(paper, BORDER_INSET);
    let mut parts = vec![
        Part::new(
            Primitive::Rect { rect: frame },
            Style::outline(Ink::Secondary, Weight::Thin),
        )
        .linked(BORDER)
        .requires(BORDER),
    ];
    let across = facts.horizontal_zones;
    if across > 0 {
        let step = frame.width / f32::from(across);
        for i in 0..across {
            let x = frame.x + step * (f32::from(i) + 0.5);
            let y = paper.y + BORDER_INSET / 2.0;
            parts.push(zone_label(Point::new(x, y), (i + 1).to_string()));
        }
    }
    let down = facts.vertical_zones;
    if down > 0 {
        let step = frame.height / f32::from(down);
        for i in 0..down {
            let y = frame.y + step * (f32::from(i) + 0.5);
            let x = paper.x + BORDER_INSET / 2.0;
            parts.push(zone_label(Point::new(x, y), zone_letter(i)));
        }
    }
    parts
}

fn title_block(paper: Rectangle) -> Part {
    let frame = inset(paper, BORDER_INSET);
    let size = Size::new(
        frame.width * TITLE_WIDTH_SHARE,
        frame.height * TITLE_HEIGHT_SHARE,
    );
    let rect = Rectangle::new(
        Point::new(
            frame.x + frame.width - size.width,
            frame.y + frame.height - size.height,
        ),
        size,
    );
    Part::new(
        Primitive::Rect { rect },
        Style::outline(Ink::Secondary, Weight::Thin)
            .with_fill(Paint::Ink(Ink::Secondary), Tint::Light),
    )
    .linked(TITLE_BLOCK)
    .requires(TITLE_BLOCK)
}

/// The origin marker at the corner the sheet counts from.
fn origin(paper: Rectangle, facts: &SheetFacts, i18n: &I18n) -> Vec<Part> {
    let corner = match facts.origin {
        PageOrigin::UpperLeft => Point::new(paper.x, paper.y),
        PageOrigin::LowerLeft => Point::new(paper.x, paper.y + paper.height),
    };
    let arm = |dx: f32, dy: f32| {
        Part::new(
            Primitive::Line {
                from: corner,
                to: Point::new(corner.x + dx, corner.y + dy),
            },
            Style::outline(Ink::Accent, Weight::Regular),
        )
        .linked(ORIGIN)
    };
    let up = match facts.origin {
        PageOrigin::UpperLeft => ORIGIN_ARM,
        PageOrigin::LowerLeft => -ORIGIN_ARM,
    };
    vec![
        arm(ORIGIN_ARM, 0.0),
        arm(0.0, up),
        Part::leader(Leader {
            at: corner,
            label_at: Point::new(AREA.x + AREA.width + 8.0, corner.y),
            label: t!(i18n, ids::PROPERTIES_ILLUSTRATION_ORIGIN).into(),
            anchor: Anchor::Start,
        })
        .linked(ORIGIN),
    ]
}

/// The W and H callouts, linked to the fields that set the size.
fn size_callouts(paper: Rectangle, facts: &SheetFacts, i18n: &I18n) -> Vec<Part> {
    let (width_field, height_field) = if facts.is_custom {
        (PAPER_WIDTH, PAPER_HEIGHT)
    } else {
        (PAPER, PAPER)
    };
    let (left, right) = (paper.x, paper.x + paper.width);
    let (top, bottom) = (paper.y, paper.y + paper.height);
    let w_y = top - CALLOUT_GAP;
    let width = Callout::new(Point::new(left, w_y), Point::new(right, w_y))
        .label(
            t!(i18n, ids::PROPERTIES_ILLUSTRATION_PAPER_WIDTH, "size" => mm_text(facts.width_mm)),
        )
        .extension(Point::new(left, top), Point::new(left, w_y - 3.0))
        .extension(Point::new(right, top), Point::new(right, w_y - 3.0));
    let h_x = right + CALLOUT_GAP;
    let height = Callout::new(Point::new(h_x, top), Point::new(h_x, bottom))
        .label(
            t!(i18n, ids::PROPERTIES_ILLUSTRATION_PAPER_HEIGHT, "size" => mm_text(facts.height_mm)),
        )
        .label_at(Point::new(h_x + 5.0, (top + bottom) / 2.0), Anchor::Start)
        .extension(Point::new(right, top), Point::new(h_x + 3.0, top))
        .extension(Point::new(right, bottom), Point::new(h_x + 3.0, bottom));
    let mut w = Part::callout(width).linked(width_field);
    let mut h = Part::callout(height).linked(height_field);
    if !facts.is_custom {
        w = w.linked(ORIENTATION);
        h = h.linked(ORIENTATION);
    }
    vec![w, h]
}

/// The Sheet illustration for `facts`.
pub fn sheet_illustration(facts: &SheetFacts, i18n: &I18n) -> Illustration {
    let paper = paper_rect(facts);
    let mut sheet = Part::new(
        Primitive::Rect { rect: paper },
        Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Wash),
    );
    sheet = if facts.is_custom {
        sheet.linked_all([PAPER_WIDTH, PAPER_HEIGHT])
    } else {
        sheet.linked_all([PAPER, ORIENTATION])
    };
    Illustration::new(VIEW.width, VIEW.height)
        .part(sheet)
        .parts(border(paper, facts))
        .part(title_block(paper))
        .parts(origin(paper, facts, i18n))
        .parts(size_callouts(paper, facts, i18n))
        .feature(BORDER, facts.show_border)
        .feature(TITLE_BLOCK, facts.show_title_block)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a4() -> SheetFacts {
        SheetFacts {
            width_mm: 297.0,
            height_mm: 210.0,
            show_border: true,
            show_title_block: true,
            vertical_zones: 4,
            horizontal_zones: 6,
            origin: PageOrigin::UpperLeft,
            is_custom: false,
        }
    }

    fn shows(illustration: &Illustration, field: &FieldId) -> bool {
        illustration.visible_parts().any(|p| p.is_linked_to(field))
    }

    fn labels(illustration: &Illustration) -> Vec<String> {
        illustration
            .visible_parts()
            .filter_map(|p| match &p.primitive {
                Primitive::Label(l) => Some(l.text.as_str().to_string()),
                _ => None,
            })
            .collect()
    }

    /// The border carries one name per zone: numbers across, letters down.
    #[test]
    fn the_border_names_its_zones() {
        let i18n = I18n::en_us();
        let names = labels(&sheet_illustration(&a4(), &i18n));
        for name in ["1", "6", "A", "D"] {
            assert!(names.contains(&name.to_string()), "{name} in {names:?}");
        }
        assert!(!names.contains(&"7".to_string()));
        assert!(!names.contains(&"E".to_string()));
    }

    /// Show Border off drops the border and its zones; Show Title Block
    /// off drops the title block.
    #[test]
    fn switched_off_parts_are_not_drawn() {
        let i18n = I18n::en_us();
        let all = sheet_illustration(&a4(), &i18n);
        assert!(shows(&all, &BORDER) && shows(&all, &ZONES) && shows(&all, &TITLE_BLOCK));
        let bare = sheet_illustration(
            &SheetFacts {
                show_border: false,
                show_title_block: false,
                ..a4()
            },
            &i18n,
        );
        assert!(!shows(&bare, &BORDER));
        assert!(!shows(&bare, &ZONES));
        assert!(!shows(&bare, &TITLE_BLOCK));
        assert!(shows(&bare, &PAPER), "the paper stays");
    }

    /// The W / H callouts follow Paper in a standard format and the typed
    /// W / H fields in a custom one.
    #[test]
    fn the_size_callouts_follow_the_size_fields() {
        let i18n = I18n::en_us();
        let standard = sheet_illustration(&a4(), &i18n);
        assert!(!shows(&standard, &PAPER_WIDTH));
        let custom = sheet_illustration(
            &SheetFacts {
                is_custom: true,
                ..a4()
            },
            &i18n,
        );
        assert!(shows(&custom, &PAPER_WIDTH) && shows(&custom, &PAPER_HEIGHT));
        assert!(!shows(&custom, &PAPER));
    }

    /// A portrait sheet is drawn taller than wide.
    #[test]
    fn the_paper_is_drawn_at_its_orientation() {
        let landscape = paper_rect(&a4());
        assert!(landscape.width > landscape.height);
        let portrait = paper_rect(&SheetFacts {
            width_mm: 210.0,
            height_mm: 297.0,
            ..a4()
        });
        assert!(portrait.height > portrait.width);
    }

    #[test]
    fn whole_millimetres_read_whole() {
        assert_eq!(mm_text(297.0), "297");
        assert_eq!(mm_text(100.5), "100.5");
    }
}
