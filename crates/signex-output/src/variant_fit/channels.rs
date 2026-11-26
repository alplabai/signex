//! Per-channel variant state in the matrix (#2662): each channel's copy of a
//! part in a repeated sheet is a row of its own, named and set per channel;
//! the copies of one part are found together so the Variant Manager can show
//! one row for the part, read whether its channels agree in a variant, and
//! set every copy at once. See `docs/MULTI_CHANNEL.md`, "Per-channel variant
//! state".

use std::collections::HashMap;

use signex_types::pcb::SymbolLink;
use signex_types::schematic::InstancePath;

use super::matrix::{VariantMatrix, VariantRow};

/// The channel placements of a project as the matrix reads them: per
/// channel placement, the repeated placement its channels share and the
/// channel's name. Empty for a project without repeated sheets.
#[derive(Debug, Clone, Default)]
pub struct MatrixChannels {
    placements: HashMap<InstancePath, (InstancePath, String)>,
}

impl MatrixChannels {
    /// The channel placed at `channel`, a channel of the repeated placement
    /// `logical`, named `name` (`CH2`).
    pub fn insert(&mut self, channel: InstancePath, logical: InstancePath, name: String) {
        self.placements.insert(channel, (logical, name));
    }

    /// The repeated placement and the channel's name of `path`, when it is a
    /// channel placement.
    pub(super) fn of(&self, path: &InstancePath) -> Option<&(InstancePath, String)> {
        self.placements.get(path)
    }
}

/// What a row knows of its channel when it is one channel's copy of a part
/// in a repeated sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowChannel {
    /// The channel's name (`CH2`).
    pub name: String,
    /// The part every channel's copy shares: the repeated placement and the
    /// part's units.
    pub part: SymbolLink,
    /// The part's designator at the repeated placement (`R5`).
    pub designator: String,
}

/// How the channels' copies of one part fit in one column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelFit {
    /// Every copy is fitted.
    Fitted,
    /// No copy is fitted.
    NotFitted,
    /// The channels disagree: `not_fitted` of `of` copies are not fitted.
    Mixed { not_fitted: usize, of: usize },
}

impl VariantMatrix {
    /// The rows of every channel's copy of `part`, in row order.
    pub fn channel_copies(&self, part: &SymbolLink) -> Vec<&VariantRow> {
        self.rows
            .iter()
            .filter(|row| row.channel.as_ref().is_some_and(|ch| ch.part == *part))
            .collect()
    }

    /// How the channels' copies of `part` fit in `column`.
    pub fn channel_fit(&self, part: &SymbolLink, column: usize) -> ChannelFit {
        let copies = self.channel_copies(part);
        let not_fitted = copies
            .iter()
            .filter(|row| !row.fitted.get(column).copied().unwrap_or(true))
            .count();
        match not_fitted {
            0 => ChannelFit::Fitted,
            n if n == copies.len() => ChannelFit::NotFitted,
            n => ChannelFit::Mixed {
                not_fitted: n,
                of: copies.len(),
            },
        }
    }
}

/// The channel of the part `link` names, read from `channels`, with the
/// designator `designator_at` gives it at the repeated placement.
pub(super) fn row_channel(
    channels: &MatrixChannels,
    link: Option<&SymbolLink>,
    designator_at: impl FnOnce(&InstancePath) -> String,
) -> Option<RowChannel> {
    let link = link?;
    let (logical, name) = channels.of(&link.sheet)?;
    Some(RowChannel {
        name: name.clone(),
        part: SymbolLink::new(logical.clone(), link.units.iter().copied()),
        designator: designator_at(logical),
    })
}
