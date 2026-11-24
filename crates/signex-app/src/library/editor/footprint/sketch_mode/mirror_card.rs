//! The Mirror options card (mockup row 191; DECISIONS 2026-11-23 and
//! 2026-11-24): Objects, Mirror line, Keep linked, OK and Cancel. While
//! Mirror is armed, canvas clicks are picks that fill the card and write
//! nothing; OK sends one [`MirrorRequest`], one undo step; Cancel writes
//! nothing. Keep linked starts on each time the card opens.
//!
//! The card is session state beside the editor, never in its document
//! state: one per editor path, in `LibraryState::mirror_cards`, so the
//! footprint and board hosts share it. [`route`] answers the card's
//! messages and picks before the editor sees them.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::SketchData;
use signex_sketch::array::ArrayId;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use super::value_edit::SketchCanvasMsg;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{EditorMode, SketchTool};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// How far from a stroke or Point a pick still lands, in mm.
const PICK_TOL_MM: f64 = 0.30;

/// The card field the next pick fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorField {
    Objects,
    Line,
}

/// One thing to mirror: sketch geometry, or a pattern, which is mirrored
/// live whatever Keep linked says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorObject {
    Entity(SketchEntityId),
    Pattern(ArrayId),
}

/// The open card of one editor.
#[derive(Debug, Clone, PartialEq)]
pub struct MirrorCard {
    pub objects: Vec<MirrorObject>,
    pub line: Option<SketchEntityId>,
    pub keep_linked: bool,
    pub field: MirrorField,
}

/// What OK asks the editor to write.
#[derive(Debug, Clone, PartialEq)]
pub struct MirrorRequest {
    pub objects: Vec<MirrorObject>,
    pub line: SketchEntityId,
    pub keep_linked: bool,
}

/// What happened on the card.
#[derive(Debug, Clone, PartialEq)]
pub enum MirrorCardMsg {
    FieldFocused(MirrorField),
    KeepLinkedToggled(bool),
    Confirmed,
    Cancelled,
}

/// Whether `editor` shows the card: Mirror armed in sketch mode.
pub fn is_armed(editor: &FootprintDocument) -> bool {
    editor.state.mode == EditorMode::Sketch && editor.state.active_tool == SketchTool::Mirror
}

/// The card `editor` shows: its open `card`, or the one the next message
/// opens. `None` while Mirror is not armed.
pub fn shown(editor: &FootprintDocument, card: Option<&MirrorCard>) -> Option<MirrorCard> {
    is_armed(editor).then(|| card.cloned().unwrap_or_else(|| MirrorCard::opened(editor)))
}

/// A click of the armed Mirror tool: a card pick, never a tool click.
pub fn is_card_pick(editor: &FootprintDocument, msg: &FootprintEditorMsg) -> bool {
    is_armed(editor)
        && matches!(
            msg,
            FootprintEditorMsg::SketchToolClick { .. }
                | FootprintEditorMsg::SketchToolClickInferred { .. }
        )
}

impl MirrorCard {
    /// A fresh card: the selection becomes the objects, a selected
    /// centerline the mirror line, and the pattern selected on the canvas
    /// an object. Keep linked is on.
    pub fn opened(editor: &FootprintDocument) -> Self {
        let mut card = Self {
            objects: Vec::new(),
            line: None,
            keep_linked: true,
            field: MirrorField::Objects,
        };
        let state = &editor.state;
        if let Some(sketch) = editor.primitive().sketch.as_ref() {
            let selected = state
                .selected_sketch
                .into_iter()
                .chain(state.selected_sketch_secondary)
                .chain(state.selected_sketch_extra.iter().copied());
            for id in selected {
                card.pick(sketch, id);
            }
        }
        if let Some(pattern) = state.sketch_overlay.selected_pattern {
            card.add_pattern(pattern);
        }
        card
    }

    /// Take `id` into the card. With the line field focused a Line becomes
    /// the mirror line. Otherwise a centerline fills an empty line first,
    /// and anything else toggles in or out of the objects.
    pub fn pick(&mut self, sketch: &SketchData, id: SketchEntityId) {
        let Some(entity) = sketch.entities.iter().find(|e| e.id == id) else {
            return;
        };
        let is_line = matches!(entity.kind, EntityKind::Line { .. });
        if self.field == MirrorField::Line {
            if is_line {
                self.take_line(id);
            }
            return;
        }
        if is_line && entity.centerline && self.line.is_none() {
            self.take_line(id);
            return;
        }
        if self.line == Some(id) {
            return;
        }
        let object = MirrorObject::Entity(id);
        if let Some(at) = self.objects.iter().position(|o| *o == object) {
            self.objects.remove(at);
        } else {
            self.objects.push(object);
        }
        self.focus_line_when_due();
    }

    /// Add a pattern picked on the canvas to the objects.
    pub fn add_pattern(&mut self, pattern: ArrayId) {
        let object = MirrorObject::Pattern(pattern);
        if !self.objects.contains(&object) {
            self.objects.push(object);
        }
        self.focus_line_when_due();
    }

    fn take_line(&mut self, id: SketchEntityId) {
        self.objects.retain(|o| *o != MirrorObject::Entity(id));
        self.line = Some(id);
        self.field = MirrorField::Objects;
    }

    /// Once there is something to mirror, the next pick is the line.
    fn focus_line_when_due(&mut self) {
        if self.line.is_none() && !self.objects.is_empty() {
            self.field = MirrorField::Line;
        }
    }

    /// The objects still in `sketch`.
    fn live_objects(&self, sketch: &SketchData) -> Vec<MirrorObject> {
        self.objects
            .iter()
            .copied()
            .filter(|o| match o {
                MirrorObject::Entity(id) => sketch.entities.iter().any(|e| e.id == *id),
                MirrorObject::Pattern(p) => sketch.is_pattern(*p),
            })
            .collect()
    }

    /// The mirror line, while it is still a Line of `sketch`.
    fn live_line(&self, sketch: &SketchData) -> Option<SketchEntityId> {
        self.line.filter(|id| {
            sketch
                .entities
                .iter()
                .any(|e| e.id == *id && matches!(e.kind, EntityKind::Line { .. }))
        })
    }

    /// What OK writes; `None` until there are objects and a line.
    pub fn request(&self, sketch: &SketchData) -> Option<MirrorRequest> {
        let objects = self.live_objects(sketch);
        let line = self.live_line(sketch)?;
        (!objects.is_empty()).then_some(MirrorRequest {
            objects,
            line,
            keep_linked: self.keep_linked,
        })
    }
}

/// Answer `msg` for the card of `editor`, before the editor sees it.
/// `None` means the card took it; `Some` is the message the editor gets.
/// A card left behind by a tool that is no longer armed is dropped, so
/// the next arming opens a fresh one.
pub fn route(
    editor: &FootprintDocument,
    cards: &mut HashMap<PathBuf, MirrorCard>,
    msg: FootprintEditorMsg,
) -> Option<FootprintEditorMsg> {
    if !is_armed(editor) {
        cards.remove(&editor.path);
        return Some(msg);
    }
    let sketch = editor.primitive().sketch.as_ref();
    let card = cards
        .entry(editor.path.clone())
        .or_insert_with(|| MirrorCard::opened(editor));
    // The canvas draws the card's picks and copies (`mirror_ghost`). The
    // cache is iced's own bookkeeping, cleared through a shared reference.
    editor.canvas_cache.clear();
    match msg {
        FootprintEditorMsg::SketchMirrorCard(answer) => match answer {
            MirrorCardMsg::FieldFocused(field) => {
                card.field = field;
                None
            }
            MirrorCardMsg::KeepLinkedToggled(on) => {
                card.keep_linked = on;
                None
            }
            MirrorCardMsg::Cancelled => {
                cards.remove(&editor.path);
                Some(FootprintEditorMsg::SketchSetTool(SketchTool::Select))
            }
            MirrorCardMsg::Confirmed => {
                let request = sketch.and_then(|s| card.request(s))?;
                cards.remove(&editor.path);
                Some(FootprintEditorMsg::SketchMirrorApplied(request))
            }
        },
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        }
        | FootprintEditorMsg::SketchToolClickInferred {
            x_mm,
            y_mm,
            snap_id,
            ..
        } => {
            if let Some(sketch) = sketch {
                let snapped = snap_id.filter(|id| sketch.entities.iter().any(|e| e.id == *id));
                if let Some(id) = snapped.or_else(|| entity_at(sketch, x_mm, y_mm)) {
                    card.pick(sketch, id);
                }
            }
            None
        }
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::PatternSelected(Some(pattern))) => {
            card.add_pattern(pattern);
            Some(msg)
        }
        other => Some(other),
    }
}

/// The sketch entity under a click: a Point first, then the nearest Line,
/// Circle or Arc stroke.
fn entity_at(sketch: &SketchData, x: f64, y: f64) -> Option<SketchEntityId> {
    let at = |id: SketchEntityId| {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    let near_point = |p: (f64, f64)| (p.0 - x).hypot(p.1 - y);
    let mut best_point: Option<(f64, SketchEntityId)> = None;
    let mut best_stroke: Option<(f64, SketchEntityId)> = None;
    for e in &sketch.entities {
        let (gap, slot) = match e.kind {
            EntityKind::Point { x: px, y: py } => (near_point((px, py)), &mut best_point),
            EntityKind::Line { start, end } => {
                let (Some(a), Some(b)) = (at(start), at(end)) else {
                    continue;
                };
                (segment_gap((x, y), a, b), &mut best_stroke)
            }
            EntityKind::Circle { center, radius } => {
                let Some(c) = at(center) else { continue };
                ((near_point(c) - radius).abs(), &mut best_stroke)
            }
            EntityKind::Arc { center, start, .. } => {
                let (Some(c), Some(s)) = (at(center), at(start)) else {
                    continue;
                };
                let radius = (s.0 - c.0).hypot(s.1 - c.1);
                ((near_point(c) - radius).abs(), &mut best_stroke)
            }
        };
        if gap <= PICK_TOL_MM && slot.is_none_or(|(g, _)| gap < g) {
            *slot = Some((gap, e.id));
        }
    }
    best_point.or(best_stroke).map(|(_, id)| id)
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_gap(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (a.0 + t * dx - p.0).hypot(a.1 + t * dy - p.1)
}

/// The card at the canvas's top centre while Mirror is armed; an empty
/// layer otherwise. `card` is the editor's open card, if a message has
/// opened one yet.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    card: Option<&MirrorCard>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(open) = shown(editor, card) else {
        return Space::new().boxed();
    };
    let card = &open;
    let send = |msg: MirrorCardMsg| {
        Some(LibraryMessage::PrimitiveEditorEvent {
            path: editor.path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchMirrorCard(msg)),
        })
    };
    let sketch = editor.primitive().sketch.as_ref();
    let objects = sketch.map_or(0, |s| card.live_objects(s).len());
    let line = sketch.and_then(|s| {
        let id = card.live_line(s)?;
        s.entities.iter().find(|e| e.id == id).map(|e| e.centerline)
    });
    let objects_value = if objects == 0 {
        t!(i18n, ids::SKETCH_TOOLS_MIRROR_OBJECTS_NONE)
    } else {
        t!(i18n, ids::SKETCH_TOOLS_MIRROR_OBJECTS_COUNT, "count" => objects)
    };
    let line_value = match line {
        None => t!(i18n, ids::SKETCH_TOOLS_MIRROR_LINE_NONE),
        Some(true) => t!(i18n, ids::SKETCH_TOOLS_MIRROR_LINE_CENTERLINE),
        Some(false) => t!(i18n, ids::SKETCH_TOOLS_MIRROR_LINE_PICKED),
    };
    let field_row = |label: Localized, value: Localized, field: MirrorField| {
        let kind = if card.field == field {
            DialogButtonKind::Primary
        } else {
            DialogButtonKind::Secondary
        };
        row![
            text(label)
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
                .width(Length::Fill),
            dialog_button(
                value,
                kind,
                send(MirrorCardMsg::FieldFocused(field)),
                tokens
            ),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
    };
    let keep_linked = row![
        signex_widgets::checkbox(card.keep_linked)
            .on_toggle(move |on| LibraryMessage::PrimitiveEditorEvent {
                path: editor.path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchMirrorCard(
                    MirrorCardMsg::KeepLinkedToggled(on),
                )),
            })
            .size(TS.body)
            .spacing(0)
            .style(crate::panels::form_look::checkbox_style),
        text(t!(i18n, ids::SKETCH_TOOLS_MIRROR_KEEP_LINKED))
            .size(TS.caption)
            .color(theme_ext::text_primary(tokens)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let caption = if card.keep_linked {
        t!(i18n, ids::SKETCH_TOOLS_MIRROR_CAPTION_LINKED)
    } else {
        t!(i18n, ids::SKETCH_TOOLS_MIRROR_CAPTION_FREE)
    };
    let ready = sketch.and_then(|s| card.request(s)).is_some();
    let buttons = row![
        dialog_button(
            t!(i18n, ids::SKETCH_TOOLS_MIRROR_OK),
            DialogButtonKind::Primary,
            send(MirrorCardMsg::Confirmed).filter(|_| ready),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::SKETCH_TOOLS_MIRROR_CANCEL),
            DialogButtonKind::Secondary,
            send(MirrorCardMsg::Cancelled),
            tokens,
        ),
    ]
    .spacing(SP.s);
    let body = column![
        text(t!(i18n, ids::SKETCH_TOOLS_MIRROR_TITLE))
            .size(TS.body)
            .color(theme_ext::text_primary(tokens)),
        field_row(
            t!(i18n, ids::SKETCH_TOOLS_MIRROR_OBJECTS),
            objects_value,
            MirrorField::Objects
        ),
        field_row(
            t!(i18n, ids::SKETCH_TOOLS_MIRROR_LINE),
            line_value,
            MirrorField::Line
        ),
        keep_linked,
        text(caption)
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens)),
        buttons,
    ]
    .spacing(SP.m);
    let card = container(body)
        .padding([SP.m, SP.l])
        .style(crate::styles::modal_card(tokens));
    container(card)
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top)
        .boxed()
}

#[cfg(test)]
#[path = "mirror_card_tests.rs"]
mod tests;
