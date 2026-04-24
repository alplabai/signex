//! Wire, bus, and bus-entry rendering.
//!
//! Connected wire segments that share the same colour are chained into a
//! single polyline path so that 45° and 90° corners are drawn with
//! `LineJoin::Round` instead of producing square-cap artefacts.

use std::collections::{HashMap, HashSet};

use iced::Color;
use iced::widget::canvas::{self, LineCap, LineJoin, path};

use signex_types::schematic::{Bus, BusEntry, Wire};

use super::ScreenTransform;

/// KiCad default wire stroke width in mm (when wire.stroke_width == 0.0).
pub(crate) const WIRE_DEFAULT_WIDTH_MM: f64 = 0.15;
/// KiCad default bus stroke width in mm.
const BUS_DEFAULT_WIDTH_MM: f64 = 0.5;

// ---------------------------------------------------------------------------
// Key helpers for grouping
// ---------------------------------------------------------------------------

/// Round a world coordinate to 1 µm precision for adjacency hashing.
fn pt_key(x: f64, y: f64) -> (i64, i64) {
    ((x * 1_000.0).round() as i64, (y * 1_000.0).round() as i64)
}

/// Discrete RGBA key so wires of the same visual colour can be grouped.
fn color_key(c: Color) -> (u8, u8, u8, u8) {
    (
        (c.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// Screen width rounded to 0.01 px so nearly-identical widths collapse.
fn width_key(px: f32) -> i32 {
    (px * 100.0).round() as i32
}

type GroupKey = ((u8, u8, u8, u8), i32);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Draw all wires.
///
/// Same-colour, connected segments are chained into polylines so corners
/// receive proper `LineJoin::Round` instead of clashing square caps.
/// `color_for` returns the resolved display colour for each wire.
pub fn draw_wires(
    frame: &mut canvas::Frame,
    wires: &[Wire],
    transform: &ScreenTransform,
    color_for: impl Fn(&Wire) -> Color,
) {
    if wires.is_empty() {
        return;
    }

    // Compute (group_key, color, screen_width) per wire.
    let keyed: Vec<(GroupKey, Color, f32)> = wires
        .iter()
        .map(|w| {
            let color = color_for(w);
            let mm = if w.stroke_width > 0.0 {
                w.stroke_width
            } else {
                WIRE_DEFAULT_WIDTH_MM
            };
            let px = transform.world_len(mm).max(1.0);
            let key = (color_key(color), width_key(px));
            (key, color, px)
        })
        .collect();

    // Partition indices by group key.
    let mut groups: HashMap<GroupKey, Vec<usize>> = HashMap::new();
    for (i, (key, _, _)) in keyed.iter().enumerate() {
        groups.entry(*key).or_default().push(i);
    }

    // Chain and draw each group.
    for (key, indices) in &groups {
        let (_, color, px) = keyed[indices[0]];
        let _ = key;
        draw_wire_group(frame, wires, indices, transform, color, px);
    }
}

/// Draw a bus (thick line, no chaining needed — buses are already polylines
/// in KiCad but are represented as individual segments here).
pub fn draw_bus(frame: &mut canvas::Frame, bus: &Bus, transform: &ScreenTransform, color: Color) {
    let p1 = transform.to_screen_point(bus.start.x, bus.start.y);
    let p2 = transform.to_screen_point(bus.end.x, bus.end.y);

    let width = transform.world_len(BUS_DEFAULT_WIDTH_MM).max(2.0);
    let line = canvas::Path::line(p1, p2);
    let stroke = canvas::Stroke {
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..canvas::Stroke::default()
            .with_color(color)
            .with_width(width)
    };
    frame.stroke(&line, stroke);
}

/// Draw a bus entry diagonal.
pub fn draw_bus_entry(
    frame: &mut canvas::Frame,
    entry: &BusEntry,
    transform: &ScreenTransform,
    color: Color,
) {
    let p1 = transform.to_screen_point(entry.position.x, entry.position.y);
    let p2 = transform.to_screen_point(
        entry.position.x + entry.size.0,
        entry.position.y + entry.size.1,
    );

    let path = canvas::Path::new(|b: &mut path::Builder| {
        b.move_to(p1);
        b.line_to(p2);
    });

    let width = transform.world_len(WIRE_DEFAULT_WIDTH_MM).max(1.0);
    let stroke = canvas::Stroke {
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..canvas::Stroke::default()
            .with_color(color)
            .with_width(width)
    };
    frame.stroke(&path, stroke);
}

// ---------------------------------------------------------------------------
// Internal: chaining
// ---------------------------------------------------------------------------

fn draw_wire_group(
    frame: &mut canvas::Frame,
    wires: &[Wire],
    indices: &[usize],
    transform: &ScreenTransform,
    color: Color,
    screen_width: f32,
) {
    // Build: point_key → list of wire indices that touch that point.
    let mut pt_map: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for &idx in indices {
        let w = &wires[idx];
        pt_map
            .entry(pt_key(w.start.x, w.start.y))
            .or_default()
            .push(idx);
        pt_map
            .entry(pt_key(w.end.x, w.end.y))
            .or_default()
            .push(idx);
    }

    // Collect chain start candidates: endpoints where exactly 1 wire touches
    // (degree-1 = dead end; chain starts there).
    let mut starts: Vec<(usize, bool)> = Vec::new(); // (wire_idx, enter_from_start_end)
    for &idx in indices {
        let w = &wires[idx];
        if pt_map[&pt_key(w.start.x, w.start.y)].len() == 1 {
            starts.push((idx, true));
        }
        if pt_map[&pt_key(w.end.x, w.end.y)].len() == 1 {
            starts.push((idx, false));
        }
    }

    let mut visited: HashSet<usize> = HashSet::new();
    let mut chains: Vec<Vec<iced::Point>> = Vec::new();

    // Trace from degree-1 endpoints first.
    for (start_idx, from_start) in &starts {
        if visited.contains(start_idx) {
            continue;
        }
        let chain = trace_chain(
            *start_idx,
            *from_start,
            wires,
            &pt_map,
            &mut visited,
            transform,
        );
        if chain.len() >= 2 {
            chains.push(chain);
        }
    }

    // Handle any remaining unvisited segments (loops / isolated segments in
    // T-junction clusters that were never reached from a dead end).
    for &idx in indices {
        if visited.contains(&idx) {
            continue;
        }
        let chain = trace_chain(idx, true, wires, &pt_map, &mut visited, transform);
        if chain.len() >= 2 {
            chains.push(chain);
        }
    }

    // Render all chains.
    for chain in &chains {
        if chain.len() < 2 {
            continue;
        }
        let p = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(chain[0]);
            for &pt in &chain[1..] {
                b.line_to(pt);
            }
        });
        let stroke = canvas::Stroke {
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..canvas::Stroke::default()
                .with_color(color)
                .with_width(screen_width)
        };
        frame.stroke(&p, stroke);
    }
}

/// Follow a chain starting at one end of `start_idx`.
///
/// `from_start = true`  → we enter the wire at its `start` point and exit
///                         towards its `end` point.
/// `from_start = false` → we enter at `end` and exit towards `start`.
///
/// The chain continues as long as the exit point has degree exactly 2
/// (i.e. exactly one other same-colour wire continues from there).
/// At junctions (degree ≥ 3) or dead ends (degree 1) the chain stops.
fn trace_chain(
    start_idx: usize,
    from_start: bool,
    wires: &[Wire],
    pt_map: &HashMap<(i64, i64), Vec<usize>>,
    visited: &mut HashSet<usize>,
    transform: &ScreenTransform,
) -> Vec<iced::Point> {
    let mut chain: Vec<iced::Point> = Vec::new();
    let mut cur_idx = start_idx;
    let mut enter_from_start = from_start;

    loop {
        if visited.contains(&cur_idx) {
            break;
        }
        visited.insert(cur_idx);

        let w = &wires[cur_idx];
        let sk = pt_key(w.start.x, w.start.y);
        let ek = pt_key(w.end.x, w.end.y);

        let (entry_screen, exit_screen, exit_key) = if enter_from_start {
            (
                transform.to_screen_point(w.start.x, w.start.y),
                transform.to_screen_point(w.end.x, w.end.y),
                ek,
            )
        } else {
            (
                transform.to_screen_point(w.end.x, w.end.y),
                transform.to_screen_point(w.start.x, w.start.y),
                sk,
            )
        };

        if chain.is_empty() {
            chain.push(entry_screen);
        }
        chain.push(exit_screen);

        // Continue only when the exit point has exactly 2 wires in this group
        // (one we came from, one to follow).
        let neighbors = match pt_map.get(&exit_key) {
            Some(n) => n,
            None => break,
        };
        if neighbors.len() != 2 {
            break;
        }
        let next_idx = match neighbors.iter().copied().find(|&ni| ni != cur_idx) {
            Some(ni) => ni,
            None => break,
        };
        if visited.contains(&next_idx) {
            break;
        }

        // Determine which end of the next wire we enter from.
        let nw = &wires[next_idx];
        enter_from_start = pt_key(nw.start.x, nw.start.y) == exit_key;
        cur_idx = next_idx;
    }

    chain
}
