//! The Grid & Snap popover (m6, GG-05): a right-click on the Grid read-out of
//! a schematic opens the Document Options grid settings above it.
//!
//! The card holds a modest line-art drawing (visible-grid dots, snap crosses,
//! a hotspot ring and a wire end snapped to the nearest point) with pitch
//! callouts linked to the Pitch fields, then the enable subsections Visible
//! Grid, Snap Grid and Snap to Hotspots. Every control sends the message the
//! Document Options row sends, so both surfaces show one state. The drawing
//! is rebuilt from the settings on every view: a Pitch change redraws it and
//! a switched-off feature drops out of it. There is no handle and no editing
//! on the drawing; the fields are the only place to edit.

use iced::widget::{Space, column, container, mouse_area, pick_list, row};
use iced::{Element, Font, Length, Padding, Point, Rectangle, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::property_illustration::{
    Callout, FieldId, Illustration, IllustrationHover, IllustrationMsg, Ink, Paint, Part,
    Primitive, Style, Tint, Weight, property_illustration,
};
use signex_widgets::text;

use super::graphite::{SP, TS, grid_value, popup};
use crate::choice::Choice;
use crate::panels::PanelMsg;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Width of the card (the mock's 320 px popover).
const CARD_WIDTH: f32 = 320.0;
/// Width of the "Pitch" label column under a subsection header.
const PITCH_LABEL: f32 = TS.body * 8.0;
/// Height of the drawing, in captions.
const DRAWING_HEIGHT_EM: f32 = 10.0;
/// The command whose key the Snap to Hotspots header shows.
pub(crate) const HOTSPOTS_COMMAND: &str = "toggle_electrical_grid";

/// The field the visible-grid pitch callout and dots stand for.
pub(crate) const VISIBLE_PITCH: FieldId = FieldId::new("grid-snap.visible-pitch");
/// The field the snap-grid pitch callout and crosses stand for.
pub(crate) const SNAP_PITCH: FieldId = FieldId::new("grid-snap.snap-pitch");
/// Feature switches: a part that needs one is dropped while it is off.
pub(crate) const VISIBLE_ON: FieldId = FieldId::new("grid-snap.visible-grid");
pub(crate) const SNAP_ON: FieldId = FieldId::new("grid-snap.snap-grid");
pub(crate) const HOTSPOTS_ON: FieldId = FieldId::new("grid-snap.hotspots");

/// The open popover: the window whose status bar opened it and the hover
/// link between its drawing and its Pitch fields.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSnapPopover {
    pub window: iced::window::Id,
    pub hover: IllustrationHover,
}

impl GridSnapPopover {
    pub fn new(window: iced::window::Id) -> Self {
        Self {
            window,
            hover: IllustrationHover::default(),
        }
    }
}

/// The Document Options grid settings the popover shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSnapSettings {
    pub grid_visible: bool,
    pub visible_grid_mm: f32,
    pub snap_enabled: bool,
    pub snap_grid_mm: f32,
    pub snap_hotspots: bool,
    pub unit: Unit,
}

/// What the popover reports.
#[derive(Debug, Clone)]
pub enum GridPopoverEvent {
    /// A setting changed: the message the Document Options row sends.
    SettingChanged(PanelMsg),
    /// The pointer moved over the drawing or a Pitch row.
    PreviewHovered(IllustrationMsg),
}

/// A subsection's Pitch field: the field it is linked to in the drawing,
/// the pitch, and the message a pick sends.
#[derive(Clone)]
pub(super) struct Pitch {
    pub field: FieldId,
    pub mm: f32,
    pub picked: fn(f32) -> PanelMsg,
}

/// One enable subsection: its header, whether it is on, the message its
/// header sends and its Pitch field, if it has one.
#[derive(Clone)]
pub(super) struct Section {
    pub label: MessageId,
    pub is_enabled: bool,
    pub toggled: PanelMsg,
    pub pitch: Option<Pitch>,
}

/// The three subsections of the card, top to bottom, with the messages of
/// the Document Options ▸ General rows.
pub(super) fn sections(s: &GridSnapSettings) -> [Section; 3] {
    [
        Section {
            label: ids::PROPERTIES_VISIBLE_GRID,
            is_enabled: s.grid_visible,
            toggled: PanelMsg::ToggleGrid,
            pitch: Some(Pitch {
                field: VISIBLE_PITCH,
                mm: s.visible_grid_mm,
                picked: PanelMsg::SetVisibleGridSize,
            }),
        },
        Section {
            label: ids::PROPERTIES_SNAP_GRID,
            is_enabled: s.snap_enabled,
            toggled: PanelMsg::ToggleSnap,
            pitch: Some(Pitch {
                field: SNAP_PITCH,
                mm: s.snap_grid_mm,
                picked: PanelMsg::SetGridSize,
            }),
        },
        Section {
            label: ids::PROPERTIES_SNAP_TO_HOTSPOTS,
            is_enabled: s.snap_hotspots,
            toggled: PanelMsg::ToggleSnapHotspots,
            pitch: None,
        },
    ]
}

// The drawing, in view units (about one logical px each at the card width).
const VIEW_W: f32 = 320.0;
const VIEW_H: f32 = 110.0;
/// View units per mm: a 0.635 mm pitch is 11.4 units, 5.08 mm is 91.
const UNITS_PER_MM: f32 = 18.0;
/// The finest pitch drawn, so an odd setting cannot flood the drawing.
const MIN_PITCH_UNITS: f32 = 6.0;
/// Where the grids start, and how far they reach.
const GRID_ORIGIN: Point = Point::new(12.0, 24.0);
const GRID_RIGHT: f32 = 222.0;
const GRID_BOTTOM: f32 = 90.0;
/// The callout lines above and below the grid.
const TOP_CALLOUT_Y: f32 = 10.0;
const BOTTOM_CALLOUT_Y: f32 = 102.0;
/// The part body, its pin and the pin's hotspot.
const BODY: Rectangle = Rectangle {
    x: 248.0,
    y: 34.0,
    width: 60.0,
    height: 48.0,
};
const HOTSPOT: Point = Point::new(226.0, 58.0);
/// Radius of the hotspot ring, and how far a hotspot reaches a wire end.
const HOTSPOT_RING: f32 = 7.0;
const HOTSPOT_REACH: f32 = 24.0;
/// The pointer the wire end is snapped from: off every grid on purpose.
const POINTER: Point = Point::new(207.3, 66.2);
/// Where the wire starts.
const WIRE_START_X: f32 = 60.0;

/// A pitch in view units.
fn pitch_units(mm: f32) -> f32 {
    (mm * UNITS_PER_MM).max(MIN_PITCH_UNITS)
}

/// The grid points at `pitch` view units inside the grid area.
fn grid_points(pitch: f32) -> impl Iterator<Item = Point> {
    let columns = ((GRID_RIGHT - GRID_ORIGIN.x) / pitch).floor() as usize;
    let rows = ((GRID_BOTTOM - GRID_ORIGIN.y) / pitch).floor() as usize;
    (0..=rows).flat_map(move |j| {
        (0..=columns).map(move |i| {
            Point::new(
                GRID_ORIGIN.x + i as f32 * pitch,
                GRID_ORIGIN.y + j as f32 * pitch,
            )
        })
    })
}

/// The snap point nearest to `p` on a grid of `pitch` view units.
fn nearest_grid_point(p: Point, pitch: f32) -> Point {
    let snap = |v: f32, origin: f32| origin + ((v - origin) / pitch).round() * pitch;
    Point::new(snap(p.x, GRID_ORIGIN.x), snap(p.y, GRID_ORIGIN.y))
}

/// Where the wire ends: on the hotspot when hotspot snapping is on and the
/// pointer is near it, else on the nearest snap point while snapping, else
/// at the pointer.
pub(super) fn wire_end(s: &GridSnapSettings) -> Point {
    if s.snap_hotspots && POINTER.distance(HOTSPOT) <= HOTSPOT_REACH {
        HOTSPOT
    } else if s.snap_enabled {
        nearest_grid_point(POINTER, pitch_units(s.snap_grid_mm))
    } else {
        POINTER
    }
}

/// A pitch callout from `x0` across one `pitch`, on the line at `y`, with
/// extension lines from the grid edge at `edge_y`.
fn pitch_callout(x0: f32, pitch: f32, y: f32, edge_y: f32, label: signex_i18n::Localized) -> Part {
    let (from, to) = (Point::new(x0, y), Point::new(x0 + pitch, y));
    Part::callout(
        Callout::new(from, to)
            .label(label)
            .extension(Point::new(x0, edge_y), from)
            .extension(Point::new(x0 + pitch, edge_y), to),
    )
}

/// A small cross centred on `at`.
fn cross(at: Point, half: f32, style: Style) -> [Part; 2] {
    [
        Part::new(
            Primitive::Line {
                from: Point::new(at.x - half, at.y),
                to: Point::new(at.x + half, at.y),
            },
            style,
        ),
        Part::new(
            Primitive::Line {
                from: Point::new(at.x, at.y - half),
                to: Point::new(at.x, at.y + half),
            },
            style,
        ),
    ]
}

/// The line-art drawing of `s`: what is on draws, what is off drops out.
pub(super) fn illustration(s: &GridSnapSettings, i18n: &I18n) -> Illustration {
    let visible = pitch_units(s.visible_grid_mm);
    let snap = pitch_units(s.snap_grid_mm);
    let pitch_label =
        |mm: f32| t!(i18n, ids::PANELS_GRID_SNAP_PITCH_VALUE, "pitch" => grid_value(mm, s.unit));
    let dot = Style::filled(Paint::Ink(Ink::Rule), Tint::Solid);
    let dots = grid_points(visible).map(move |p| {
        Part::new(
            Primitive::Circle {
                center: p,
                radius: 1.2,
            },
            dot,
        )
        .linked(VISIBLE_PITCH)
        .requires(VISIBLE_ON)
    });
    let crosses = grid_points(snap).flat_map(|p| {
        cross(p, 2.5, Style::outline(Ink::Tertiary, Weight::Thin))
            .map(|part| part.linked(SNAP_PITCH).requires(SNAP_ON))
    });
    let end = wire_end(s);
    let mut parts: Vec<Part> = dots.chain(crosses).collect();
    parts.extend([
        pitch_callout(
            GRID_ORIGIN.x,
            visible,
            TOP_CALLOUT_Y,
            GRID_ORIGIN.y,
            pitch_label(s.visible_grid_mm),
        )
        .linked(VISIBLE_PITCH)
        .requires(VISIBLE_ON),
        pitch_callout(
            GRID_ORIGIN.x,
            snap,
            BOTTOM_CALLOUT_Y,
            GRID_BOTTOM,
            pitch_label(s.snap_grid_mm),
        )
        .linked(SNAP_PITCH)
        .requires(SNAP_ON),
        // The part, its pin and the hotspot ring at the pin's end.
        Part::new(
            Primitive::Rect { rect: BODY },
            Style::edge().with_fill(Paint::Ink(Ink::Panel), Tint::Solid),
        ),
        Part::new(
            Primitive::Line {
                from: Point::new(BODY.x, HOTSPOT.y),
                to: HOTSPOT,
            },
            Style::edge(),
        ),
        Part::new(
            Primitive::Circle {
                center: HOTSPOT,
                radius: HOTSPOT_RING,
            },
            Style::dashed(),
        )
        .requires(HOTSPOTS_ON),
    ]);
    // The pointer, and the snap that took the wire end off it.
    parts.extend(cross(
        POINTER,
        2.0,
        Style::outline(Ink::Faint, Weight::Hairline),
    ));
    if end != POINTER {
        parts.push(Part::new(
            Primitive::Line {
                from: POINTER,
                to: end,
            },
            Style::dashed(),
        ));
    }
    parts.extend([
        Part::new(
            Primitive::Line {
                from: Point::new(WIRE_START_X, end.y),
                to: end,
            },
            Style::outline(Ink::Secondary, Weight::Heavy),
        ),
        Part::new(
            Primitive::Circle {
                center: end,
                radius: 2.0,
            },
            Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
        ),
    ]);
    Illustration::new(VIEW_W, VIEW_H)
        .height_em(DRAWING_HEIGHT_EM)
        .parts(parts)
        .feature(VISIBLE_ON, s.grid_visible)
        .feature(SNAP_ON, s.snap_enabled)
        .feature(HOTSPOTS_ON, s.snap_hotspots)
}

/// The pitch choices in `unit`, as the Document Options row lists them.
fn pitch_choices(unit: Unit) -> Vec<Choice<f32>> {
    use crate::canvas::grid::{GRID_SIZE_LABELS, GRID_SIZE_LABELS_MIL, GRID_SIZES_MM};
    let labels = if unit == Unit::Mil {
        GRID_SIZE_LABELS_MIL
    } else {
        GRID_SIZE_LABELS
    };
    GRID_SIZES_MM
        .iter()
        .zip(labels)
        .map(|(&value, label)| Choice {
            value,
            label: (*label).to_string(),
        })
        .collect()
}

/// The UI face at `weight`.
fn ui_font(weight: iced::font::Weight) -> Font {
    Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A Pitch row: the label and the pitch pick list, indented under its
/// header. Hovering it lights the linked callout; a hovered callout lights
/// its label.
fn pitch_row<'a>(
    pitch: Pitch,
    unit: Unit,
    hover: &IllustrationHover,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, GridPopoverEvent> {
    let lit = hover.is_field_highlighted(&pitch.field);
    let label_ink = if lit {
        resolve(tokens).accent_text
    } else {
        resolve(tokens).ink3
    };
    let choices = pitch_choices(unit);
    let selected = choices
        .iter()
        .find(|c| (c.value - pitch.mm).abs() < 1e-4)
        .cloned();
    let picked = pitch.picked;
    // raw: grid pitches with their unit symbol (mm, mil), not language.
    let field = pick_list(selected, choices, |c: &Choice<f32>| c.label.clone())
        .graphite()
        .on_select(move |c: Choice<f32>| GridPopoverEvent::SettingChanged(picked(c.value)))
        .width(Length::Fill);
    let line = row![
        text(t!(i18n, ids::PANELS_GRID_SNAP_PITCH))
            .size(TS.body)
            .color(label_ink)
            .width(PITCH_LABEL),
        field,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let indented = container(line).padding(Padding {
        top: SP.xs,
        right: SP.l,
        bottom: SP.s,
        // Under the header's label: its inset, checkbox and gap.
        left: SP.l + TS.body + SP.s + SP.xs,
    });
    mouse_area(indented)
        .on_enter(GridPopoverEvent::PreviewHovered(
            IllustrationMsg::FieldHovered(pitch.field.clone()),
        ))
        .on_exit(GridPopoverEvent::PreviewHovered(
            IllustrationMsg::FieldLeft(pitch.field),
        ))
        .boxed()
}

/// A key cap at the right of a header, as the keymap writes the key.
fn key_cap<'a>(key: String, tokens: &ThemeTokens) -> Element<'a, GridPopoverEvent> {
    let cap = container(
        text(UserText::new(key))
            .size(TS.caption)
            .font(super::graphite::mono_font()),
    )
    .padding([1.0, SP.s + 1.0])
    .style(crate::styles::key_cap(tokens));
    // Level with the header's label, below its top inset.
    container(cap)
        .padding(Padding {
            top: SP.m - SP.xs,
            right: SP.l,
            bottom: 0.0,
            left: 0.0,
        })
        .boxed()
}

/// The card: the header with the document's file name, the drawing, the
/// three subsections and the foot caption.
pub(crate) fn view<'a>(
    s: &GridSnapSettings,
    hover: &IllustrationHover,
    file_name: Option<String>,
    hotspot_key: String,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, GridPopoverEvent> {
    let r = resolve(tokens);
    let mut head = row![
        text(t!(i18n, ids::PANELS_GRID_SNAP_TITLE))
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text)),
        Space::new().width(Length::Fill),
    ]
    .align_y(iced::Alignment::Center);
    if let Some(name) = file_name {
        head = head.push(
            text(UserText::new(name))
                .size(TS.caption)
                .font(super::graphite::mono_font())
                .color(r.ink4)
                .boxed(),
        );
    }
    let mut card = column![
        container(head).padding(Padding {
            top: 0.0,
            right: SP.l,
            bottom: SP.m,
            left: SP.l,
        }),
        property_illustration(illustration(s, i18n), hover, tokens, TS.caption)
            .map(GridPopoverEvent::PreviewHovered),
    ]
    .width(Length::Fill);
    let style = || crate::panels::enable_section::enable_section_style(tokens);
    for section in sections(s) {
        let toggled = section.toggled.clone();
        let header = EnableSection::new(t!(i18n, section.label), section.is_enabled)
            .on_toggle(move |_| GridPopoverEvent::SettingChanged(toggled.clone()));
        card = card.push(match section.pitch {
            Some(pitch) => header.view(style(), pitch_row(pitch, s.unit, hover, tokens, i18n)),
            None if hotspot_key.is_empty() => header.view(style(), Space::new().boxed()),
            None => row![
                container(header.view(style(), Space::new().boxed())).width(Length::Fill),
                key_cap(hotspot_key.clone(), tokens),
            ]
            .boxed(),
        });
    }
    card = card.push(
        container(
            text(t!(i18n, ids::PANELS_GRID_SNAP_FOOT))
                .size(TS.caption)
                .color(r.ink4),
        )
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: 0.0,
            left: SP.l,
        })
        .boxed(),
    );
    container(card)
        .width(CARD_WIDTH)
        .padding([SP.m, 0.0])
        .style(popup(tokens))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> GridSnapSettings {
        GridSnapSettings {
            grid_visible: true,
            visible_grid_mm: 2.54,
            snap_enabled: true,
            snap_grid_mm: 1.27,
            snap_hotspots: true,
            unit: Unit::Mm,
        }
    }

    fn drawn_linked_to(ill: &Illustration, field: &FieldId) -> usize {
        ill.visible_parts()
            .filter(|p| p.is_linked_to(field))
            .count()
    }

    fn drawn_needing(ill: &Illustration, feature: &FieldId) -> usize {
        ill.visible_parts()
            .filter(|p| p.requires.as_ref() == Some(feature))
            .count()
    }

    /// The popover's switches and pick lists send the very messages the
    /// Document Options ▸ General rows send, so the two stay in step.
    #[test]
    fn the_popover_sends_the_document_options_messages() {
        let [visible, snap, hotspots] = sections(&settings());
        assert_eq!(visible.label, ids::PROPERTIES_VISIBLE_GRID);
        assert!(matches!(visible.toggled, PanelMsg::ToggleGrid));
        let pitch = visible.pitch.expect("Visible Grid has a Pitch field");
        assert!(matches!(
            (pitch.picked)(5.08),
            PanelMsg::SetVisibleGridSize(mm) if (mm - 5.08).abs() < f32::EPSILON
        ));
        assert_eq!(snap.label, ids::PROPERTIES_SNAP_GRID);
        assert!(matches!(snap.toggled, PanelMsg::ToggleSnap));
        let pitch = snap.pitch.expect("Snap Grid has a Pitch field");
        assert!(matches!(
            (pitch.picked)(0.635),
            PanelMsg::SetGridSize(mm) if (mm - 0.635).abs() < f32::EPSILON
        ));
        assert_eq!(hotspots.label, ids::PROPERTIES_SNAP_TO_HOTSPOTS);
        assert!(matches!(hotspots.toggled, PanelMsg::ToggleSnapHotspots));
        assert!(hotspots.pitch.is_none());
    }

    /// A subsection is on exactly when its setting is: switched off, its
    /// Pitch field is greyed by the enable section.
    #[test]
    fn each_subsection_is_on_when_its_setting_is() {
        let mut s = settings();
        assert!(sections(&s).iter().all(|section| section.is_enabled));
        s.grid_visible = false;
        s.snap_hotspots = false;
        let [visible, snap, hotspots] = sections(&s);
        assert!(!visible.is_enabled);
        assert!(snap.is_enabled);
        assert!(!hotspots.is_enabled);
    }

    /// With everything on, the drawing shows the dots, the crosses, both
    /// pitch callouts and the hotspot ring.
    #[test]
    fn everything_on_draws_every_feature() {
        let ill = illustration(&settings(), &I18n::en_us());
        assert!(drawn_needing(&ill, &VISIBLE_ON) > 1);
        assert!(drawn_needing(&ill, &SNAP_ON) > 1);
        assert_eq!(drawn_needing(&ill, &HOTSPOTS_ON), 1);
        assert!(drawn_linked_to(&ill, &VISIBLE_PITCH) > 0);
        assert!(drawn_linked_to(&ill, &SNAP_PITCH) > 0);
    }

    /// Turning an enable off drops its feature from the drawing.
    #[test]
    fn a_switched_off_feature_drops_out_of_the_drawing() {
        let i18n = I18n::en_us();
        let mut s = settings();
        s.grid_visible = false;
        let ill = illustration(&s, &i18n);
        assert_eq!(drawn_needing(&ill, &VISIBLE_ON), 0);
        assert_eq!(drawn_linked_to(&ill, &VISIBLE_PITCH), 0);
        assert!(drawn_needing(&ill, &SNAP_ON) > 0);

        let mut s = settings();
        s.snap_enabled = false;
        let ill = illustration(&s, &i18n);
        assert_eq!(drawn_needing(&ill, &SNAP_ON), 0);
        assert!(drawn_needing(&ill, &VISIBLE_ON) > 0);

        let mut s = settings();
        s.snap_hotspots = false;
        let ill = illustration(&s, &i18n);
        assert_eq!(drawn_needing(&ill, &HOTSPOTS_ON), 0);
    }

    /// A Pitch change redraws the grid: a finer pitch draws more marks.
    #[test]
    fn the_pitch_fields_redraw_the_grid_and_snap_marks() {
        let i18n = I18n::en_us();
        let mut s = settings();
        let coarse = illustration(&s, &i18n);
        s.visible_grid_mm = 1.27;
        s.snap_grid_mm = 0.635;
        let fine = illustration(&s, &i18n);
        assert!(drawn_needing(&fine, &VISIBLE_ON) > drawn_needing(&coarse, &VISIBLE_ON));
        assert!(drawn_needing(&fine, &SNAP_ON) > drawn_needing(&coarse, &SNAP_ON));
    }

    /// The wire end snaps to the hotspot, else to the snap grid, else stays
    /// at the pointer.
    #[test]
    fn the_wire_end_snaps_as_the_settings_say() {
        let mut s = settings();
        assert_eq!(wire_end(&s), HOTSPOT);
        s.snap_hotspots = false;
        let on_grid = wire_end(&s);
        let pitch = pitch_units(s.snap_grid_mm);
        let steps = (on_grid.x - GRID_ORIGIN.x) / pitch;
        assert!((steps - steps.round()).abs() < 1e-3, "{on_grid:?}");
        assert_ne!(on_grid, POINTER);
        s.snap_enabled = false;
        assert_eq!(wire_end(&s), POINTER);
    }

    /// The Pitch pick lists offer the Document Options presets, in mil when
    /// the document shows mil.
    #[test]
    fn the_pitch_choices_follow_the_unit() {
        let mm: Vec<String> = pitch_choices(Unit::Mm)
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(mm, ["0.635 mm", "1.27 mm", "2.54 mm", "5.08 mm"]);
        let mil: Vec<f32> = pitch_choices(Unit::Mil)
            .into_iter()
            .map(|c| c.value)
            .collect();
        assert_eq!(mil, crate::canvas::grid::GRID_SIZES_MM);
    }
}
