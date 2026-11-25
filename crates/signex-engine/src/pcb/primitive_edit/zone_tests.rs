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

/// #2216: a pour takes a whole-number priority and its island rules; other
/// areas have none and are left as they are.
#[test]
fn a_pour_takes_a_priority_and_island_rules() {
    let mut b = with_fill(POUR_FILL);
    assert_eq!(edit(&mut b, ZoneEdit::Priority(3.0)), Ok(true));
    assert_eq!(edit(&mut b, ZoneEdit::MinIslandArea(2.5)), Ok(true));
    assert_eq!(edit(&mut b, ZoneEdit::RemoveDeadIslands(true)), Ok(true));
    let zone = &b.zones[0];
    assert_eq!(
        (
            zone.priority,
            zone.min_island_area,
            zone.remove_dead_islands
        ),
        (3, 2.5, true)
    );
    assert_eq!(
        edit(&mut b, ZoneEdit::Priority(1.5)),
        Err(PrimitiveEditRefusal::NotWhole)
    );
    assert_eq!(
        edit(&mut b, ZoneEdit::Priority(-1.0)),
        Err(PrimitiveEditRefusal::Negative)
    );
    assert_eq!(
        edit(&mut b, ZoneEdit::MinIslandArea(-0.5)),
        Err(PrimitiveEditRefusal::Negative)
    );
    let mut solid = with_fill("solid");
    assert_eq!(edit(&mut solid, ZoneEdit::Priority(2.0)), Ok(false));
    assert_eq!(
        edit(&mut solid, ZoneEdit::RemoveDeadIslands(true)),
        Ok(false)
    );
}

/// #2523: Shelve is one replace and its exact inverse undoes it; a fill,
/// region or keepout has nothing to shelve.
#[test]
fn a_pour_is_shelved_in_one_step_and_undone_exactly() {
    let b = with_fill(POUR_FILL);
    let command = zone_edit_command(&b, &[id(ZONE_1)], ZoneEdit::Shelved(true))
        .expect("allowed")
        .expect("a change");
    let mut engine = crate::pcb::BoardEngine::new(b.clone(), 1);
    engine.execute(command).expect("applies");
    assert!(engine.board().zones[0].is_shelved_pour());
    assert_eq!(
        zone_edit_command(engine.board(), &[id(ZONE_1)], ZoneEdit::Shelved(true)),
        Ok(None),
        "shelving a shelved pour changes nothing"
    );
    engine.undo().expect("undo").expect("a step");
    assert_eq!(
        engine.board().zones,
        b.zones,
        "the inverse restores the board"
    );

    let mut solid = with_fill("solid");
    assert_eq!(edit(&mut solid, ZoneEdit::Shelved(true)), Ok(false));
    let mut keepout = with_fill(KEEPOUT_FILL);
    assert_eq!(edit(&mut keepout, ZoneEdit::Shelved(true)), Ok(false));
}

/// #2506: Unfill is one replace and its exact inverse undoes it; the pour
/// keeps its outline and is not shelved; a fill, region or keepout has
/// nothing to unfill.
#[test]
fn a_pour_is_unfilled_in_one_step_and_undone_exactly() {
    let b = with_fill(POUR_FILL);
    let command = zone_edit_command(&b, &[id(ZONE_1)], ZoneEdit::Unfilled(true))
        .expect("allowed")
        .expect("a change");
    let mut engine = crate::pcb::BoardEngine::new(b.clone(), 1);
    engine.execute(command).expect("applies");
    let zone = &engine.board().zones[0];
    assert!(zone.is_unfilled_pour() && !zone.is_shelved_pour());
    assert_eq!(zone.outline, b.zones[0].outline, "the outline stays");
    assert_eq!(
        zone_edit_command(engine.board(), &[id(ZONE_1)], ZoneEdit::Unfilled(true)),
        Ok(None),
        "unfilling an unfilled pour changes nothing"
    );
    engine.undo().expect("undo").expect("a step");
    assert_eq!(
        engine.board().zones,
        b.zones,
        "the inverse restores the board"
    );

    let mut solid = with_fill("solid");
    assert_eq!(edit(&mut solid, ZoneEdit::Unfilled(true)), Ok(false));
    let mut keepout = with_fill(KEEPOUT_FILL);
    assert_eq!(edit(&mut keepout, ZoneEdit::Unfilled(true)), Ok(false));
    let mut shelved = with_fill(POUR_FILL);
    shelved.zones[0].shelved = true;
    assert_eq!(
        edit(&mut shelved, ZoneEdit::Unfilled(true)),
        Ok(false),
        "a shelved pour has nothing to unfill"
    );
}
