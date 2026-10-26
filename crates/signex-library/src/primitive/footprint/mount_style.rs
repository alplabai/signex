//! Mount style derived from a footprint's pads.

use serde::{Deserialize, Serialize};

use super::{Footprint, PadKind};

/// How a footprint attaches to the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountStyle {
    Smd,
    ThroughHole,
    Mixed,
    Unknown,
}

impl Footprint {
    /// Derived from `Smd` and `Tht` pads only; every other pad kind
    /// (mounting hole, connector, castellated, fiducial) is ignored.
    pub fn mount_style(&self) -> MountStyle {
        let (mut smd, mut tht) = (false, false);
        for pad in &self.pads {
            match pad.kind {
                PadKind::Smd => smd = true,
                PadKind::Tht => tht = true,
                _ => {}
            }
        }
        match (smd, tht) {
            (true, false) => MountStyle::Smd,
            (false, true) => MountStyle::ThroughHole,
            (true, true) => MountStyle::Mixed,
            (false, false) => MountStyle::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::footprint::Pad;

    fn style(kinds: &[PadKind]) -> MountStyle {
        let mut fp = Footprint::empty("t");
        fp.pads = kinds
            .iter()
            .map(|&kind| Pad {
                kind,
                ..Pad::default()
            })
            .collect();
        fp.mount_style()
    }

    #[test]
    fn derives_from_pad_kinds() {
        use PadKind::*;
        assert_eq!(style(&[]), MountStyle::Unknown);
        assert_eq!(style(&[Smd, Smd]), MountStyle::Smd);
        assert_eq!(style(&[Tht]), MountStyle::ThroughHole);
        assert_eq!(style(&[Smd, Tht]), MountStyle::Mixed);
        assert_eq!(style(&[Smd, NptHole, Fiducial]), MountStyle::Smd);
        assert_eq!(style(&[NptHole, Fiducial]), MountStyle::Unknown);
        assert_eq!(style(&[Castellated, ConnectorPad]), MountStyle::Unknown);
    }

    #[test]
    fn serde_is_snake_case() {
        let s = serde_json::to_string(&MountStyle::ThroughHole).unwrap();
        assert_eq!(s, "\"through_hole\"");
    }
}
