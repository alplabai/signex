//! #1279: a repeated sheet compiles to one occurrence per channel, with
//! channel designators, channel-local nets and Repeat entries joined to the
//! parent bus member by member.
use signex_types::project::ChannelNaming;
use signex_types::schematic::ChannelRepeat;

use super::{
    StitchIssue, build_project_netlist, build_project_netlist_indexed, build_project_netlist_named,
    channel_uuid, project_occurrences,
};
use crate::fixtures::multi_channel::{BUCK, BUCK_PARTS, BUCK_SYMBOL, TOP, project};
use signex_types::net::Netlist;

/// The references on the net named `name`, sorted.
fn refs_on(nl: &Netlist, name: &str) -> Vec<String> {
    let net = nl
        .nets
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no net named {name}; nets: {:?}", names(nl)));
    let mut refs: Vec<String> = net.terminals.iter().map(|t| t.reference.clone()).collect();
    refs.sort();
    refs
}

fn names(nl: &Netlist) -> Vec<&str> {
    nl.nets.iter().map(|n| n.name.as_str()).collect()
}

#[test]
fn a_repeated_placement_is_walked_once_per_channel() {
    let p = project();
    let occs = project_occurrences(&p.graph());
    let buck: Vec<_> = occs.iter().filter(|o| o.key.as_str() == BUCK).collect();
    assert_eq!(buck.len(), 4);
    let chains: Vec<&str> = buck.iter().map(|o| o.name_chain[0].as_str()).collect();
    assert_eq!(chains, ["CH1", "CH2", "CH3", "CH4"]);
    for (i, o) in buck.iter().enumerate() {
        let want = channel_uuid(BUCK_SYMBOL, i as u32 + 1);
        assert_eq!(o.instance_path.chain(), [want], "channel {}", i + 1);
    }
}

#[test]
fn channel_uuids_are_stable_and_distinct() {
    let a = channel_uuid(BUCK_SYMBOL, 1);
    assert_eq!(a, channel_uuid(BUCK_SYMBOL, 1));
    assert_ne!(a, channel_uuid(BUCK_SYMBOL, 2));
    assert_ne!(a, BUCK_SYMBOL);
    assert_eq!(a.get_version_num(), 8);
}

#[test]
fn every_channel_part_takes_its_channel_designator() {
    let p = project();
    let nl = build_project_netlist(&p.graph()).netlist;
    for ch in 1..=4 {
        let want = format!("CH{ch}_R5");
        assert!(
            nl.nets
                .iter()
                .any(|n| n.terminals.iter().any(|t| t.reference == want)),
            "{want} is on a net"
        );
    }
    assert!(
        !nl.nets
            .iter()
            .any(|n| n.terminals.iter().any(|t| t.reference == "R5")),
        "no bare R5 is left"
    );
}

#[test]
fn a_repeat_entry_joins_each_channel_to_its_bus_member() {
    let p = project();
    let nl = build_project_netlist(&p.graph()).netlist;
    for ch in 1..=4 {
        assert_eq!(
            refs_on(&nl, &format!("VOUT_3V3_CH{ch}")),
            [
                format!("CH{ch}_C4"),
                format!("CH{ch}_L1"),
                format!("R1{ch}")
            ],
        );
    }
}

#[test]
fn plain_entries_are_shared_by_every_channel() {
    let p = project();
    let nl = build_project_netlist(&p.graph()).netlist;
    let mut vin: Vec<String> = (1..=4)
        .flat_map(|ch| [format!("CH{ch}_C5"), format!("CH{ch}_U1")])
        .collect();
    vin.push("J1".into());
    vin.sort();
    assert_eq!(refs_on(&nl, "VIN_12V"), vin);
    let gnd = refs_on(&nl, "GND");
    for ch in 1..=4 {
        assert!(gnd.contains(&format!("CH{ch}_C3")), "{gnd:?}");
        assert!(gnd.contains(&format!("CH{ch}_C6")), "{gnd:?}");
    }
    assert!(gnd.contains(&"J2".to_string()));
    let en = refs_on(&nl, "EN");
    assert_eq!(en.len(), 5, "R1 and one R4 per channel: {en:?}");
}

#[test]
fn a_channel_local_net_takes_the_channel_net_format() {
    let p = project();
    let nl = build_project_netlist(&p.graph()).netlist;
    for ch in 1..=4 {
        assert_eq!(
            refs_on(&nl, &format!("FB_CH{ch}")),
            [format!("CH{ch}_D1"), format!("CH{ch}_R5")],
        );
    }
    assert!(
        !names(&nl).iter().any(|n| n.contains("Repeat(")),
        "{:?}",
        names(&nl)
    );
}

#[test]
fn channels_raise_no_shared_reference_issue() {
    let p = project();
    let issues = build_project_netlist(&p.graph()).issues;
    assert!(
        !issues
            .iter()
            .any(|i| matches!(i, StitchIssue::SharedReferenceAcrossInstances { .. })),
        "{issues:?}"
    );
}

#[test]
fn the_component_report_lists_every_channel_part() {
    let p = project();
    let (_, index) = build_project_netlist_indexed(&p.graph());
    let parts = index.components();
    let channel: Vec<&str> = parts
        .iter()
        .map(|c| c.designator.as_str())
        .filter(|d| d.starts_with("CH"))
        .collect();
    assert_eq!(channel.len(), 4 * BUCK_PARTS.len());
    for ch in 1..=4 {
        for part in BUCK_PARTS {
            let want = format!("CH{ch}_{part}");
            assert!(channel.contains(&want.as_str()), "{want} missing");
        }
    }
}

#[test]
fn the_project_naming_is_applied() {
    let p = project();
    let naming = ChannelNaming {
        designator_format: "${Designator}${ChannelAlpha}".into(),
        net_format: "${Channel}_${Net}".into(),
    };
    let nl = build_project_netlist_named(&p.graph(), &naming).netlist;
    assert_eq!(refs_on(&nl, "CH2_FB"), ["D1B", "R5B"]);
    assert_eq!(refs_on(&nl, "CH4_VOUT_3V3"), ["C4D", "L1D", "R14"]);
}

#[test]
fn a_plain_designator_is_one_occurrence() {
    let mut p = project();
    let top = p.sheet_mut(TOP);
    top.child_sheets[1].name = "buck".into();
    let occs = project_occurrences(&p.graph());
    assert_eq!(occs.iter().filter(|o| o.key.as_str() == BUCK).count(), 1);
    let nl = build_project_netlist(&p.graph()).netlist;
    assert!(refs_on(&nl, "VIN_12V").contains(&"U1".to_string()));
}

#[test]
fn a_repeat_entry_off_a_bus_joins_nothing() {
    let mut p = project();
    p.sheet_mut(TOP).buses.clear();
    let nl = build_project_netlist(&p.graph()).netlist;
    for ch in 1..=4 {
        assert_eq!(
            refs_on(&nl, &format!("VOUT_3V3_CH{ch}")),
            [format!("CH{ch}_C4"), format!("CH{ch}_L1")],
        );
    }
}

#[test]
fn the_repeat_count_follows_the_designator() {
    let mut p = project();
    let six = ChannelRepeat {
        channel: "CH".into(),
        from: 1,
        to: 6,
    };
    p.sheet_mut(TOP).child_sheets[1].name = six.to_designator();
    let occs = project_occurrences(&p.graph());
    assert_eq!(occs.iter().filter(|o| o.key.as_str() == BUCK).count(), 6);
    // Channels 5 and 6 meet no bus member: their VOUT nets stay local.
    let nl = build_project_netlist(&p.graph()).netlist;
    assert_eq!(refs_on(&nl, "VOUT_3V3_CH6"), ["CH6_C4", "CH6_L1"]);
}
