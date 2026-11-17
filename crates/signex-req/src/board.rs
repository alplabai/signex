//! `board_size` and `net_class`: what the project's board must be.

use std::collections::BTreeSet;

use serde_json::json;
use signex_types::net_class::{NetClass, first_matching_class, glob_matches};
use signex_types::pcb::PcbBoard;

use crate::{Board, Miss, mm};

/// Below a nanometre two lengths are the same (the file stores integer nm).
const SAME_MM: f64 = 1e-6;

/// The board, or the miss that says why there is none to check.
fn board_of(board: &Board) -> Result<(&str, &PcbBoard), Miss> {
    match board {
        Board::NotListed => Err(Miss::new("The project lists no board")),
        Board::Unreadable { path, why } => Err(Miss::new(format!(
            "The board {path} could not be read: {why}"
        ))
        .with("reason", json!(why))
        .on(path)),
        Board::Read { path, board } => Ok((path, board)),
    }
}

/// A limit in mm that is no positive, finite length.
fn bad_limit(name: &str, value: f64) -> Option<Miss> {
    (!value.is_finite() || value <= 0.0).then(|| {
        Miss::new(format!(
            "`{name}` is {value}, not a positive length in mm, so the requirement was not checked"
        ))
    })
}

/// `board_size`: the outline's bounding box against the limits.
pub(crate) fn board_size(board: &Board, max_width_mm: f64, max_height_mm: f64) -> Vec<Miss> {
    let bad = [
        bad_limit("max_width_mm", max_width_mm),
        bad_limit("max_height_mm", max_height_mm),
    ];
    let bad: Vec<Miss> = bad.into_iter().flatten().collect();
    if !bad.is_empty() {
        return bad;
    }
    let (path, board) = match board_of(board) {
        Ok(found) => found,
        Err(miss) => return vec![miss],
    };
    let Some((width, height)) = outline_extent(board) else {
        return vec![Miss::new("The board has no outline to measure").on(path)];
    };
    if width <= max_width_mm + SAME_MM && height <= max_height_mm + SAME_MM {
        return Vec::new();
    }
    let miss = Miss::new(format!(
        "The board is {} x {} mm, over the limit of {} x {} mm",
        mm(width),
        mm(height),
        mm(max_width_mm),
        mm(max_height_mm)
    ))
    .with("width_mm", json!(width))
    .with("height_mm", json!(height))
    .with("max_width_mm", json!(max_width_mm))
    .with("max_height_mm", json!(max_height_mm))
    .on(path);
    vec![miss]
}

/// The width and height of the outline's bounding box, in mm; `None` for an
/// outline of fewer than three points or one with a point that is not
/// finite.
pub(crate) fn outline_extent(board: &PcbBoard) -> Option<(f64, f64)> {
    let points = &board.outline;
    if points.len() < 3 || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return None;
    }
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in points {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }
    Some((max_x - min_x, max_y - min_y))
}

/// What a `net_class` requirement asks.
pub(crate) struct NetClassWanted<'a> {
    pub(crate) nets: &'a str,
    pub(crate) class: Option<&'a str>,
    pub(crate) min_trace_width_mm: Option<f64>,
    pub(crate) min_clearance_mm: Option<f64>,
}

impl NetClassWanted<'_> {
    fn has_minimums(&self) -> bool {
        self.min_trace_width_mm.is_some() || self.min_clearance_mm.is_some()
    }

    /// Why the requirement cannot be checked as written.
    fn problems(&self) -> Vec<Miss> {
        let mut out = Vec::new();
        if self.nets.trim().is_empty() {
            out.push(Miss::new(
                "`nets` is empty, so the requirement was not checked",
            ));
        }
        if self.class.is_none() && !self.has_minimums() {
            out.push(Miss::new(
                "The requirement names no class and no minimum, so there is nothing to check",
            ));
        }
        let limits = [
            ("min_trace_width_mm", self.min_trace_width_mm),
            ("min_clearance_mm", self.min_clearance_mm),
        ];
        for (name, value) in limits {
            if let Some(value) = value.and_then(|v| bad_limit(name, v)) {
                out.push(value);
            }
        }
        out
    }
}

/// `net_class`: the board nets matching `nets` are in the class, and their
/// classes set the minimum track width and clearance.
pub(crate) fn net_class(board: &Board, classes: &[NetClass], wanted: &NetClassWanted) -> Vec<Miss> {
    let problems = wanted.problems();
    if !problems.is_empty() {
        return problems;
    }
    let (path, board) = match board_of(board) {
        Ok(found) => found,
        Err(miss) => return vec![miss],
    };
    if let Some(name) = wanted.class
        && !classes.iter().any(|c| c.name == name)
    {
        return vec![Miss::new(format!("The project has no net class {name}")).on(path)];
    }
    let nets: BTreeSet<&str> = board
        .nets
        .iter()
        .map(|net| net.name.as_str())
        .filter(|name| !name.is_empty() && glob_matches(wanted.nets, name))
        .collect();
    if nets.is_empty() {
        let miss = Miss::new(format!("No net of the board matches {}", wanted.nets));
        return vec![miss.with("nets", json!(wanted.nets)).on(path)];
    }
    let mut misses = Vec::new();
    let mut in_classes: BTreeSet<&str> = BTreeSet::new();
    for net in nets {
        let class = first_matching_class(classes, net);
        if let Some(class) = class {
            in_classes.insert(class.name.as_str());
        }
        let actual = class.map(|c| c.name.as_str());
        let message = match (wanted.class, actual) {
            (Some(name), Some(actual)) if actual == name => continue,
            (Some(name), Some(actual)) => format!("Net {net} is in class {actual}, not {name}"),
            (Some(name), None) => format!("Net {net} is in no class, not {name}"),
            (None, Some(_)) => continue,
            (None, None) => format!("Net {net} is in no class, so no class sets its minimums"),
        };
        let miss = Miss::new(message)
            .with("net", json!(net))
            .with("class", json!(actual))
            .on(path)
            .about(format!("net:{net}"));
        misses.push(miss);
    }
    for name in in_classes {
        let Some(class) = classes.iter().find(|c| c.name == name) else {
            continue;
        };
        let limits = [
            ("track width", "trace_width", class.trace_width, wanted.min_trace_width_mm),
            ("clearance", "clearance", class.clearance, wanted.min_clearance_mm),
        ];
        for (words, field, set, min) in limits {
            let Some(min) = min else { continue };
            if set > 0.0 && set + SAME_MM >= min {
                continue;
            }
            let message = if set > 0.0 {
                format!(
                    "Class {name} sets a {words} of {} mm, under {} mm",
                    mm(set),
                    mm(min)
                )
            } else {
                format!(
                    "Class {name} sets no {words}; at least {} mm is required",
                    mm(min)
                )
            };
            let miss = Miss::new(message)
                .with("class", json!(name))
                .with(&format!("{field}_mm"), json!(set))
                .with(&format!("min_{field}_mm"), json!(min))
                .on(path)
                .about(format!("class:{name}:{field}"));
            misses.push(miss);
        }
    }
    misses
}
