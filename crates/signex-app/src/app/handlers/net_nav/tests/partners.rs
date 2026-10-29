//! Go to net partner (#1272).

use signex_types::schematic::SelectedKind;

use super::fixture::*;
use crate::app::NetNavMsg;

#[test]
fn a_global_label_jumps_to_its_partner_on_another_sheet() {
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);

    p.send_settled(NetNavMsg::PartnerJumpRequested);

    assert_eq!(p.active(), Some(p.mid.clone()));
    let picked: Vec<_> = p.selected().iter().map(|i| (i.uuid, i.kind)).collect();
    assert_eq!(picked, [(id(MID_VCC), SelectedKind::Label)]);
}

#[test]
fn repeating_the_jump_cycles_through_every_partner_and_back() {
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);
    let mut visited = Vec::new();
    for _ in 0..3 {
        p.send_settled(NetNavMsg::PartnerJumpRequested);
        visited.push(p.active().expect("sheet"));
    }
    assert_eq!(visited, [p.mid.clone(), p.third.clone(), p.root.clone()]);
}

#[test]
fn a_hierarchical_label_jumps_up_to_its_sheet_entry_and_back_down() {
    let mut p = project();
    p.focus(&p.mid.clone());
    p.select(MID_SIG, SelectedKind::Label);

    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(p.active(), Some(p.root.clone()));
    let picked: Vec<_> = p.selected().iter().map(|i| (i.uuid, i.kind)).collect();
    assert_eq!(picked, [(id(SIG_ENTRY), SelectedKind::SheetPin)]);

    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(p.active(), Some(p.mid.clone()));
    let picked: Vec<_> = p.selected().iter().map(|i| (i.uuid, i.kind)).collect();
    assert_eq!(picked, [(id(MID_SIG), SelectedKind::Label)]);
}

#[test]
fn a_port_with_no_partner_stays_put_and_says_what_to_do() {
    let mut p = project();
    p.focus(&p.mid.clone());
    p.select(ALONE, SelectedKind::Label);

    p.send_settled(NetNavMsg::PartnerJumpRequested);

    assert_eq!(p.active(), Some(p.mid.clone()));
    assert!(logged(&["has no partner", "ALONE"]));
}

#[test]
fn a_wire_is_not_a_port_and_the_jump_says_so() {
    let mut p = project();
    p.select(ROOT_VCC_WIRE, SelectedKind::Wire);

    p.send_settled(NetNavMsg::PartnerJumpRequested);

    assert_eq!(p.active(), Some(p.root.clone()));
    assert!(logged(&["Select a label, sheet entry or power port"]));
}
