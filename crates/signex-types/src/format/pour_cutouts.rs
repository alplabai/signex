//! The optional `[[pour_cutouts]]` array of a `.snxpcb` (#2871): the
//! board's pour cutouts (`zone_cutout`), each a zone table as in
//! `[[zones]]`.
//!
//! Kept apart from `[[zones]]` so a build that does not know cutouts keeps
//! the array as a foreign key and re-emits it, instead of reading a cutout
//! as a copper zone. Written only when the board has a cutout, so a board
//! without one re-saves byte-identical. On load each entry is a cutout
//! whatever its `fill_type` says, appended after the zones.

use serde::Serialize;

use super::{FormatError, finite_toml};
use crate::pcb::{PcbBoard, Zone};
use crate::zone_cutout::POUR_CUTOUT_FILL;

/// The cutouts of the file; none when it has no `[[pour_cutouts]]`.
pub(in crate::format) fn read(raw: Option<Vec<Zone>>) -> impl Iterator<Item = Zone> {
    raw.unwrap_or_default().into_iter().map(|zone| Zone {
        fill_type: POUR_CUTOUT_FILL.into(),
        ..zone
    })
}

/// The zones `[[zones]]` holds: every zone but the cutouts.
pub(in crate::format) fn zones_of(board: &PcbBoard) -> Vec<&Zone> {
    board.zones.iter().filter(|z| !z.is_pour_cutout()).collect()
}

/// Append `[[pour_cutouts]]`; nothing for a board without a cutout.
pub(in crate::format) fn write(out: &mut String, board: &PcbBoard) -> Result<(), FormatError> {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        pour_cutouts: Vec<&'a Zone>,
    }
    let pour_cutouts: Vec<&Zone> = board.zones.iter().filter(|z| z.is_pour_cutout()).collect();
    if pour_cutouts.is_empty() {
        return Ok(());
    }
    let wrapper = Wrapper { pour_cutouts };
    finite_toml::ensure_finite(&wrapper)?;
    out.push('\n');
    out.push_str(&toml::to_string_pretty(&wrapper)?);
    Ok(())
}
