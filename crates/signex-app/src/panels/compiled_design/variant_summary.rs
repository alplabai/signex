//! Per-channel variant state read from the channel strip (#2662): what a
//! channel tab's Properties say about the active variant (mockup row 151:
//! the channels it leaves out, how the shown channel fits, the parts the
//! channels fit differently, "mixed · 1 of 3") and the copies of one part
//! the canvas menu's In variant rows set (mockup row 32). Read from the
//! strip's per-channel fits, the cards' own (`ChannelTabView::fits`), so
//! the Properties, the cards and the menu never disagree.

use signex_types::project::PartRef;
use uuid::Uuid;

use super::{ChannelPartFit, ChannelStripView, ChannelTabView};

/// The copies of one part on a repeated sheet, for the canvas menu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChannelCopies {
    /// The shown channel's name and its copy; `None` on Editor.
    pub shown: Option<(String, PartRef)>,
    /// Every channel's copy, in strip order; empty before a compile.
    pub all: Vec<PartRef>,
}

impl ChannelStripView {
    /// The copies of the part with the unit `unit` on the sheet.
    pub fn copies_of(&self, unit: Uuid) -> ChannelCopies {
        let copy_of = |tab: &ChannelTabView| {
            tab.fits
                .iter()
                .find(|fit| {
                    fit.part
                        .part
                        .as_ref()
                        .is_some_and(|link| link.units.contains(&unit))
                })
                .map(|fit| fit.part.clone())
        };
        let shown = self.active.and_then(|index| {
            let tab = self.tabs.iter().find(|tab| tab.channel.index == index)?;
            Some((tab.channel.name.clone(), copy_of(tab)?))
        });
        ChannelCopies {
            shown,
            all: self.tabs.iter().filter_map(copy_of).collect(),
        }
    }
}

/// The Variant section of a channel tab's Properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelVariantSummary {
    /// The channels the variant leaves out entirely, in strip order.
    pub left_out: Vec<String>,
    /// How many of the shown channel's parts the variant leaves out.
    pub not_fitted: usize,
    /// How many parts the shown channel has in the last compile.
    pub parts: usize,
    /// The shown channel's parts whose copies the channels fit differently.
    pub mixed: Vec<MixedPart>,
}

/// A part fitted in some channels and not in others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedPart {
    /// Its designator in the shown channel (`CH2_R5`).
    pub designator: String,
    /// How many channels leave their copy out.
    pub not_fitted: usize,
    /// How many channels have a copy.
    pub of: usize,
}

impl ChannelVariantSummary {
    /// Whether the last compile listed the channel's parts at all.
    pub fn is_compiled(&self) -> bool {
        self.parts > 0
    }
}

impl ChannelStripView {
    /// The Variant section of the shown channel; `None` on Editor.
    pub fn variant_summary(&self) -> Option<ChannelVariantSummary> {
        let index = self.active?;
        let shown = self.tabs.iter().find(|tab| tab.channel.index == index)?;
        let left_out = self
            .tabs
            .iter()
            .filter(|tab| tab.is_not_fitted())
            .map(|tab| tab.channel.name.clone())
            .collect();
        let mixed = shown
            .fits
            .iter()
            .filter_map(|fit| self.mixed_part(fit))
            .collect();
        Some(ChannelVariantSummary {
            left_out,
            not_fitted: shown.not_fitted(),
            parts: shown.fits.len(),
            mixed,
        })
    }

    /// `fit`'s part when the channels fit their copies of it differently.
    /// A copy is the same part placed in another channel: it shares a unit.
    fn mixed_part(&self, fit: &ChannelPartFit) -> Option<MixedPart> {
        let units = &fit.part.part.as_ref()?.units;
        let copies: Vec<bool> = self
            .tabs
            .iter()
            .filter_map(|tab| copy_in(tab, units))
            .collect();
        let not_fitted = copies.iter().filter(|fitted| !**fitted).count();
        (not_fitted > 0 && not_fitted < copies.len()).then(|| MixedPart {
            designator: fit.part.designator.clone(),
            not_fitted,
            of: copies.len(),
        })
    }
}

/// Whether `tab`'s copy of the part made of `units` is fitted; `None` when
/// the channel has no copy of it.
fn copy_in(tab: &ChannelTabView, units: &[uuid::Uuid]) -> Option<bool> {
    tab.fits
        .iter()
        .find(|fit| {
            fit.part
                .part
                .as_ref()
                .is_some_and(|link| link.units.iter().any(|unit| units.contains(unit)))
        })
        .map(|fit| fit.is_fitted)
}
