//! Differential pairs (#2389): which net routes with which, the coupled
//! width and gap they keep, and matching their lengths. A pair is found by
//! name (`USB_P` / `USB_N`, `CLK+` / `CLK-`), else by a net class with a
//! differential-pair gap that holds exactly two of the board's nets. The
//! pair is routed as a two-member bus (`super::bus`).

use pcb_geom::net_routed_length_mm;
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::super::command::BoardCommand;
use super::tune::{Meander, TuneRefusal, tune_nets};

/// The name endings that mark the two nets of a pair: positive, negative.
const SUFFIXES: [(&str, &str); 3] = [("_P", "_N"), ("_p", "_n"), ("+", "-")];

/// The two nets of a differential pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairNets {
    pub positive: u32,
    pub negative: u32,
}

impl PairNets {
    pub fn nets(self) -> [u32; 2] {
        [self.positive, self.negative]
    }
}

/// The pair `net` belongs to on `board`: by its name's ending first, else
/// by its net class, when that class sets a differential-pair gap and
/// holds exactly two of the board's nets (the one first by name is the
/// positive). `None` for a net in no pair.
pub fn diff_pair_of(board: &PcbBoard, classes: &[NetClass], net: u32) -> Option<PairNets> {
    let name = &board
        .nets
        .iter()
        .find(|n| n.number == net && net != 0)?
        .name;
    let number_of = |wanted: &str| {
        board
            .nets
            .iter()
            .find(|n| n.name == wanted && n.number != 0 && n.number != net)
            .map(|n| n.number)
    };
    for (positive, negative) in SUFFIXES {
        if let Some(stem) = name.strip_suffix(positive)
            && let Some(other) = number_of(&format!("{stem}{negative}"))
        {
            return Some(PairNets {
                positive: net,
                negative: other,
            });
        }
        if let Some(stem) = name.strip_suffix(negative)
            && let Some(other) = number_of(&format!("{stem}{positive}"))
        {
            return Some(PairNets {
                positive: other,
                negative: net,
            });
        }
    }
    let class = first_matching_class(classes, name).filter(|c| c.diff_pair_gap > 0.0)?;
    let mut members: Vec<(&str, u32)> = board
        .nets
        .iter()
        .filter(|n| n.number != 0)
        .filter(|n| first_matching_class(classes, &n.name).is_some_and(|c| c.name == class.name))
        .map(|n| (n.name.as_str(), n.number))
        .collect();
    members.sort_unstable();
    match members[..] {
        [(_, positive), (_, negative)] => Some(PairNets { positive, negative }),
        _ => None,
    }
}

/// The width and gap a pair keeps: its class's differential-pair values
/// where set (above zero), else `width_mm` and `gap_mm`; never a gap below
/// `min_gap_mm`, the clearance the rules ask between the two nets.
pub fn pair_width_gap(
    class: Option<&NetClass>,
    width_mm: f64,
    gap_mm: f64,
    min_gap_mm: f64,
) -> (f64, f64) {
    let set = |value: f64| (value.is_finite() && value > 0.0).then_some(value);
    let width = class
        .and_then(|c| set(c.diff_pair_width))
        .unwrap_or(width_mm);
    let gap = class.and_then(|c| set(c.diff_pair_gap)).unwrap_or(gap_mm);
    (width, gap.max(min_gap_mm))
}

/// What matching a pair's lengths did: the net that was lengthened and the
/// length it now has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairTuned {
    pub net: u32,
    pub length_mm: f64,
}

/// Lengthen the shorter net of `pair` to the longer one's routed length
/// with accordions (`super::tune_nets`), as one set of commands.
pub fn tune_pair(
    board: &PcbBoard,
    pair: PairNets,
    meander: &Meander,
    ids: impl FnMut() -> Uuid,
) -> Result<(Vec<BoardCommand>, PairTuned), TuneRefusal> {
    let length = |net| net_routed_length_mm(board, net).ok_or(TuneRefusal::Unrepresentable);
    let (positive, negative) = (length(pair.positive)?, length(pair.negative)?);
    let (short, target) = if positive < negative {
        (pair.positive, negative)
    } else {
        (pair.negative, positive)
    };
    let commands = tune_nets(board, &[short], target, meander, ids)?;
    Ok((
        commands,
        PairTuned {
            net: short,
            length_mm: target,
        },
    ))
}
