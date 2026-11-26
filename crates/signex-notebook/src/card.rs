//! The hover card of a reference (mockup row 196), opened in the margin
//! lane level with the reference. What it lists depends on the kind: a
//! part's value, footprint, sheet, nets, variants and uses; a pin's net,
//! connected pins and last measurement; a net's parts, sheets and last
//! measurements. A stale reference's card says why and offers its fixes.

use uuid::Uuid;

use crate::annotation::AnnotationKind;
use crate::badge::{Badge, BadgeTarget, BlockMention, badge_of};
use crate::design::{DesignIndex, PinName};
use crate::fix::FixOffer;
use crate::reference::Reference;
use crate::resolution::{ReferenceState, ResolvedReference};

/// An action a card, or the reference's right-click menu, offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardAction {
    /// Switch to the sheet and highlight the target.
    CrossProbe,
    /// Open the target's sheet.
    OpenSheet,
    /// The measurement timeline of the target (mockup row 199).
    Timeline,
    /// Highlight the net on every sheet.
    HighlightNet,
    /// Copy the reference as the notebook writes it.
    CopyReference,
    /// Go to the reference in the source.
    ShowInSource,
    /// Update every stale reference, listing the changes first.
    UpdateAllStale,
}

/// The facts a card lists, by kind.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CardFacts {
    Part {
        value: String,
        description: String,
        footprint: String,
        sheet: String,
        nets: Vec<String>,
        /// The variants the part is fitted in, and how many there are.
        fitted_in: Vec<String>,
        variant_count: usize,
    },
    Pin {
        /// The pin's name (`SW`), empty when it has none.
        pin_name: String,
        /// The part's description (`buck regulator`).
        part_description: String,
        net: Option<String>,
        /// The other pins on its net.
        connected: Vec<PinName>,
        sheet: String,
    },
    Net {
        pin_count: usize,
        parts: Vec<String>,
        sheets: Vec<String>,
    },
    /// The reference does not resolve.
    Stale {
        #[serde(flatten)]
        state: ReferenceState,
        /// The id the reference stored, so a renamed part reads as the
        /// same part.
        id: Option<Uuid>,
        /// The part's value when the stored id still finds it.
        value: Option<String>,
        fixes: Vec<FixOffer>,
    },
}

/// One reference's card.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReferenceCard {
    pub reference: Reference,
    pub facts: CardFacts,
    /// The blocks that name it, notebook order.
    pub blocks: Vec<BlockMention>,
    /// The newest dated measurement among `blocks` (for a pin or a net).
    pub last_measurement: Option<BlockMention>,
    /// How many times this file uses it.
    pub uses: usize,
    pub actions: Vec<CardAction>,
}

/// The card of `entry`, with the blocks naming it from the notebook's
/// `badges` (a part's card counts the blocks naming its pins too) and the
/// stale count of the file (for Update all).
pub fn reference_card(
    entry: &ResolvedReference,
    design: &DesignIndex,
    badges: &[Badge],
    stale_count: usize,
) -> ReferenceCard {
    let reference = &entry.reference;
    let named = badge_of(badges, &BadgeTarget::of(reference))
        .map(|badge| badge.blocks.clone())
        .unwrap_or_default();
    let last_measurement = match reference {
        Reference::Component(_) => None,
        _ => last_measurement(&named),
    };
    let facts = facts_of(entry, design);
    let actions = actions_of(reference, &facts, stale_count);
    ReferenceCard {
        reference: reference.clone(),
        facts,
        blocks: named,
        last_measurement,
        uses: entry.uses(),
        actions,
    }
}

fn facts_of(entry: &ResolvedReference, design: &DesignIndex) -> CardFacts {
    let stale = || {
        let value = entry
            .id
            .and_then(|id| design.parts_with_id(id).next())
            .map(|part| part.value.clone());
        CardFacts::Stale {
            state: entry.state.clone(),
            id: entry.id,
            value,
            fixes: entry.offers(),
        }
    };
    if entry.is_stale() {
        return stale();
    }
    match &entry.reference {
        Reference::Component(name) => match design.part(name) {
            Some(part) => CardFacts::Part {
                value: part.value.clone(),
                description: part.description.clone(),
                footprint: part.footprint.clone(),
                sheet: part.sheet.clone(),
                nets: design
                    .nets_of(part)
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                fitted_in: part.fitted_in.clone(),
                variant_count: part.variant_count,
            },
            None => stale(),
        },
        Reference::Pin { component, pin } => {
            let Some((part, found)) = design
                .part(component)
                .and_then(|part| part.pin(pin).map(|found| (part, found)))
            else {
                return stale();
            };
            let this = PinName {
                component: part.designator.clone(),
                pin: found.number.clone(),
            };
            let connected: Vec<PinName> = found
                .net
                .as_deref()
                .and_then(|net| design.net(net))
                .map(|net| net.pins.iter().filter(|p| **p != this).cloned().collect())
                .unwrap_or_default();
            CardFacts::Pin {
                pin_name: found.name.clone(),
                part_description: part.description.clone(),
                net: found.net.clone(),
                connected,
                sheet: part.sheet.clone(),
            }
        }
        Reference::Net(name) => match design.net(name) {
            Some(net) => CardFacts::Net {
                pin_count: net.pins.len(),
                parts: net.parts().into_iter().map(str::to_string).collect(),
                sheets: net.sheets.clone(),
            },
            None => stale(),
        },
    }
}

/// The card's buttons, as drawn: a part Cross-probe, Open sheet, Timeline;
/// a pin Cross-probe, Timeline, Copy reference; a net Cross-probe,
/// Highlight net, Timeline; a stale reference its fixes, Update all when
/// more than one is stale, and Show in source.
fn actions_of(reference: &Reference, facts: &CardFacts, stale_count: usize) -> Vec<CardAction> {
    use CardAction::*;
    if let CardFacts::Stale { state, .. } = facts {
        let renamed = matches!(state, ReferenceState::Renamed { .. });
        return if renamed && stale_count > 1 {
            vec![UpdateAllStale, ShowInSource]
        } else {
            vec![ShowInSource]
        };
    }
    match reference {
        Reference::Component(_) => vec![CrossProbe, OpenSheet, Timeline],
        Reference::Pin { .. } => vec![CrossProbe, Timeline, CopyReference],
        Reference::Net(_) => vec![CrossProbe, HighlightNet, Timeline],
    }
}

/// The right-click menu of a reference in the page, as drawn.
pub const CONTEXT_ACTIONS: [CardAction; 5] = [
    CardAction::CrossProbe,
    CardAction::OpenSheet,
    CardAction::Timeline,
    CardAction::ShowInSource,
    CardAction::CopyReference,
];

/// The newest dated measurement of `blocks`; undated ones only when none
/// is dated, the last of them.
pub fn last_measurement(blocks: &[BlockMention]) -> Option<BlockMention> {
    let mut measurements = blocks
        .iter()
        .filter(|b| b.kind == AnnotationKind::Measurement);
    measurements
        .clone()
        .filter(|b| b.date.is_some())
        .max_by_key(|b| b.date)
        .or_else(|| measurements.next_back())
        .cloned()
}
