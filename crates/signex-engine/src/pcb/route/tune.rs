//! Length tuning (#2389): a straight track of a net becomes an accordion
//! (`pcb_geom::route::accordion`) that adds what the net lacks of a target
//! length, on whichever side keeps the clearance from other nets' copper.
//! The track is replaced in one undo step.

use pcb_geom::route::{ClearanceRule, NewCopper, RouteWorld, accordion, accordion_room};
use pcb_geom::{PointNm, layer_mask, mm_to_nm, net_routed_length_mm};
use signex_types::pcb::{PcbBoard, Segment};
use uuid::Uuid;

use super::super::command::BoardCommand;
use super::super::items::{BoardItem, BoardItemId};

/// The accordion's shape and the rule it keeps.
pub struct Meander<'a> {
    pub clearance: &'a ClearanceRule<'a>,
    /// The widest clearance the rules can ask of any pair, in nm.
    pub reach: i64,
    /// How far apart the accordion's legs run; a bump is never lower than
    /// half of it, so it never folds onto its own copper.
    pub pitch_mm: f64,
    /// The tallest a bump may be.
    pub amplitude_mm: f64,
}

/// Why a track was not tuned.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TuneRefusal {
    #[error("the track is not on the board")]
    Gone,
    #[error("the track is on no net")]
    NoNet,
    #[error("the net is {current_mm:.3} mm long already")]
    LongEnough { current_mm: f64 },
    #[error("the track is too short for the meanders this needs")]
    TooShort,
    #[error("the net is short by less than one meander adds ({least_mm:.3} mm)")]
    TooLittle { least_mm: f64 },
    #[error("the meanders would not keep the clearance the rules ask")]
    Blocked,
    #[error("a length here is not one the board geometry can hold")]
    Unrepresentable,
}

/// Lengthen `track`'s net to `target_mm`: the track replaced by an
/// accordion of the fewest bumps no taller than the amplitude, left of the
/// track if that side is clear, else right. The net ends at the target or
/// at most a few nm over. `ids` names each new track.
pub fn tune_length(
    board: &PcbBoard,
    track: Uuid,
    target_mm: f64,
    meander: &Meander,
    mut ids: impl FnMut() -> Uuid,
) -> Result<Vec<BoardCommand>, TuneRefusal> {
    use TuneRefusal::Unrepresentable;
    let segment = board
        .segments
        .iter()
        .find(|s| s.uuid == track)
        .ok_or(TuneRefusal::Gone)?;
    if segment.net == 0 {
        return Err(TuneRefusal::NoNet);
    }
    let current_mm = net_routed_length_mm(board, segment.net).ok_or(Unrepresentable)?;
    let nm = |mm: f64| mm_to_nm(mm, "length").map_err(|_| Unrepresentable);
    let extra = nm(target_mm)? - nm(current_mm)?;
    if extra <= 0 {
        return Err(TuneRefusal::LongEnough { current_mm });
    }
    let (pitch, amplitude) = (nm(meander.pitch_mm)?, nm(meander.amplitude_mm)?);
    if pitch <= 0 || amplitude <= 0 {
        return Err(Unrepresentable);
    }
    let a = PointNm::from_mm(segment.start, "track start").map_err(|_| Unrepresentable)?;
    let b = PointNm::from_mm(segment.end, "track end").map_err(|_| Unrepresentable)?;
    let length = (a.distance_sq(b) as f64).sqrt().floor() as i64;
    // A bump lower than half a pitch lays its copper over itself: it adds
    // centre-line length but no real length.
    let lowest = (pitch + 1) / 2;
    if extra < 2 * lowest {
        return Err(TuneRefusal::TooLittle {
            least_mm: pcb_geom::nm_to_mm(2 * lowest),
        });
    }
    let bumps = (extra + 2 * amplitude - 1) / (2 * amplitude);
    let bumps = bumps.min(extra / (2 * lowest)).max(1);
    let bumps = usize::try_from(bumps).map_err(|_| Unrepresentable)?;
    if bumps > accordion_room(length, pitch) {
        return Err(TuneRefusal::TooShort);
    }
    let twice = 2 * bumps as i64;
    let height = (extra + twice - 1) / twice;
    let world = RouteWorld::with_reach(board, meander.clearance, meander.reach);
    let radius = (nm(segment.width)? / 2).abs();
    let layers = layer_mask(std::iter::once(segment.layer.as_str()));
    let is_clear = |corners: &[PointNm]| {
        corners.windows(2).all(|leg| {
            world.is_clear(&NewCopper::track(
                leg[0],
                leg[1],
                radius,
                segment.net,
                layers,
            ))
        })
    };
    let corners = [true, false]
        .into_iter()
        .filter_map(|left| accordion(a, b, bumps, height, pitch, left))
        .find(|corners| is_clear(corners.as_slice()))
        .ok_or(TuneRefusal::Blocked)?;
    let last = corners.len() - 1;
    let point = |i: usize, p: PointNm| match i {
        0 => segment.start,
        i if i == last => segment.end,
        _ => p.to_mm(),
    };
    let tracks: Vec<BoardItem> = corners
        .windows(2)
        .enumerate()
        .map(|(i, leg)| {
            BoardItem::Segment(Segment {
                uuid: ids(),
                start: point(i, leg[0]),
                end: point(i + 1, leg[1]),
                ..segment.clone()
            })
        })
        .collect();
    Ok(vec![
        BoardCommand::DeleteItems {
            items: vec![BoardItemId::Segment(track)],
        },
        BoardCommand::AddItems { items: tracks },
    ])
}
