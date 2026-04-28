//! Symbol-tab editor state.
//!
//! The editor mutates a typed [`signex_library::Symbol`] primitive
//! in-place. Helpers below operate on a `&mut Symbol` so the
//! dispatcher can call them directly off the active editor state.
//!
//! Selection / hit-test / pin-add / move / delete logic preserves
//! the canvas + AI-stub apply behaviour the pre-refactor `SymbolDoc`
//! had.

use signex_library::{
    PinElectricalType, PinOrientation, Symbol, SymbolGraphicKind, SymbolPin,
};

/// Coarse pin classification — kept independent of the canonical
/// [`PinElectricalType`] so the AI-stub heuristic can hand back a
/// limited subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinKind {
    Input,
    Output,
    Bidirectional,
    Power,
    Passive,
    Unknown,
}

impl PinKind {
    pub fn from_ai_stub(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "input" => PinKind::Input,
            "output" => PinKind::Output,
            "bidirectional" | "bidir" => PinKind::Bidirectional,
            "power" | "power_in" | "power_out" => PinKind::Power,
            "passive" => PinKind::Passive,
            _ => PinKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKey {
    Reference,
    Value,
}

/// Selected element on the Symbol canvas — drives delete + drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolSelection {
    Pin(usize),
    Field(FieldKey),
}

/// Resize-handle identity for a placed [`SymbolGraphic`]. Each
/// variant identifies one grabbable point on the graphic so the
/// canvas can fire [`canvas::CanvasAction::MoveGraphicHandle`] with
/// enough context for the dispatcher to mutate the right field.
///
/// Corner ordering for `RectCorner`: `0=TL, 1=TR, 2=BR, 3=BL` in the
/// Standard y-up world (so TL has minx + maxy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicHandle {
    /// Rectangle corner — `0=TL, 1=TR, 2=BR, 3=BL`.
    RectCorner(u8),
    /// Line endpoint — `0=from, 1=to`.
    LineEndpoint(u8),
    /// Circle radius handle (drawn at `(center.x + radius, center.y)`).
    CircleRadius,
    /// Arc start point on the circumference.
    ArcStart,
    /// Arc end point on the circumference.
    ArcEnd,
    /// Text anchor / `position` field.
    TextAnchor,
}

/// Default new-pin layout: place new pins to the right of the body.
const DEFAULT_PIN_LENGTH_MM: f64 = 2.54;

/// Add a pin at the given canvas coordinates and return its index in
/// `Symbol::pins`. Auto-assigns the next free numeric pin number.
pub fn add_pin(sym: &mut Symbol, x: f64, y: f64) -> usize {
    let next_num = next_pin_number(sym);
    let mut pin = SymbolPin::new(next_num.clone(), format!("PIN{next_num}"));
    pin.position = [x, y];
    pin.length = DEFAULT_PIN_LENGTH_MM;
    sym.pins.push(pin);
    sym.pins.len() - 1
}

/// Pick the next integer pin number — one above the highest numeric
/// pin number, or `"1"` if no numeric pins exist.
fn next_pin_number(sym: &Symbol) -> String {
    let highest = sym
        .pins
        .iter()
        .filter_map(|p| p.number.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (highest + 1).to_string()
}

/// Move the currently-selected element to a new canvas position.
/// Coordinates are in mm; callers should snap to the grid first.
pub fn move_selected(sym: &mut Symbol, sel: Option<SymbolSelection>, x: f64, y: f64) {
    if let Some(SymbolSelection::Pin(idx)) = sel
        && let Some(pin) = sym.pins.get_mut(idx)
    {
        pin.position = [x, y];
    }
    // SymbolSelection::Field — no-op; the on-canvas designator /
    // value drag re-binds against `ComponentRow` once that pipeline
    // ships.
}

/// Delete whatever is currently selected. Returns `Some(new_sel)` if
/// the caller should update its selection (typically `None` after a
/// pin removal), or `None` if no selection change is needed.
pub fn delete_selected(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
) -> Option<Option<SymbolSelection>> {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if idx < sym.pins.len() {
                sym.pins.remove(idx);
                Some(None)
            } else {
                None
            }
        }
        Some(SymbolSelection::Field(_)) => None,
        None => None,
    }
}

/// Hit-test cursor world coordinates against pins.
pub fn hit_test(sym: &Symbol, x: f64, y: f64) -> Option<SymbolSelection> {
    const PIN_HIT_R_SQ: f64 = 1.5 * 1.5;
    for (i, pin) in sym.pins.iter().enumerate() {
        let dx = pin.position[0] - x;
        let dy = pin.position[1] - y;
        if dx * dx + dy * dy <= PIN_HIT_R_SQ {
            return Some(SymbolSelection::Pin(i));
        }
    }
    None
}

/// Hit radius for graphic resize handles — same 1.5 mm budget as the
/// pin click target so the gesture feels consistent across the canvas.
const HANDLE_HIT_R_SQ: f64 = 1.5 * 1.5;

/// Compute the world (mm) position of a graphic's resize handle.
/// Returns `None` if the handle variant doesn't match the graphic
/// kind — defensive against stale `GraphicHandle` values lingering
/// across selection swaps.
pub fn graphic_handle_position(
    sym: &Symbol,
    idx: usize,
    handle: GraphicHandle,
) -> Option<[f64; 2]> {
    let g = sym.graphics.get(idx)?;
    Some(match (&g.kind, handle) {
        (SymbolGraphicKind::Rectangle { from, to }, GraphicHandle::RectCorner(c)) => match c {
            0 => [from[0], to[1]],   // TL
            1 => [to[0], to[1]],     // TR
            2 => [to[0], from[1]],   // BR
            3 => [from[0], from[1]], // BL
            _ => return None,
        },
        (SymbolGraphicKind::Line { from, to }, GraphicHandle::LineEndpoint(e)) => match e {
            0 => *from,
            1 => *to,
            _ => return None,
        },
        (SymbolGraphicKind::Circle { center, radius }, GraphicHandle::CircleRadius) => {
            [center[0] + radius, center[1]]
        }
        (
            SymbolGraphicKind::Arc {
                center,
                radius,
                start_deg,
                ..
            },
            GraphicHandle::ArcStart,
        ) => {
            let s = start_deg.to_radians();
            [center[0] + radius * s.cos(), center[1] + radius * s.sin()]
        }
        (
            SymbolGraphicKind::Arc {
                center,
                radius,
                end_deg,
                ..
            },
            GraphicHandle::ArcEnd,
        ) => {
            let e = end_deg.to_radians();
            [center[0] + radius * e.cos(), center[1] + radius * e.sin()]
        }
        (SymbolGraphicKind::Text { position, .. }, GraphicHandle::TextAnchor) => *position,
        _ => return None,
    })
}

/// Enumerate every resize handle for the graphic at `idx` (variant
/// + world position). Used by the canvas to draw the handle squares
/// when the Select tool is active.
pub fn graphic_handles(sym: &Symbol, idx: usize) -> Vec<(GraphicHandle, [f64; 2])> {
    let Some(g) = sym.graphics.get(idx) else {
        return Vec::new();
    };
    match &g.kind {
        SymbolGraphicKind::Rectangle { from, to } => vec![
            (GraphicHandle::RectCorner(0), [from[0], to[1]]),
            (GraphicHandle::RectCorner(1), [to[0], to[1]]),
            (GraphicHandle::RectCorner(2), [to[0], from[1]]),
            (GraphicHandle::RectCorner(3), [from[0], from[1]]),
        ],
        SymbolGraphicKind::Line { from, to } => vec![
            (GraphicHandle::LineEndpoint(0), *from),
            (GraphicHandle::LineEndpoint(1), *to),
        ],
        SymbolGraphicKind::Circle { center, radius } => {
            vec![(GraphicHandle::CircleRadius, [center[0] + radius, center[1]])]
        }
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let s = start_deg.to_radians();
            let e = end_deg.to_radians();
            vec![
                (
                    GraphicHandle::ArcStart,
                    [center[0] + radius * s.cos(), center[1] + radius * s.sin()],
                ),
                (
                    GraphicHandle::ArcEnd,
                    [center[0] + radius * e.cos(), center[1] + radius * e.sin()],
                ),
            ]
        }
        SymbolGraphicKind::Text { position, .. } => {
            vec![(GraphicHandle::TextAnchor, *position)]
        }
    }
}

/// Hit-test world coordinates against every placed graphic's resize
/// handles. Returns `(graphic_idx, handle)` for the first hit, scanning
/// graphics in reverse so the most-recently-placed graphic wins when
/// handles overlap.
pub fn hit_test_graphic_handle(
    sym: &Symbol,
    x: f64,
    y: f64,
) -> Option<(usize, GraphicHandle)> {
    for idx in (0..sym.graphics.len()).rev() {
        for (handle, pos) in graphic_handles(sym, idx) {
            let dx = pos[0] - x;
            let dy = pos[1] - y;
            if dx * dx + dy * dy <= HANDLE_HIT_R_SQ {
                return Some((idx, handle));
            }
        }
    }
    None
}

/// Move the named handle of the graphic at `idx` to world coordinates
/// `(x, y)`. No-op when `idx` is out of range or the handle variant
/// doesn't match the graphic kind. For arc endpoints the handle drag
/// only updates the angle (radius is preserved) so the user can sweep
/// the arc without resizing it.
pub fn move_graphic_handle(
    sym: &mut Symbol,
    idx: usize,
    handle: GraphicHandle,
    x: f64,
    y: f64,
) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };
    match (&mut g.kind, handle) {
        (SymbolGraphicKind::Rectangle { from, to }, GraphicHandle::RectCorner(c)) => match c {
            0 => {
                from[0] = x;
                to[1] = y;
            }
            1 => {
                to[0] = x;
                to[1] = y;
            }
            2 => {
                to[0] = x;
                from[1] = y;
            }
            3 => {
                from[0] = x;
                from[1] = y;
            }
            _ => {}
        },
        (SymbolGraphicKind::Line { from, .. }, GraphicHandle::LineEndpoint(0)) => {
            from[0] = x;
            from[1] = y;
        }
        (SymbolGraphicKind::Line { to, .. }, GraphicHandle::LineEndpoint(1)) => {
            to[0] = x;
            to[1] = y;
        }
        (SymbolGraphicKind::Circle { center, radius }, GraphicHandle::CircleRadius) => {
            let dx = x - center[0];
            let dy = y - center[1];
            // Floor at 0.1 mm so a click on the centre doesn't make
            // the circle vanish — matches the pin-length floor.
            *radius = (dx * dx + dy * dy).sqrt().max(0.1);
        }
        (
            SymbolGraphicKind::Arc {
                center, start_deg, ..
            },
            GraphicHandle::ArcStart,
        ) => {
            *start_deg = (y - center[1]).atan2(x - center[0]).to_degrees();
        }
        (
            SymbolGraphicKind::Arc {
                center, end_deg, ..
            },
            GraphicHandle::ArcEnd,
        ) => {
            *end_deg = (y - center[1]).atan2(x - center[0]).to_degrees();
        }
        (SymbolGraphicKind::Text { position, .. }, GraphicHandle::TextAnchor) => {
            position[0] = x;
            position[1] = y;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::Symbol;

    #[test]
    fn add_pin_assigns_next_number() {
        let mut s = Symbol::empty("test");
        // Symbol::empty seeds one default pin "1".
        let idx = add_pin(&mut s, 1.0, 1.0);
        assert_eq!(idx, 1);
        assert_eq!(s.pins[1].number, "2");
    }

    #[test]
    fn delete_pin_clears_selection_via_return() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 1.0, 1.0);
        let new_sel = delete_selected(&mut s, Some(SymbolSelection::Pin(0)));
        assert_eq!(new_sel, Some(None));
        assert_eq!(s.pins.len(), 1);
    }

    #[test]
    fn move_selected_updates_position() {
        let mut s = Symbol::empty("test");
        move_selected(&mut s, Some(SymbolSelection::Pin(0)), 5.5, -2.0);
        assert_eq!(s.pins[0].position, [5.5, -2.0]);
    }

    #[test]
    fn hit_test_returns_pin() {
        let mut s = Symbol::empty("test");
        s.pins[0].position = [3.0, 4.0];
        let sel = hit_test(&s, 3.0, 4.0);
        assert_eq!(sel, Some(SymbolSelection::Pin(0)));
    }

    #[test]
    fn graphic_handle_position_returns_rectangle_corners() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [-2.0, -1.0],
                to: [2.0, 1.0],
            },
            stroke_width: 0.15,
        });
        // TL = (from.x, to.y), BR = (to.x, from.y)
        assert_eq!(
            graphic_handle_position(&s, 0, GraphicHandle::RectCorner(0)),
            Some([-2.0, 1.0])
        );
        assert_eq!(
            graphic_handle_position(&s, 0, GraphicHandle::RectCorner(2)),
            Some([2.0, -1.0])
        );
    }

    #[test]
    fn hit_test_graphic_handle_finds_rectangle_corner() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [10.0, 5.0],
            },
            stroke_width: 0.15,
        });
        // BR corner is at (to.x, from.y) = (10.0, 0.0).
        let hit = hit_test_graphic_handle(&s, 10.0, 0.0);
        assert_eq!(hit, Some((0, GraphicHandle::RectCorner(2))));
    }

    #[test]
    fn move_graphic_handle_moves_line_endpoint() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [5.0, 0.0],
            },
            stroke_width: 0.15,
        });
        move_graphic_handle(&mut s, 0, GraphicHandle::LineEndpoint(1), 7.0, 3.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Line { to, .. } => assert_eq!(*to, [7.0, 3.0]),
            _ => panic!("expected Line"),
        }
    }

    #[test]
    fn move_graphic_handle_resizes_circle_radius() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Circle {
                center: [0.0, 0.0],
                radius: 1.0,
            },
            stroke_width: 0.15,
        });
        move_graphic_handle(&mut s, 0, GraphicHandle::CircleRadius, 3.0, 4.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Circle { radius, .. } => assert!((*radius - 5.0).abs() < 1e-9),
            _ => panic!("expected Circle"),
        }
    }

    #[test]
    fn move_graphic_handle_no_op_for_mismatched_variant() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [5.0, 0.0],
            },
            stroke_width: 0.15,
        });
        // Asking to move a rectangle corner on a Line — should silently no-op.
        move_graphic_handle(&mut s, 0, GraphicHandle::RectCorner(0), 99.0, 99.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Line { from, to } => {
                assert_eq!(*from, [0.0, 0.0]);
                assert_eq!(*to, [5.0, 0.0]);
            }
            _ => panic!("expected Line"),
        }
    }
}
