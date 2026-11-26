//! References against the design (mockup row 196): resolution, the three
//! stale causes, their fixes, the hover card and the `@` picker.

use uuid::Uuid;

use crate::{
    CONTEXT_ACTIONS, CardAction, CardFacts, DesignIndex, DesignNet, DesignPart, DesignPin, Fix,
    FixError, FixOffer, Header, NotebookFile, PinName, Reference, ReferenceState, apply_fix,
    apply_plan, badges, markup_text, natural_order, picker_entries, plan_update_all,
    reference_card, references, resolve_one, resolve_references, scan,
};

const R7_ID: u128 = 7;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pin(number: &str, name: &str, net: Option<&str>) -> DesignPin {
    DesignPin {
        number: number.into(),
        name: name.into(),
        net: net.map(str::to_string),
    }
}

fn part(designator: &str, n: u128, value: &str, pins: Vec<DesignPin>) -> DesignPart {
    DesignPart {
        designator: designator.into(),
        id: id(n),
        value: value.into(),
        description: String::new(),
        footprint: String::new(),
        sheet: "power.snxsch".into(),
        fitted_in: Vec::new(),
        variant_count: 0,
        pins,
    }
}

fn pin_name(component: &str, pin: &str) -> PinName {
    PinName {
        component: component.into(),
        pin: pin.into(),
    }
}

fn net(name: &str, pins: &[(&str, &str)], sheets: &[&str]) -> DesignNet {
    DesignNet {
        name: name.into(),
        pins: pins.iter().map(|(c, p)| pin_name(c, p)).collect(),
        sheets: sheets.iter().map(|s| s.to_string()).collect(),
    }
}

/// The buck stage of the mock-up: R7 has been renamed R17.
fn design() -> DesignIndex {
    let mut u3 = part(
        "U3",
        3,
        "Buck 3 A",
        vec![
            pin("1", "VIN", Some("VIN")),
            pin("3", "SW", Some("SW")),
            pin("12", "FB", Some("FB")),
        ],
    );
    u3.description = "buck regulator".into();
    let mut l1 = part(
        "L1",
        1,
        "4.7µH",
        vec![pin("1", "", Some("SW")), pin("2", "", Some("+3V3"))],
    );
    l1.footprint = "IND_4040".into();
    l1.fitted_in = vec!["Base".into(), "Lite".into(), "Pro".into()];
    l1.variant_count = 3;
    let parts = vec![
        u3,
        l1,
        part("R17", R7_ID, "100k", vec![pin("1", "", Some("FB"))]),
        part("C9", 9, "100n", vec![pin("2", "", Some("SW"))]),
    ];
    let nets = vec![
        net(
            "SW",
            &[("U3", "3"), ("L1", "1"), ("C9", "2")],
            &["power.snxsch"],
        ),
        net("+3V3", &[("L1", "2")], &["power.snxsch", "mcu.snxsch"]),
        net("FB", &[("U3", "12"), ("R17", "1")], &["power.snxsch"]),
        net("VIN", &[("U3", "1")], &["power.snxsch"]),
    ];
    DesignIndex::new(parts, nets)
}

fn component(name: &str) -> Reference {
    Reference::Component(name.into())
}

fn pin_ref(component: &str, pin: &str) -> Reference {
    Reference::Pin {
        component: component.into(),
        pin: pin.into(),
    }
}

/// A notebook whose header stored R7's id.
const PAGE: &str = "+++\nformat = \"snxnbk/1\"\n\n[[references]]\nkind = \"component\"\ncomponent = \"R7\"\nid = \"00000000-0000-0000-0000-000000000007\"\n+++\n= Inductor selection\n#component(\"L1\") is driven by #pin(\"U3\", \"3\").\nThe output set by #component(\"R7\") on #net(\"+3V3\").\nU7 was removed: #component(\"U7\"). Power good #pin(\"U3.14\").\nAgain #component(\"R7\").\n";

fn report_of(text: &str) -> crate::ReferenceReport {
    let file = NotebookFile::parse(text).expect("parses");
    resolve_references(&references(text), &file.header, &design())
}

#[test]
fn the_design_index_sorts_naturally_and_finds_parts_pins_and_nets() {
    let design = design();
    let names: Vec<_> = design
        .parts()
        .iter()
        .map(|p| p.designator.as_str())
        .collect();
    assert_eq!(names, ["C9", "L1", "R17", "U3"]);
    assert_eq!(natural_order("R2", "R10"), std::cmp::Ordering::Less);
    assert_eq!(natural_order("R10", "R10"), std::cmp::Ordering::Equal);
    let u3 = design.part("U3").expect("U3");
    assert_eq!(u3.pin("3").map(|p| p.name.as_str()), Some("SW"));
    assert_eq!(
        u3.pin("FB").map(|p| p.number.as_str()),
        Some("12"),
        "by name"
    );
    assert!(u3.pin("14").is_none());
    assert_eq!(design.nets_of(u3), ["VIN", "SW", "FB"]);
    assert_eq!(design.net("SW").expect("SW").parts(), ["C9", "L1", "U3"]);
    assert_eq!(design.parts_with_id(id(R7_ID)).count(), 1);
}

#[test]
fn a_reference_the_design_has_resolves_and_counts_every_use() {
    let report = report_of(PAGE);
    let l1 = report.get(&component("L1")).expect("L1");
    assert_eq!(l1.state, ReferenceState::Resolved);
    assert_eq!(l1.id, Some(id(1)));
    assert_eq!(l1.lines, [10]);
    assert_eq!(
        report
            .get(&Reference::Net("+3V3".into()))
            .expect("net")
            .state,
        ReferenceState::Resolved
    );
    assert_eq!(report.get(&component("R7")).expect("R7").uses(), 2);
    assert_eq!(report.total_uses(), 7);
}

#[test]
fn a_renamed_part_is_found_by_its_stored_id() {
    let report = report_of(PAGE);
    let r7 = report.get(&component("R7")).expect("R7");
    assert_eq!(
        r7.state,
        ReferenceState::Renamed {
            now: component("R17")
        }
    );
    assert_eq!(r7.id, Some(id(R7_ID)));
    assert_eq!(
        r7.offers(),
        [FixOffer::Update {
            to: component("R17")
        }]
    );
}

#[test]
fn a_part_not_in_the_design_is_deleted_and_a_missing_pin_lists_the_pins() {
    let report = report_of(PAGE);
    let u7 = report.get(&component("U7")).expect("U7");
    assert_eq!(u7.state, ReferenceState::Deleted);
    assert_eq!(
        u7.offers(),
        [FixOffer::RemoveReference, FixOffer::KeepAsPlainText]
    );
    let u3_14 = report.get(&pin_ref("U3", "14")).expect("U3.14");
    assert_eq!(
        u3_14.state,
        ReferenceState::MissingPin {
            pins: vec!["1".into(), "3".into(), "12".into()]
        }
    );
    assert_eq!(report.stale_count(), 3, "R7, U7 and U3.14 as drawn");
    let net = resolve_one(&Reference::Net("GONE".into()), None, &design());
    assert_eq!(net.0, ReferenceState::Deleted);
}

#[test]
fn a_pin_of_a_renamed_part_reads_as_renamed_with_its_pin() {
    let (state, found) = resolve_one(&pin_ref("R7", "1"), Some(id(R7_ID)), &design());
    assert_eq!(
        state,
        ReferenceState::Renamed {
            now: pin_ref("R17", "1")
        }
    );
    assert_eq!(found, Some(id(R7_ID)));
    let (missing, _) = resolve_one(&pin_ref("R7", "9"), Some(id(R7_ID)), &design());
    assert!(matches!(missing, ReferenceState::MissingPin { .. }));
}

/// #3021: every placement of a reused sheet carries the same id, so a
/// rename resolves within the reference's own channel, and a channel that
/// is gone reads as deleted, never as renamed to another channel.
#[test]
fn a_rename_in_a_reused_sheet_stays_in_its_channel() {
    const U: u128 = 40;
    let renamed = DesignIndex::new(
        vec![
            part("R1_A", U, "10k", vec![]),
            part("R2_B", U, "10k", vec![]),
        ],
        vec![],
    );
    let (state, _) = resolve_one(&component("R1_B"), Some(id(U)), &renamed);
    assert_ne!(
        state,
        ReferenceState::Renamed {
            now: component("R1_A")
        }
    );
    assert_eq!(
        state,
        ReferenceState::Renamed {
            now: component("R2_B")
        }
    );
    let channel_gone = DesignIndex::new(vec![part("R1_A", U, "10k", vec![])], vec![]);
    let (gone, _) = resolve_one(&component("R1_B"), Some(id(U)), &channel_gone);
    assert_eq!(gone, ReferenceState::Deleted);
}

#[test]
fn a_stored_id_no_part_carries_falls_back_to_the_name() {
    let (state, found) = resolve_one(&component("L1"), Some(id(999)), &design());
    assert_eq!(state, ReferenceState::Resolved);
    assert_eq!(found, Some(id(1)));
}

#[test]
fn resolved_ids_are_stored_and_unused_references_dropped() {
    let report = report_of(PAGE);
    let mut header = Header::default();
    header.set_id(&component("GONE"), Some(id(5)));
    assert!(report.store_ids(&mut header));
    assert_eq!(header.id_of(&component("L1")), Some(id(1)));
    assert_eq!(header.id_of(&component("R7")), Some(id(R7_ID)));
    assert_eq!(header.id_of(&component("GONE")), None);
    assert!(
        !report.store_ids(&mut header),
        "a second store changes nothing"
    );
}

#[test]
fn update_rewrites_every_use_and_keeps_the_stored_id() {
    let fixed = apply_fix(
        PAGE,
        &component("R7"),
        &Fix::UpdateTo {
            to: component("R17"),
        },
    )
    .expect("fixed");
    assert_eq!(fixed.changed, 2);
    assert!(fixed.text.contains("set by #component(\"R17\") on"));
    assert!(fixed.text.contains("Again #component(\"R17\")."));
    assert!(!fixed.text.contains("\"R7\""));
    let file = NotebookFile::parse(&fixed.text).expect("parses");
    assert_eq!(file.header.id_of(&component("R17")), Some(id(R7_ID)));
    let report = resolve_references(&references(&fixed.text), &file.header, &design());
    assert_eq!(
        report.get(&component("R17")).expect("R17").state,
        ReferenceState::Resolved
    );
}

#[test]
fn remove_and_keep_as_plain_text_rewrite_markup() {
    let removed = apply_fix(PAGE, &component("U7"), &Fix::RemoveReference).expect("removed");
    assert!(removed.text.contains("U7 was removed: . Power good"));
    let kept =
        apply_fix(PAGE, &Reference::Net("+3V3".into()), &Fix::KeepAsPlainText).expect("kept");
    assert!(kept.text.contains("on \\+3V3."), "{}", kept.text);
    assert_eq!(markup_text("a*b_c"), "a\\*b\\_c");
}

#[test]
fn pick_another_pin_points_every_use_at_the_new_pin() {
    let fixed = apply_fix(
        PAGE,
        &pin_ref("U3", "14"),
        &Fix::PickAnotherPin { pin: "12".into() },
    )
    .expect("fixed");
    assert!(fixed.text.contains("Power good #pin(\"U3\", \"12\")."));
    assert_eq!(
        apply_fix(
            PAGE,
            &component("U7"),
            &Fix::PickAnotherPin { pin: "1".into() }
        ),
        Err(FixError::NotOffered {
            fix: "Pick another pin",
            name: "U7".into()
        })
    );
}

#[test]
fn a_fix_that_would_break_code_is_refused() {
    let code = "#note(target: component(\"U7\"))[x]\n";
    assert!(matches!(
        apply_fix(code, &component("U7"), &Fix::RemoveReference),
        Err(FixError::InCode { line: 1, .. })
    ));
    let kept = apply_fix(code, &component("U7"), &Fix::KeepAsPlainText).expect("a string");
    assert_eq!(kept.text, "#note(target: \"U7\")[x]\n");
    let value = "R is #component(\"U7\").value here\n";
    assert!(matches!(
        apply_fix(value, &component("U7"), &Fix::KeepAsPlainText),
        Err(FixError::ReadAsValue { line: 1, .. })
    ));
    assert_eq!(
        apply_fix(value, &component("Q1"), &Fix::RemoveReference),
        Err(FixError::NotUsed("Q1".into()))
    );
}

#[test]
fn update_all_lists_every_stale_reference_and_applies_only_the_renames() {
    let report = report_of(PAGE);
    let plan = plan_update_all(&report);
    let listed: Vec<_> = plan.iter().map(|p| p.reference.name()).collect();
    assert_eq!(listed, ["R7", "U7", "U3.14"]);
    assert_eq!(
        plan[0].fix,
        Some(Fix::UpdateTo {
            to: component("R17")
        })
    );
    assert_eq!(plan[0].uses, 2);
    assert!(
        plan[1].fix.is_none() && plan[2].fix.is_none(),
        "need a choice"
    );
    let applied = apply_plan(PAGE, &plan).expect("applied");
    assert_eq!(applied.changed, 2);
    assert!(
        applied.text.contains("#component(\"U7\")"),
        "left for a choice"
    );
}

fn card_of(reference: &Reference) -> crate::ReferenceCard {
    let report = report_of(PAGE);
    let entry = report.get(reference).expect("used");
    let (blocks, _) = scan(
        "#annotation(kind: \"measurement\", status: \"done\", date: \"2026-11-24\", title: \"Rise\")[#pin(\"U3\", \"3\") 4.1 ns]\n\
         #annotation(kind: \"measurement\", status: \"done\", date: \"2026-11-20\", title: \"Old\")[#pin(\"U3\", \"3\")]\n\
         #annotation(kind: \"note\")[#component(\"L1\") chosen]\n",
    );
    let badges = badges([("design.snxnbk", blocks.as_slice())]);
    reference_card(entry, &design(), &badges, report.stale_count())
}

#[test]
fn a_part_card_lists_value_footprint_sheet_nets_variants_and_uses() {
    let card = card_of(&component("L1"));
    let CardFacts::Part {
        value,
        footprint,
        sheet,
        nets,
        fitted_in,
        variant_count,
        ..
    } = &card.facts
    else {
        panic!("a part card: {:?}", card.facts);
    };
    assert_eq!((value.as_str(), footprint.as_str()), ("4.7µH", "IND_4040"));
    assert_eq!(sheet, "power.snxsch");
    assert_eq!(nets, &["SW", "+3V3"]);
    assert_eq!((fitted_in.len(), *variant_count), (3, 3));
    assert_eq!(card.blocks.len(), 1);
    assert_eq!(
        card.actions,
        [
            CardAction::CrossProbe,
            CardAction::OpenSheet,
            CardAction::Timeline
        ]
    );
    assert!(card.last_measurement.is_none(), "a part card has none");
}

#[test]
fn a_pin_card_lists_its_net_connected_pins_and_last_measurement() {
    let card = card_of(&pin_ref("U3", "3"));
    let CardFacts::Pin {
        pin_name,
        net,
        connected,
        part_description,
        ..
    } = &card.facts
    else {
        panic!("a pin card: {:?}", card.facts);
    };
    assert_eq!(pin_name, "SW");
    assert_eq!(part_description, "buck regulator");
    assert_eq!(net.as_deref(), Some("SW"));
    assert_eq!(connected, &[pin_name_of("C9", "2"), pin_name_of("L1", "1")]);
    assert_eq!(
        card.last_measurement.as_ref().map(|m| m.title.as_str()),
        Some("Rise"),
        "the newest dated one"
    );
    assert_eq!(
        card.actions,
        [
            CardAction::CrossProbe,
            CardAction::Timeline,
            CardAction::CopyReference
        ]
    );
}

fn pin_name_of(component: &str, pin: &str) -> PinName {
    pin_name(component, pin)
}

#[test]
fn a_net_card_lists_its_pins_parts_and_sheets() {
    let card = card_of(&Reference::Net("+3V3".into()));
    assert_eq!(
        card.facts,
        CardFacts::Net {
            pin_count: 1,
            parts: vec!["L1".into()],
            sheets: vec!["mcu.snxsch".into(), "power.snxsch".into()],
        }
    );
    assert_eq!(
        card.actions,
        [
            CardAction::CrossProbe,
            CardAction::HighlightNet,
            CardAction::Timeline
        ]
    );
}

#[test]
fn a_stale_card_says_why_and_offers_its_fixes() {
    let card = card_of(&component("R7"));
    let CardFacts::Stale {
        state,
        value,
        fixes,
        ..
    } = &card.facts
    else {
        panic!("a stale card: {:?}", card.facts);
    };
    assert!(matches!(state, ReferenceState::Renamed { .. }));
    assert_eq!(value.as_deref(), Some("100k"), "the same part");
    assert_eq!(
        fixes,
        &[FixOffer::Update {
            to: component("R17")
        }]
    );
    assert_eq!(card.uses, 2);
    assert_eq!(
        card.actions,
        [CardAction::UpdateAllStale, CardAction::ShowInSource]
    );
    let deleted = card_of(&component("U7"));
    assert_eq!(deleted.actions, [CardAction::ShowInSource]);
    assert_eq!(CONTEXT_ACTIONS.len(), 5);
}

#[test]
fn typing_at_lists_parts_then_nets_and_a_dot_lists_pins() {
    let design = design();
    let all = picker_entries(&design, "", 50);
    assert_eq!(all.len(), 8, "four parts and four nets");
    assert_eq!(all[0].reference, component("C9"));
    let r = picker_entries(&design, "r1", 50);
    assert_eq!(r[0].reference, component("R17"));
    assert_eq!(r[0].insert, "#component(\"R17\")");
    assert_eq!(r[0].detail, "100k");
    let pins = picker_entries(&design, "u3.", 50);
    let names: Vec<_> = pins.iter().map(|e| e.reference.name()).collect();
    assert_eq!(names, ["U3.1", "U3.3", "U3.12"]);
    let by_name = picker_entries(&design, "U3.f", 50);
    assert_eq!(by_name[0].insert, "#pin(\"U3\", \"12\")");
    let contains = picker_entries(&design, "3V", 50);
    assert_eq!(contains[0].reference, Reference::Net("+3V3".into()));
    assert_eq!(picker_entries(&design, "", 2).len(), 2, "the limit holds");
    assert!(picker_entries(&design, "Q9.", 50).is_empty());
}
