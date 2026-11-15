//! #2393 — area fields: one replace per edit, a keepout keeps its kind of
//! layer and has no net, a pour's clearance is never negative.

use signex_types::pcb::{NetDef, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::zone_keepout::KEEPOUT_FILL;
use signex_types::zone_pour::POUR_FILL;

use super::*;
use crate::pcb::tests::{ZONE_1, board, id};

fn with_fill(fill: &str) -> PcbBoard {
    let mut b = board();
    b.zones[0].fill_type = fill.into();
    b.nets.push(NetDef {
        number: 4,
        name: "GND".into(),
    });
    b
}

fn edit(b: &mut PcbBoard, e: ZoneEdit) -> Result<bool, PrimitiveEditRefusal> {
    let command = zone_edit_command(b, &[id(ZONE_1)], e)?;
    let changed = command.is_some();
    if let Some(command) = command {
        command.apply(b).expect("applies");
    }
    Ok(changed)
}

#[test]
fn a_copper_area_takes_a_net_with_its_name_and_a_copper_layer() {
    let mut b = with_fill("solid");
    assert_eq!(edit(&mut b, ZoneEdit::Net(4)), Ok(true));
    assert_eq!((b.zones[0].net, b.zones[0].net_name.as_str()), (4, "GND"));
    assert_eq!(
        edit(&mut b, ZoneEdit::Layer(PcbLayer::Copper(CopperLayer::Top))),
        Ok(true)
    );
    assert_eq!(b.zones[0].layer, "TopCopper");
    let keepout_layer = PcbLayer::Technical(TechnicalLayer::KeepoutTop);
    assert_eq!(
        edit(&mut b, ZoneEdit::Layer(keepout_layer)),
        Err(PrimitiveEditRefusal::LayerNotAllowed)
    );
    assert_eq!(
        edit(&mut b, ZoneEdit::Clearance(0.3)),
        Ok(false),
        "only a pour has a clearance"
    );
}

#[test]
fn a_keepout_has_no_net_and_may_sit_on_a_keep_out_layer() {
    let mut b = with_fill(KEEPOUT_FILL);
    assert_eq!(edit(&mut b, ZoneEdit::Net(4)), Ok(false));
    let keepout_layer = PcbLayer::Technical(TechnicalLayer::KeepoutTop);
    assert_eq!(edit(&mut b, ZoneEdit::Layer(keepout_layer)), Ok(true));
}

#[test]
fn a_pour_takes_a_clearance_and_minimum_width_never_below_zero() {
    let mut b = with_fill(POUR_FILL);
    assert_eq!(edit(&mut b, ZoneEdit::Clearance(0.3)), Ok(true));
    assert_eq!(edit(&mut b, ZoneEdit::MinThickness(0.2)), Ok(true));
    assert_eq!((b.zones[0].clearance, b.zones[0].min_thickness), (0.3, 0.2));
    assert_eq!(
        edit(&mut b, ZoneEdit::Clearance(-1.0)),
        Err(PrimitiveEditRefusal::Negative)
    );
    assert_eq!(
        edit(&mut b, ZoneEdit::MinThickness(f64::NAN)),
        Err(PrimitiveEditRefusal::NonFinite)
    );
}
