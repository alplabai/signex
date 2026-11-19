//! Named unions on the sheet (#158): a dashed outline around the members, a
//! name tab at its top-left with the tags beside it, a padlock on the tab.
//! Drawn in the selection layer, so the outline follows the camera, is
//! editor-only and never reaches a print or an export. A hidden union keeps
//! its outline and a grey tab while its members fade behind a veil in the
//! paper colour (about 15 % of their ink shows). Rules:
//! `docs/SCHEMATIC_UNIONS.md`.

use super::*;
use signex_types::schematic::SchUnion;

/// Outline dash, gap and stroke width in screen px (mockup `.union`).
const DASH: [f32; 2] = [6.0, 4.0];
const OUTLINE_PX: f32 = 1.2;
/// Space between the members' boxes and the outline, in screen px.
const PAD_PX: f32 = 10.0;
/// Outline corner radius in screen px.
const RADIUS_PX: f32 = 6.0;
/// Name tab: height, side padding, corner radius, text size, in screen px.
const TAB_H_PX: f32 = 18.0;
const TAB_PAD_PX: f32 = 8.0;
const TAB_RADIUS_PX: f32 = 4.0;
const TAB_TEXT_PX: f32 = 10.5;
/// The tab overlaps the outline by this much, as drawn.
const TAB_OVERLAP_PX: f32 = 4.0;
/// Average advance of a tab character, for the tab's width.
const TAB_CHAR_PX: f32 = 6.2;
/// Room for the padlock at the tab's right end.
const LOCK_PX: f32 = 16.0;
/// Gap between the tab and the tag text.
const TAG_GAP_PX: f32 = 6.0;
/// How much of a hidden member's ink shows through the veil.
const HIDDEN_SHOWS: f32 = 0.15;

/// The colours a union is drawn in, from the theme tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnionInk {
    /// The dashed outline of an unselected union (ink3).
    pub outline: Color,
    /// Outline and tab of the selected union (accent).
    pub accent: Color,
    /// Name text on the accent tab.
    pub on_accent: Color,
    /// Tab fill and stroke of an unselected union (sunken, line-strong).
    pub tab_fill: Color,
    pub tab_stroke: Color,
    /// Tab name (ink2) and the name of a hidden union (ink4).
    pub tab_text: Color,
    pub hidden_text: Color,
    /// Tag text and the padlock (ink3).
    pub caption: Color,
    /// The sheet paper, for the veil over a hidden union's members.
    pub paper: Color,
}

impl UnionInk {
    #[must_use]
    pub fn from_tokens(tokens: &signex_types::theme::ThemeTokens) -> Self {
        let r = crate::styles::tokens::resolve(tokens);
        let ti = crate::styles::ti;
        Self {
            outline: r.ink3,
            accent: ti(tokens.accent),
            on_accent: r.primary_ink,
            tab_fill: r.sunken,
            tab_stroke: r.line_strong,
            tab_text: ti(tokens.text_secondary),
            hidden_text: r.ink4,
            caption: r.ink3,
            paper: ti(tokens.paper),
        }
    }
}

/// What the canvas needs to draw the unions of the sheet in view: the ones
/// hidden there (a view setting, not saved) and the inks.
#[derive(Debug, Clone, Copy)]
pub struct UnionView<'a> {
    pub hidden: &'a [uuid::Uuid],
    pub ink: UnionInk,
}

/// Where a union sits, in world mm: its outline and its name tab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnionFrame {
    pub outline: Aabb,
    pub tab: Aabb,
}

/// Whether the sheet has a union to draw.
#[must_use]
pub fn has_unions(snapshot: &SchematicRenderSnapshot) -> bool {
    !snapshot.unions.is_empty()
}

/// The member boxes of `union` that are on the sheet.
fn member_boxes(
    snapshot: &SchematicRenderSnapshot,
    union: &SchUnion,
    designators: &Designators,
    widths: WireWidths<'_>,
) -> Vec<Aabb> {
    let items = signex_engine::unions::member_items(snapshot, union);
    selected_aabbs(snapshot, &items, designators, widths)
        .into_iter()
        .flatten()
        .collect()
}

/// Width of the name tab in screen px.
fn tab_width_px(union: &SchUnion) -> f32 {
    TAB_PAD_PX * 2.0 + union.name.chars().count() as f32 * TAB_CHAR_PX + LOCK_PX
}

/// The outline and tab of `union` at camera scale `scale` (px per mm);
/// `None` when none of its members is on the sheet.
#[must_use]
pub fn union_frame(
    snapshot: &SchematicRenderSnapshot,
    union: &SchUnion,
    designators: &Designators,
    widths: WireWidths<'_>,
    scale: f32,
) -> Option<UnionFrame> {
    let px = |n: f32| screen_px_to_world_mm(n, scale);
    let outline = member_boxes(snapshot, union, designators, widths)
        .into_iter()
        .reduce(|a, b| a.union(&b))?
        .expand(px(PAD_PX));
    let top = outline.min_y - px(TAB_H_PX - TAB_OVERLAP_PX);
    let tab = Aabb::new(
        outline.min_x,
        top,
        outline.min_x + px(tab_width_px(union)),
        top + px(TAB_H_PX),
    );
    Some(UnionFrame { outline, tab })
}

/// The union whose name tab is under the world point `(x, y)`, the topmost
/// (last drawn) first.
#[must_use]
pub fn union_tab_at(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    widths: WireWidths<'_>,
    scale: f32,
    x: f64,
    y: f64,
) -> Option<uuid::Uuid> {
    snapshot.unions.iter().rev().find_map(|union| {
        let frame = union_frame(snapshot, union, designators, widths, scale)?;
        let tab = frame.tab;
        (x >= tab.min_x && x <= tab.max_x && y >= tab.min_y && y <= tab.max_y).then_some(union.uuid)
    })
}

fn screen_rect(transform: &ScreenTransform, b: &Aabb) -> (iced::Point, iced::Size) {
    let a = transform.world_to_screen((b.min_x, b.min_y));
    let z = transform.world_to_screen((b.max_x, b.max_y));
    (a, iced::Size::new(z.x - a.x, z.y - a.y))
}

/// Draw every union of `snapshot`. `selected` decides which one is in
/// accent: the union whose members are all selected.
pub fn draw_unions(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    view: UnionView<'_>,
    transform: &ScreenTransform,
    designators: &Designators,
    widths: WireWidths<'_>,
) {
    for union in &snapshot.unions {
        let hidden = view.hidden.contains(&union.uuid);
        if hidden {
            veil_members(
                frame,
                snapshot,
                union,
                view.ink,
                transform,
                designators,
                widths,
            );
        }
        let Some(at) = union_frame(snapshot, union, designators, widths, transform.scale) else {
            continue;
        };
        let is_selected = signex_engine::unions::selection_is_union(union, selected);
        draw_outline(frame, &at, view.ink, is_selected, hidden, transform);
        draw_tab(frame, union, &at, view.ink, is_selected, hidden, transform);
    }
}

/// Cover each member of a hidden union with the paper colour so only
/// [`HIDDEN_SHOWS`] of its ink is left.
fn veil_members(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    union: &SchUnion,
    ink: UnionInk,
    transform: &ScreenTransform,
    designators: &Designators,
    widths: WireWidths<'_>,
) {
    let veil = Color {
        a: 1.0 - HIDDEN_SHOWS,
        ..ink.paper
    };
    let grow = screen_px_to_world_mm(2.0, transform.scale);
    for b in member_boxes(snapshot, union, designators, widths) {
        let (at, size) = screen_rect(transform, &b.expand(grow));
        frame.fill_rectangle(at, size, veil);
    }
}

fn draw_outline(
    frame: &mut canvas::Frame,
    at: &UnionFrame,
    ink: UnionInk,
    is_selected: bool,
    hidden: bool,
    transform: &ScreenTransform,
) {
    let (top_left, size) = screen_rect(transform, &at.outline);
    let mut colour = if is_selected { ink.accent } else { ink.outline };
    if hidden {
        colour.a *= 0.6;
    }
    let path = canvas::Path::rounded_rectangle(top_left, size, RADIUS_PX.into());
    frame.stroke(
        &path,
        canvas::Stroke {
            line_dash: canvas::LineDash {
                segments: &DASH,
                offset: 0,
            },
            ..canvas::Stroke::default()
                .with_color(colour)
                .with_width(OUTLINE_PX)
        },
    );
}

fn draw_tab(
    frame: &mut canvas::Frame,
    union: &SchUnion,
    at: &UnionFrame,
    ink: UnionInk,
    is_selected: bool,
    hidden: bool,
    transform: &ScreenTransform,
) {
    let (top_left, size) = screen_rect(transform, &at.tab);
    let tab = canvas::Path::rounded_rectangle(top_left, size, TAB_RADIUS_PX.into());
    let (fill, text_ink) = match (is_selected && !hidden, hidden) {
        (true, _) => (ink.accent, ink.on_accent),
        (false, true) => (ink.tab_fill, ink.hidden_text),
        (false, false) => (ink.tab_fill, ink.tab_text),
    };
    frame.fill(&tab, fill);
    if !is_selected || hidden {
        frame.stroke(
            &tab,
            canvas::Stroke::default()
                .with_color(ink.tab_stroke)
                .with_width(1.0),
        );
    }
    let ui_font = crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT);
    let baseline_top = top_left.y + (TAB_H_PX - TAB_TEXT_PX) / 2.0 - 1.0;
    frame.fill_text(canvas::Text {
        content: union.name.clone(),
        position: iced::Point::new(top_left.x + TAB_PAD_PX, baseline_top),
        color: text_ink,
        size: iced::Pixels(TAB_TEXT_PX),
        font: ui_font,
        ..canvas::Text::default()
    });
    let lock_ink = if is_selected && !hidden {
        ink.on_accent
    } else {
        ink.caption
    };
    let lock_at = iced::Point::new(top_left.x + size.width - LOCK_PX + 3.0, top_left.y + 5.0);
    draw_padlock(frame, lock_at, union.locked, lock_ink);
    if !union.tags.is_empty() && !hidden {
        let tags = union
            .tags
            .iter()
            .map(|t| format!("#{t}"))
            .collect::<Vec<_>>()
            .join(" ");
        frame.fill_text(canvas::Text {
            content: tags,
            position: iced::Point::new(top_left.x + size.width + TAG_GAP_PX, top_left.y),
            color: ink.caption,
            size: iced::Pixels(TAB_TEXT_PX),
            font: ui_font,
            ..canvas::Text::default()
        });
    }
}

/// A 9 px padlock with its top-left at `at`: closed, or open with the
/// shackle's right leg lifted off the body.
fn draw_padlock(frame: &mut canvas::Frame, at: iced::Point, closed: bool, colour: Color) {
    let body = iced::Point::new(at.x, at.y + 4.0);
    frame.fill_rectangle(body, iced::Size::new(8.0, 5.0), colour);
    let (left, right) = (at.x + 1.5, at.x + 6.5);
    // Open: the shackle rises 2 px and its right leg stops short of the body.
    let (rim, right_end) = if closed {
        (at.y, body.y)
    } else {
        (at.y - 2.0, at.y)
    };
    let shackle = canvas::Path::new(|p| {
        p.move_to(iced::Point::new(left, body.y));
        p.line_to(iced::Point::new(left, rim));
        p.line_to(iced::Point::new(right, rim));
        p.line_to(iced::Point::new(right, right_end));
    });
    frame.stroke(
        &shackle,
        canvas::Stroke::default().with_color(colour).with_width(1.2),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{Junction, Point, SchematicSheet};

    fn sheet() -> SchematicSheet {
        let mut s = SchematicSheet::default();
        for n in 1..=2u128 {
            s.junctions.push(Junction {
                uuid: uuid::Uuid::from_u128(n),
                position: Point::new(10.0 * n as f64, 10.0),
                ..Default::default()
            });
        }
        s.unions.push(SchUnion {
            uuid: uuid::Uuid::from_u128(100),
            name: "Input filter".into(),
            tags: vec!["emc".into()],
            members: vec![uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2)],
            locked: false,
        });
        s
    }

    #[test]
    fn the_outline_frames_every_member_and_the_tab_sits_on_its_top_left() {
        let s = sheet();
        let frame = union_frame(
            &s,
            &s.unions[0],
            &Designators::new(),
            WireWidths::default(),
            4.0,
        )
        .expect("members on the sheet");
        assert!(frame.outline.min_x < 10.0 && frame.outline.max_x > 20.0);
        assert_eq!(frame.tab.min_x, frame.outline.min_x);
        assert!(frame.tab.min_y < frame.outline.min_y);
        assert!(
            frame.tab.max_y > frame.outline.min_y,
            "the tab overlaps the outline"
        );
    }

    #[test]
    fn a_click_on_the_tab_names_the_union() {
        let s = sheet();
        let frame = union_frame(
            &s,
            &s.unions[0],
            &Designators::new(),
            WireWidths::default(),
            4.0,
        )
        .unwrap();
        let (x, y) = (frame.tab.min_x + 0.5, frame.tab.min_y + 0.5);
        let hit = union_tab_at(&s, &Designators::new(), WireWidths::default(), 4.0, x, y);
        assert_eq!(hit, Some(uuid::Uuid::from_u128(100)));
        let miss = union_tab_at(
            &s,
            &Designators::new(),
            WireWidths::default(),
            4.0,
            15.0,
            10.0,
        );
        assert_eq!(miss, None);
    }

    #[test]
    fn the_inks_differ_between_the_graphite_themes() {
        use signex_types::theme::{ThemeId, theme_tokens};
        let light = UnionInk::from_tokens(&theme_tokens(ThemeId::GraphiteLight));
        let dark = UnionInk::from_tokens(&theme_tokens(ThemeId::GraphiteDark));
        assert_ne!(light.paper, dark.paper);
        assert_ne!(light.outline, light.paper);
        assert_ne!(dark.outline, dark.paper);
    }
}
