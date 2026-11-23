//! #1308: the compiled design lists the hierarchy with repeated sheets
//! expanded into channels, with part and net counts per row.
use signex_types::project::ChannelNaming;

use crate::compile_design;
use crate::fixtures::multi_channel::{BUCK, MCU, POWER_IN, TOP, project};

fn compiled() -> crate::CompiledDesign {
    compile_design(&project().graph(), &ChannelNaming::default())
}

#[test]
fn rows_are_depth_first_with_channels_under_their_sheet() {
    let c = compiled();
    let rows: Vec<(&str, usize)> = c
        .sheets
        .iter()
        .map(|s| (s.name.as_str(), s.depth))
        .collect();
    assert_eq!(
        rows,
        [
            ("top", 0),
            ("power_in", 1),
            ("buck_channel", 1),
            ("CH1", 2),
            ("CH2", 2),
            ("CH3", 2),
            ("CH4", 2),
            ("mcu", 1),
        ]
    );
    let files: Vec<&str> = c.sheets.iter().map(|s| s.file.as_str()).collect();
    assert_eq!(files, [TOP, POWER_IN, BUCK, BUCK, BUCK, BUCK, BUCK, MCU]);
}

#[test]
fn the_repeated_row_carries_the_repeat_and_channel_rows_their_channel() {
    let c = compiled();
    let header = &c.sheets[2];
    assert_eq!(
        header.repeat.as_ref().map(|r| r.to_designator()),
        Some("Repeat(CH, 1, 4)".to_string())
    );
    assert!(header.channel.is_none());
    for (i, row) in c.sheets[3..7].iter().enumerate() {
        let ch = row.channel.as_ref().expect("a channel row");
        assert_eq!(ch.index, i as u32 + 1);
        assert_eq!(ch.of, 4);
        assert!(row.repeat.is_none());
    }
}

#[test]
fn counts_per_row() {
    let c = compiled();
    let counts: Vec<(usize, usize)> = c.sheets.iter().map(|s| (s.parts, s.nets)).collect();
    assert_eq!(
        counts,
        [
            // top: no parts of its own; VIN_12V, EN and GND cross it.
            (0, 3),
            // power_in: J1, R1, J2.
            (3, 3),
            // buck_channel across its channels: shared VIN_12V/EN/GND plus
            // four VOUT_3V3 and four FB nets.
            (36, 11),
            (9, 5),
            (9, 5),
            (9, 5),
            (9, 5),
            // mcu: R11..R14 on the four channel nets.
            (4, 4),
        ]
    );
}

#[test]
fn totals_count_each_thing_once() {
    let c = compiled();
    assert_eq!(c.part_count, 3 + 36 + 4);
    assert_eq!(c.net_count, 11);
    assert_eq!(c.sheet_file_count, 4);
    assert_eq!(c.channel_count, 4);
    assert_eq!(c.occurrence_count(), 7);
    assert_eq!(c.sheet_keys().count(), 4);
}

#[test]
fn nets_list_their_sheet_count_and_power() {
    let c = compiled();
    let net = |name: &str| {
        c.nets
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("no net {name}"))
    };
    let vin = net("VIN_12V");
    assert_eq!(vin.sheets, 6, "top, power_in and four channels");
    assert!(!vin.is_power);
    assert!(net("GND").is_power);
    let vout = net("VOUT_3V3_CH2");
    assert_eq!(vout.sheets, 2, "channel CH2 and mcu");
    assert_eq!(c.nets.len(), c.net_count);
}

/// #2741: each sheet instance lists its own parts, by the designator it has
/// there, in walk order.
#[test]
fn each_occurrence_lists_its_own_components() {
    let c = compiled();
    let designators = |row: usize| -> Vec<&str> {
        c.sheets[row]
            .components
            .iter()
            .map(|p| p.designator.as_str())
            .collect()
    };
    assert_eq!(designators(1), ["J1", "R1", "J2"]);
    assert_eq!(designators(3).len(), 9);
    assert!(designators(3).iter().all(|d| d.starts_with("CH1_")));
    assert!(designators(4).iter().all(|d| d.starts_with("CH2_")));
    assert!(designators(2).is_empty(), "the channels list their own");
    let target = &c.sheets[4].components[0].target;
    assert_eq!(target.occurrence, c.sheets[4].path);
    assert_eq!(target.kind, signex_types::schematic::SelectedKind::Symbol);
}

/// #2741: each sheet instance lists the nets touching it, each with an
/// element of the net on that instance, as many as its count.
#[test]
fn each_occurrence_lists_its_nets_with_a_target_on_it() {
    let c = compiled();
    for row in c.sheets.iter().filter(|s| s.repeat.is_none()) {
        assert_eq!(row.net_names.len(), row.nets, "{}", row.name);
        for net in &row.net_names {
            assert_eq!(net.target.occurrence, row.path, "{}", net.name);
            assert_eq!(net.target.sheet, row.key, "{}", net.name);
        }
    }
    let ch2 = &c.sheets[4];
    assert!(ch2.net_names.iter().any(|n| n.name == "VOUT_3V3_CH2"));
    assert!(!ch2.net_names.iter().any(|n| n.name == "VOUT_3V3_CH1"));
    assert!(ch2.net_names.iter().any(|n| n.name == "GND" && n.is_power));
}

/// #2741: a net row has somewhere to go.
#[test]
fn every_net_has_a_target() {
    let c = compiled();
    assert!(c.nets.iter().all(|n| n.target.is_some()));
}

#[test]
fn a_project_without_repeats_has_no_channel_rows() {
    let mut p = project();
    p.sheet_mut(TOP).child_sheets[1].name = "buck".into();
    let c = compile_design(&p.graph(), &ChannelNaming::default());
    assert_eq!(c.channel_count, 0);
    assert!(
        c.sheets
            .iter()
            .all(|s| s.repeat.is_none() && s.channel.is_none())
    );
    assert_eq!(c.sheets.len(), 4);
    assert_eq!(c.sheets[2].name, "buck");
}
