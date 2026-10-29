//! The hover insight: a short dwell over a wire, pin or label, then a card
//! naming the net, its class and the pins on it with their sheets.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use signex_net::{NetHover, SheetKey};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

/// How long the cursor rests on an object before the card appears.
pub const DWELL: Duration = Duration::from_millis(450);

/// The most pins a card lists before it says how many more there are.
pub const MAX_CARD_PINS: usize = 12;

/// How far a cursor may drift over a placed symbol before it counts as a new
/// target (a symbol's pins are found by where the cursor is).
const SYMBOL_DRIFT_MM: f64 = 0.5;

/// What the cursor rests on, and since when.
#[derive(Debug, Clone)]
pub struct HoverCandidate {
    pub sheet: PathBuf,
    pub item: SelectedItem,
    pub world: Point,
    pub screen: (f32, f32),
    pub since: Instant,
    /// The dwell was answered (a card shown, or nothing to show): stop asking.
    pub settled: bool,
    /// The dwell is up and the answer waits for the index: no more ticks.
    pub waiting: bool,
}

impl HoverCandidate {
    pub fn is_due(&self, now: Instant) -> bool {
        !self.settled && now.saturating_duration_since(self.since) >= DWELL
    }

    fn same_target(&self, sheet: &PathBuf, item: &SelectedItem, world: Point) -> bool {
        if &self.sheet != sheet || self.item != *item {
            return false;
        }
        if item.kind != SelectedKind::Symbol {
            return true;
        }
        (self.world.x - world.x).hypot(self.world.y - world.y) <= SYMBOL_DRIFT_MM
    }
}

/// One pin line of the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverCardPin {
    /// `R1.2`: designator and pin.
    pub pin: String,
    pub sheet: String,
}

/// The card as drawn: formatted facts only, so `view` does no lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverCard {
    pub name: String,
    pub class: Option<String>,
    pub pins: Vec<HoverCardPin>,
    /// Pins beyond [`MAX_CARD_PINS`].
    pub hidden: usize,
    pub screen: (i32, i32),
}

impl HoverCard {
    pub fn from_hover(
        hover: &NetHover,
        sheet_label: impl Fn(&SheetKey) -> String,
        screen: (f32, f32),
    ) -> Self {
        let pins = hover
            .pins
            .iter()
            .take(MAX_CARD_PINS)
            .map(|p| HoverCardPin {
                pin: format!("{}.{}", p.reference, p.pin),
                sheet: sheet_label(&p.sheet),
            })
            .collect();
        Self {
            name: hover.name.clone(),
            class: hover.class.clone(),
            pins,
            hidden: hover.pins.len().saturating_sub(MAX_CARD_PINS),
            screen: (screen.0 as i32, screen.1 as i32),
        }
    }
}

#[derive(Debug, Default)]
pub struct HoverState {
    pub candidate: Option<HoverCandidate>,
    pub card: Option<HoverCard>,
}

impl HoverState {
    /// Note what the cursor is over now. A new target restarts the dwell and
    /// drops the card; leaving every target, or the preference being off,
    /// clears both. Returns `true` when a card was dropped.
    pub fn observe(
        &mut self,
        target: Option<(PathBuf, SelectedItem, Point)>,
        screen: (f32, f32),
        enabled: bool,
        now: Instant,
    ) -> bool {
        let had_card = self.card.is_some();
        let Some((sheet, item, world)) = target.filter(|_| enabled) else {
            self.candidate = None;
            self.card = None;
            return had_card;
        };
        if self
            .candidate
            .as_ref()
            .is_some_and(|c| c.same_target(&sheet, &item, world))
        {
            return false;
        }
        self.candidate = Some(HoverCandidate {
            sheet,
            item,
            world,
            screen,
            since: now,
            settled: false,
            waiting: false,
        });
        self.card = None;
        had_card
    }

    /// The dwell timer has work only while a candidate is unanswered.
    pub fn needs_ticks(&self) -> bool {
        self.candidate
            .as_ref()
            .is_some_and(|c| !c.settled && !c.waiting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_net::HoverPin;
    use uuid::Uuid;

    fn item(n: u128, kind: SelectedKind) -> SelectedItem {
        SelectedItem::new(Uuid::from_u128(n), kind)
    }

    fn target(n: u128, kind: SelectedKind, x: f64) -> Option<(PathBuf, SelectedItem, Point)> {
        Some((PathBuf::from("a.snxsch"), item(n, kind), Point::new(x, 0.0)))
    }

    #[test]
    fn a_new_target_starts_a_dwell_and_is_due_only_after_it() {
        let t0 = Instant::now();
        let mut s = HoverState::default();
        s.observe(target(1, SelectedKind::Wire, 0.0), (1.0, 2.0), true, t0);
        let c = s.candidate.as_ref().expect("candidate");
        assert!(!c.is_due(t0 + DWELL / 2));
        assert!(c.is_due(t0 + DWELL));
        assert!(s.needs_ticks());
    }

    #[test]
    fn staying_on_the_same_wire_keeps_the_dwell_running() {
        let t0 = Instant::now();
        let mut s = HoverState::default();
        s.observe(target(1, SelectedKind::Wire, 0.0), (0.0, 0.0), true, t0);
        s.observe(
            target(1, SelectedKind::Wire, 3.0),
            (0.0, 0.0),
            true,
            t0 + DWELL,
        );
        assert_eq!(s.candidate.as_ref().map(|c| c.since), Some(t0));
    }

    #[test]
    fn moving_to_another_object_restarts_the_dwell_and_drops_the_card() {
        let t0 = Instant::now();
        let mut s = HoverState::default();
        s.observe(target(1, SelectedKind::Wire, 0.0), (0.0, 0.0), true, t0);
        s.card = Some(HoverCard::from_hover(
            &NetHover {
                name: "VCC".into(),
                class: None,
                pins: Vec::new(),
            },
            |k| k.to_string(),
            (0.0, 0.0),
        ));
        let dropped = s.observe(
            target(2, SelectedKind::Wire, 0.0),
            (0.0, 0.0),
            true,
            t0 + DWELL,
        );
        assert!(dropped && s.card.is_none());
        assert_eq!(s.candidate.as_ref().map(|c| c.since), Some(t0 + DWELL));
    }

    #[test]
    fn drifting_along_a_symbol_is_a_new_target_so_another_pin_can_be_found() {
        let t0 = Instant::now();
        let mut s = HoverState::default();
        s.observe(target(1, SelectedKind::Symbol, 0.0), (0.0, 0.0), true, t0);
        s.observe(
            target(1, SelectedKind::Symbol, 0.2),
            (0.0, 0.0),
            true,
            t0 + DWELL,
        );
        assert_eq!(s.candidate.as_ref().map(|c| c.since), Some(t0));
        s.observe(
            target(1, SelectedKind::Symbol, 5.0),
            (0.0, 0.0),
            true,
            t0 + DWELL,
        );
        assert_eq!(s.candidate.as_ref().map(|c| c.since), Some(t0 + DWELL));
    }

    #[test]
    fn leaving_every_object_or_turning_the_preference_off_clears_everything() {
        let t0 = Instant::now();
        let mut s = HoverState::default();
        s.observe(target(1, SelectedKind::Label, 0.0), (0.0, 0.0), true, t0);
        s.observe(None, (0.0, 0.0), true, t0);
        assert!(s.candidate.is_none());
        s.observe(target(1, SelectedKind::Label, 0.0), (0.0, 0.0), false, t0);
        assert!(
            s.candidate.is_none(),
            "preference off: no candidate, no ticks"
        );
        assert!(!s.needs_ticks());
    }

    #[test]
    fn a_settled_candidate_stops_the_timer() {
        let mut s = HoverState::default();
        s.observe(
            target(1, SelectedKind::Wire, 0.0),
            (0.0, 0.0),
            true,
            Instant::now(),
        );
        s.candidate.as_mut().expect("candidate").settled = true;
        assert!(!s.needs_ticks());
    }

    #[test]
    fn the_card_formats_pins_with_their_sheets_and_counts_the_overflow() {
        let pins = (0..15)
            .map(|i| HoverPin {
                reference: format!("R{i}"),
                pin: "2".into(),
                sheet: SheetKey::new("pwr.snxsch"),
            })
            .collect();
        let hover = NetHover {
            name: "VCC".into(),
            class: Some("Power".into()),
            pins,
        };
        let card = HoverCard::from_hover(
            &hover,
            |k| k.as_str().trim_end_matches(".snxsch").into(),
            (10.4, 20.9),
        );
        assert_eq!(card.name, "VCC");
        assert_eq!(card.class.as_deref(), Some("Power"));
        assert_eq!(card.pins.len(), MAX_CARD_PINS);
        assert_eq!(
            card.pins[0],
            HoverCardPin {
                pin: "R0.2".into(),
                sheet: "pwr".into()
            }
        );
        assert_eq!(card.hidden, 3);
        assert_eq!(card.screen, (10, 20));
    }
}
