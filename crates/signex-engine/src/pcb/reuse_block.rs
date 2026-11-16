//! Design reuse blocks (#2410): a placed and routed group of a board saved
//! once and placed again on another channel of the same circuit. Pure:
//! [`capture`] cuts the block out of a board, [`place`] returns the commands
//! that lay it onto a board as one undo step. Storage is the caller's.
//!
//! A block footprint finds its counterpart through the schematic: a part of
//! a sheet placed more than once keeps its unit uuids in every placement, so
//! the counterpart is the footprint linked to the same units on the target
//! channel's sheet placement. The channel is the one of the anchor, a
//! footprint the caller names; the anchor keeps its place and the rest of
//! the block lands around it. Nets follow the pads: a block net takes the
//! target net of the pads it reaches. Rules: `docs/PCB_TOOLS.md`, "Design
//! reuse blocks".

use std::collections::HashMap;

use signex_types::pcb::{Footprint, PcbBoard, Point, SymbolLink};
use uuid::Uuid;

use super::command::BoardCommand;
use super::items::{BoardItem, BoardItemId};

/// Why a block cannot be saved or placed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReuseRefusal {
    #[error("a block needs at least one footprint")]
    NoFootprint,
    #[error("an item of the selection is no longer on the board")]
    Missing,
    #[error("{0} has no schematic part, so the block cannot find its counterpart")]
    Unlinked(String),
    #[error("{0} is not a part of the circuit this block was saved from")]
    AnchorNotInBlock(String),
    #[error("no part of {anchor}'s channel matches {block} of the block")]
    NoMatch { anchor: String, block: String },
    #[error("{0} is locked")]
    Locked(String),
    #[error("block net {net} meets nets {first} and {second} on the board")]
    NetConflict {
        net: String,
        first: String,
        second: String,
    },
    #[error("the turn is not a number")]
    BadTurn,
}

/// What a placement does, for the caller's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ReusePlacement {
    pub commands: Vec<BoardCommand>,
    /// Each block footprint's designator and the one it placed.
    pub footprints: Vec<(String, String)>,
    pub tracks: usize,
    pub vias: usize,
}

/// The block `items` of `board` make: their footprints, tracks and vias,
/// moved so the first footprint by designator sits at the origin, with the
/// nets they use. Zones, drawings and texts are not carried. Refused for an
/// item that is gone, a block with no footprint and a footprint with no
/// schematic part.
pub fn capture(board: &PcbBoard, items: &[BoardItemId]) -> Result<PcbBoard, ReuseRefusal> {
    let mut block = board.clone();
    let wanted = |id: BoardItemId| items.contains(&id);
    block
        .footprints
        .retain(|fp| wanted(BoardItemId::Footprint(fp.uuid)));
    block
        .segments
        .retain(|s| wanted(BoardItemId::Segment(s.uuid)));
    block.vias.retain(|v| wanted(BoardItemId::Via(v.uuid)));
    let found = block.footprints.len() + block.segments.len() + block.vias.len();
    let asked = items
        .iter()
        .filter(|id| {
            matches!(
                id,
                BoardItemId::Footprint(_) | BoardItemId::Segment(_) | BoardItemId::Via(_)
            )
        })
        .count();
    if found != asked {
        return Err(ReuseRefusal::Missing);
    }
    if let Some(fp) = block
        .footprints
        .iter()
        .find(|fp| fp.schematic_link.is_none())
    {
        return Err(ReuseRefusal::Unlinked(fp.reference.clone()));
    }
    let origin = block
        .footprints
        .iter()
        .min_by(|a, b| a.reference.cmp(&b.reference))
        .map(|fp| fp.position)
        .ok_or(ReuseRefusal::NoFootprint)?;
    let shift = |p: &mut Point| *p = Point::new(p.x - origin.x, p.y - origin.y);
    block
        .footprints
        .iter_mut()
        .for_each(|fp| shift(&mut fp.position));
    for segment in &mut block.segments {
        shift(&mut segment.start);
        shift(&mut segment.end);
    }
    block.vias.iter_mut().for_each(|v| shift(&mut v.position));
    let used: Vec<u32> = block
        .segments
        .iter()
        .map(|s| s.net)
        .chain(block.vias.iter().map(|v| v.net))
        .chain(
            block
                .footprints
                .iter()
                .flat_map(|fp| fp.pads.iter().filter_map(|p| p.net.as_ref()))
                .map(|net| net.number),
        )
        .collect();
    block.nets.retain(|net| used.contains(&net.number));
    block.outline.clear();
    block.zones.clear();
    block.graphics.clear();
    block.texts.clear();
    block.sketch = None;
    block.unknown = Default::default();
    Ok(block)
}

/// The commands that lay `block` onto `target` around `anchor`, a footprint
/// of `target`: each block footprint's counterpart on the anchor's channel
/// is moved, turned and put on the block's side so the anchor keeps its
/// place, the whole turned by `degrees` (counter-clockwise), and the block's
/// tracks and vias are added with ids from `new_id` on the nets their pads
/// reach. Refused, with nothing built, for an anchor or counterpart with no
/// schematic part, an anchor that is not in the block, a block footprint
/// with no counterpart, a locked counterpart and a block net that meets two
/// board nets.
pub fn place(
    block: &PcbBoard,
    target: &PcbBoard,
    anchor: Uuid,
    degrees: f64,
    new_id: &mut impl FnMut() -> Uuid,
) -> Result<ReusePlacement, ReuseRefusal> {
    if !degrees.is_finite() {
        return Err(ReuseRefusal::BadTurn);
    }
    let anchor_fp = target
        .footprints
        .iter()
        .find(|fp| fp.uuid == anchor)
        .ok_or(ReuseRefusal::Missing)?;
    let anchor_name = anchor_fp.reference.clone();
    let channel = anchor_fp
        .schematic_link
        .as_ref()
        .ok_or_else(|| ReuseRefusal::Unlinked(anchor_name.clone()))?;
    let pivot = block
        .footprints
        .iter()
        .find(|b| link_of(b).is_some_and(|l| shares_units(l, channel)))
        .ok_or_else(|| ReuseRefusal::AnchorNotInBlock(anchor_name.clone()))?;
    let turn = Turn::new(pivot.position, anchor_fp.position, degrees);
    let mut pairs: Vec<(&Footprint, &Footprint)> = Vec::new();
    for source in &block.footprints {
        let link =
            link_of(source).ok_or_else(|| ReuseRefusal::Unlinked(source.reference.clone()))?;
        let counterpart = target
            .footprints
            .iter()
            .find(|t| {
                t.schematic_link
                    .as_ref()
                    .is_some_and(|tl| tl.sheet == channel.sheet && shares_units(tl, link))
            })
            .ok_or_else(|| ReuseRefusal::NoMatch {
                anchor: anchor_name.clone(),
                block: source.reference.clone(),
            })?;
        if counterpart.locked {
            return Err(ReuseRefusal::Locked(counterpart.reference.clone()));
        }
        pairs.push((source, counterpart));
    }
    let nets = net_map(&pairs)?;
    let mut commands = Vec::new();
    for (source, counterpart) in &pairs {
        commands.extend(footprint_commands(source, counterpart, &turn, degrees));
    }
    let number_of = |block_net: u32| -> u32 {
        let Some(name) = block
            .nets
            .iter()
            .find(|n| n.number == block_net)
            .map(|n| &n.name)
        else {
            return 0;
        };
        let target_name = match nets.get(name.as_str()) {
            Some(Some(mapped)) => mapped.as_str(),
            Some(None) => return 0,
            None => name.as_str(),
        };
        target
            .nets
            .iter()
            .find(|n| n.name == target_name)
            .map_or(0, |n| n.number)
    };
    let mut items = Vec::new();
    for source in &block.segments {
        let mut segment = source.clone();
        segment.uuid = new_id();
        segment.start = turn.apply(segment.start);
        segment.end = turn.apply(segment.end);
        segment.net = number_of(segment.net);
        items.push(BoardItem::Segment(segment));
    }
    for source in &block.vias {
        let mut via = source.clone();
        via.uuid = new_id();
        via.position = turn.apply(via.position);
        via.net = number_of(via.net);
        items.push(BoardItem::Via(via));
    }
    let (tracks, vias) = (block.segments.len(), block.vias.len());
    if !items.is_empty() {
        commands.push(BoardCommand::AddItems { items });
    }
    Ok(ReusePlacement {
        commands,
        footprints: pairs
            .iter()
            .map(|(s, c)| (s.reference.clone(), c.reference.clone()))
            .collect(),
        tracks,
        vias,
    })
}

fn link_of(fp: &Footprint) -> Option<&SymbolLink> {
    fp.schematic_link.as_ref()
}

fn shares_units(a: &SymbolLink, b: &SymbolLink) -> bool {
    a.units.iter().any(|u| b.units.contains(u))
}

/// Each block net's board net, read from the pads of each pair: `None` when
/// the counterpart pads have no net.
fn net_map<'a>(
    pairs: &[(&'a Footprint, &Footprint)],
) -> Result<HashMap<&'a str, Option<String>>, ReuseRefusal> {
    let mut map: HashMap<&'a str, Option<String>> = HashMap::new();
    for &(source, counterpart) in pairs {
        for pad in &source.pads {
            let Some(block_net) = pad.net.as_ref() else {
                continue;
            };
            let name = block_net.name.as_str();
            let board_net = counterpart
                .pads
                .iter()
                .find(|p| p.number == pad.number)
                .and_then(|p| p.net.as_ref())
                .map(|n| n.name.clone());
            match (map.get(name).cloned().flatten(), board_net) {
                (Some(first), Some(second)) if first != second => {
                    return Err(ReuseRefusal::NetConflict {
                        net: name.to_string(),
                        first,
                        second,
                    });
                }
                (None, Some(net)) => {
                    map.insert(name, Some(net));
                }
                _ => {
                    map.entry(name).or_insert(None);
                }
            }
        }
    }
    Ok(map)
}

/// Move, turn and flip `counterpart` to where `source` lies in the block.
fn footprint_commands(
    source: &Footprint,
    counterpart: &Footprint,
    turn: &Turn,
    degrees: f64,
) -> Vec<BoardCommand> {
    let mut commands = Vec::new();
    let mut rotation = counterpart.rotation;
    if pcb_geom::Side::of_layer(&source.layer) != pcb_geom::Side::of_layer(&counterpart.layer) {
        commands.push(BoardCommand::Flip {
            footprint: counterpart.uuid,
        });
        rotation = (-rotation).rem_euclid(360.0);
    }
    let wanted = (source.rotation + degrees).rem_euclid(360.0);
    if wanted != rotation {
        commands.push(BoardCommand::Rotate {
            footprint: counterpart.uuid,
            degrees: wanted,
        });
    }
    let to = turn.apply(source.position);
    if to != counterpart.position {
        commands.push(BoardCommand::MoveFootprint {
            footprint: counterpart.uuid,
            to,
        });
    }
    commands
}

/// Block coordinates to board coordinates: `from` lands on `to`, turned by
/// the angle about it.
struct Turn {
    from: Point,
    to: Point,
    sin: f64,
    cos: f64,
}

impl Turn {
    fn new(from: Point, to: Point, degrees: f64) -> Self {
        // Right angles exactly, so a block on the grid stays on it.
        let turn = degrees.rem_euclid(360.0);
        let (sin, cos) = if turn == 0.0 {
            (0.0, 1.0)
        } else if turn == 90.0 {
            (1.0, 0.0)
        } else if turn == 180.0 {
            (0.0, -1.0)
        } else if turn == 270.0 {
            (-1.0, 0.0)
        } else {
            turn.to_radians().sin_cos()
        };
        Self { from, to, sin, cos }
    }

    fn apply(&self, p: Point) -> Point {
        let (dx, dy) = (p.x - self.from.x, p.y - self.from.y);
        Point::new(
            self.to.x + dx * self.cos - dy * self.sin,
            self.to.y + dx * self.sin + dy * self.cos,
        )
    }
}

#[cfg(test)]
#[path = "reuse_block_tests.rs"]
mod tests;
