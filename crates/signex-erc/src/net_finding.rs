//! The net a net-level finding is about.
//!
//! A DSL rule that targets a net has no point of its own. Rather than pin it
//! to a fake origin, the finding carries the net: its name and every pin on
//! it in a deterministic order. The first pin is the *representative* one the
//! canvas anchors the marker at; the rest let the app highlight and fit the
//! whole net.

use signex_types::schematic::Point;
use uuid::Uuid;

use crate::context::ErcNet;

/// One pin of a net-level finding's net.
#[derive(Debug, Clone, PartialEq)]
pub struct NetFindingPin {
    pub symbol: Uuid,
    pub reference: String,
    pub number: String,
    pub position: Point,
    /// Electrical type of the pin.
    pub direction: signex_types::schematic::PinDirection,
}

/// The net a finding is about.
#[derive(Debug, Clone, PartialEq)]
pub struct NetFinding {
    /// Winning net name (empty for an unnamed net).
    pub name: String,
    /// Every pin on the net, sorted by designator, pin number, then position.
    pub pins: Vec<NetFindingPin>,
}

impl NetFinding {
    pub fn from_net(net: &ErcNet) -> Self {
        let pins = net
            .pins
            .iter()
            .map(|p| NetFindingPin {
                symbol: p.symbol,
                reference: p.reference.clone(),
                number: p.number.clone(),
                position: p.position,
                direction: p.direction,
            })
            .collect();
        Self::from_pins(net.name.clone(), pins)
    }

    /// A net named `name` holding `pins`, put in the stable order.
    pub fn from_pins(name: String, mut pins: Vec<NetFindingPin>) -> Self {
        pins.sort_by(|a, b| {
            a.reference
                .cmp(&b.reference)
                .then_with(|| a.number.cmp(&b.number))
                .then_with(|| a.symbol.cmp(&b.symbol))
                .then_with(|| a.position.x.total_cmp(&b.position.x))
                .then_with(|| a.position.y.total_cmp(&b.position.y))
        });
        Self { name, pins }
    }

    /// The stable pin the marker is anchored at: lowest designator, then pin.
    pub fn representative(&self) -> Option<&NetFindingPin> {
        self.pins.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ErcNetPin;
    use crate::fixtures::net_of;
    use signex_types::schematic::{InstancePath, PinDirection};

    fn pin(reference: &str, number: &str, x: f64) -> ErcNetPin {
        ErcNetPin {
            symbol: Uuid::now_v7(),
            occurrence: InstancePath::root(),
            reference: reference.into(),
            number: number.into(),
            position: Point::new(x, 0.0),
            local_pos: Point::new(x, 0.0),
            direction: PinDirection::Passive,
            is_power_flag: false,
        }
    }

    #[test]
    fn representative_is_lowest_designator_then_pin_regardless_of_input_order() {
        let mut net = net_of(&[]);
        net.pins = vec![
            pin("U2", "1", 1.0),
            pin("R9", "2", 2.0),
            pin("R9", "1", 3.0),
        ];
        let finding = NetFinding::from_net(&net);
        let rep = finding.representative().expect("net has pins");
        assert_eq!((rep.reference.as_str(), rep.number.as_str()), ("R9", "1"));
        net.pins.reverse();
        assert_eq!(NetFinding::from_net(&net), finding);
    }
}
