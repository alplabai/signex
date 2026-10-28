use super::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A vertical bus (0,0)-(0,20) named `D[0..7]` mid-span, one entry at (0,5)
/// whose stub runs to (2.54, 7.54), and a wire from there to (12.54, 7.54).
struct Fixture {
    buses: Vec<(Point, Point)>,
    entries: Vec<(Point, (f64, f64))>,
    wires: Vec<(Point, Point)>,
    labels: Vec<(Point, &'static str)>,
}

impl Fixture {
    fn new(wire_label: Option<&'static str>) -> Self {
        let mut labels = vec![(pt(0.0, 10.0), "D[0..7]")];
        if let Some(text) = wire_label {
            labels.push((pt(6.0, 7.54), text));
        }
        Self {
            buses: vec![(pt(0.0, 0.0), pt(0.0, 20.0))],
            entries: vec![(pt(0.0, 5.0), (2.54, 2.54))],
            wires: vec![(pt(2.54, 7.54), pt(12.54, 7.54))],
            labels,
        }
    }

    fn analyse(&self) -> BusAnalysis {
        analyse_buses(&BusGeometry {
            buses: &self.buses,
            entries: &self.entries,
            wires: &self.wires,
            junctions: &[],
            labels: &self.labels,
        })
    }
}

#[test]
fn a_mid_span_label_names_the_whole_run() {
    let a = Fixture::new(None).analyse();
    assert_eq!(a.groups.len(), 1);
    let named: Vec<usize> = a.groups[0].names.iter().map(|n| n.label).collect();
    assert_eq!(named, [0]);
    assert_eq!(a.groups[0].members.len(), 8);
    assert!(a.groups[0].carries("D0") && a.groups[0].carries("D7"));
    assert!(!a.groups[0].carries("D8"));
}

#[test]
fn segments_sharing_an_endpoint_are_one_run_and_pool_their_names() {
    let mut f = Fixture::new(None);
    f.buses = vec![
        (pt(0.0, 0.0), pt(0.0, 10.0)),
        (pt(0.0, 10.0), pt(0.0, 20.0)),
    ];
    f.labels = vec![(pt(0.0, 3.0), "D[0..3]"), (pt(0.0, 15.0), "{SDA SCL}")];
    let a = f.analyse();
    assert_eq!(a.groups.len(), 1);
    assert_eq!(a.groups[0].segments, [0, 1]);
    assert_eq!(a.groups[0].members, ["D0", "D1", "D2", "D3", "SDA", "SCL"]);
}

#[test]
fn a_nameless_run_has_no_members_and_a_non_bus_label_is_rejected_not_adopted() {
    let mut f = Fixture::new(None);
    f.labels = vec![(pt(0.0, 0.0), "VCC")];
    let a = f.analyse();
    assert!(a.groups[0].names.is_empty());
    assert!(a.groups[0].members.is_empty());
    assert_eq!(a.groups[0].rejected.len(), 1);
}

#[test]
fn an_entry_reports_the_member_label_on_its_wire() {
    let a = Fixture::new(Some("D3")).analyse();
    assert_eq!(a.entries.len(), 1);
    assert_eq!(a.entries[0].group, 0);
    assert_eq!(a.entries[0].member_labels, [1]);
}

#[test]
fn an_unlabeled_entry_wire_has_no_member_labels() {
    let a = Fixture::new(None).analyse();
    assert_eq!(a.entries.len(), 1);
    assert!(a.entries[0].member_labels.is_empty());
}

#[test]
fn the_entry_end_off_the_bus_is_the_wire_end_whichever_way_it_is_drawn() {
    let mut f = Fixture::new(Some("D3"));
    // Same stub, drawn from the wire side with a negative size.
    f.entries = vec![(pt(2.54, 7.54), (-2.54, -2.54))];
    let a = f.analyse();
    assert_eq!(a.entries.len(), 1);
    assert_eq!(pt_key(&a.entries[0].wire_end), pt_key(&pt(2.54, 7.54)));
    assert_eq!(a.entries[0].member_labels, [1]);
}

#[test]
fn a_floating_entry_is_not_attached() {
    let mut f = Fixture::new(None);
    f.entries = vec![(pt(50.0, 50.0), (2.54, 2.54))];
    assert!(f.analyse().entries.is_empty());
}

#[test]
fn a_label_off_the_wire_is_not_a_member_label_of_the_entry() {
    let mut f = Fixture::new(None);
    f.labels.push((pt(6.0, 30.0), "D3"));
    assert!(f.analyse().entries[0].member_labels.is_empty());
}

#[test]
fn a_non_bus_label_mid_span_is_not_on_the_bus() {
    // Same rule `orphan_label` applies: only a bus-name label anchors
    // mid-span, any other label needs an endpoint.
    let mut f = Fixture::new(None);
    f.labels = vec![(pt(0.0, 10.0), "VCC")];
    assert!(f.analyse().groups[0].rejected.is_empty());
}

#[test]
fn a_t_branch_with_a_junction_is_part_of_the_named_run() {
    let buses = [
        (pt(0.0, 0.0), pt(0.0, 20.0)),
        (pt(0.0, 10.0), pt(10.0, 10.0)),
    ];
    let labels = [(pt(0.0, 3.0), "D[0..7]")];
    let a = analyse_buses(&BusGeometry {
        buses: &buses,
        junctions: &[pt(0.0, 10.0)],
        labels: &labels,
        ..Default::default()
    });
    assert_eq!(a.groups.len(), 1, "spur and trunk are one run");
    assert_eq!(a.groups[0].segments, [0, 1]);
    assert_eq!(a.groups[0].names.len(), 1);
}

#[test]
fn a_branch_ending_on_the_middle_of_a_bus_joins_it_with_or_without_a_dot() {
    let buses = [
        (pt(0.0, 0.0), pt(0.0, 20.0)),
        (pt(0.0, 10.0), pt(10.0, 10.0)),
    ];
    let a = analyse_buses(&BusGeometry {
        buses: &buses,
        ..Default::default()
    });
    assert_eq!(a.groups.len(), 1);
}

#[test]
fn a_junction_on_two_crossing_buses_joins_them() {
    let buses = [
        (pt(-5.0, 10.0), pt(5.0, 10.0)),
        (pt(0.0, 0.0), pt(0.0, 20.0)),
    ];
    let apart = analyse_buses(&BusGeometry {
        buses: &buses,
        ..Default::default()
    });
    assert_eq!(
        apart.groups.len(),
        2,
        "crossing without a dot is not a join"
    );
    let joined = analyse_buses(&BusGeometry {
        buses: &buses,
        junctions: &[pt(0.0, 10.0)],
        ..Default::default()
    });
    assert_eq!(joined.groups.len(), 1);
}

#[test]
fn two_different_names_on_one_run_are_both_kept() {
    let mut f = Fixture::new(None);
    f.labels = vec![(pt(0.0, 3.0), "D[0..7]"), (pt(0.0, 15.0), "D[8..15]")];
    let g = &f.analyse().groups[0];
    assert_eq!(g.names.len(), 2);
    assert_ne!(g.names[0].members, g.names[1].members);
    assert_eq!(g.names[0].width, g.names[1].width);
    assert_eq!(g.members.len(), 16);
}

#[test]
fn a_huge_label_text_is_refused_not_expanded() {
    let big = format!("{{{}}}", ["A[0..4095]"; 40].join(" "));
    let labels = [(pt(0.0, 3.0), big.as_str())];
    let buses = [(pt(0.0, 0.0), pt(0.0, 20.0))];
    let a = analyse_buses(&BusGeometry {
        buses: &buses,
        labels: &labels,
        ..Default::default()
    });
    assert!(a.groups[0].names.is_empty());
    assert!(a.groups[0].members.is_empty());
}

#[test]
fn a_big_sheet_is_analysed_correctly_and_each_lookup_stays_local() {
    // 2000 separate buses, each with a name, an entry, a wire and a member
    // label. Correctness is asserted here; the cost of a lookup is pinned by
    // `a_point_lookup_tests_only_the_segments_in_its_cell`, and the wall-clock
    // benchmark is `big_sheet_benchmark` (ignored by default).
    const N: usize = 2000;
    let (mut buses, mut entries, mut wires, mut labels) = (vec![], vec![], vec![], vec![]);
    let mut texts = vec![];
    for i in 0..N {
        let y = 30.0 * i as f64;
        buses.push((pt(0.0, y), pt(0.0, y + 20.0)));
        entries.push((pt(0.0, y + 5.0), (2.54, 2.54)));
        wires.push((pt(2.54, y + 7.54), pt(12.54, y + 7.54)));
        texts.push(("D[0..7]".to_string(), pt(0.0, y + 15.0)));
        texts.push(("D3".to_string(), pt(6.0, y + 7.54)));
    }
    for (t, p) in &texts {
        labels.push((*p, t.as_str()));
    }
    let a = analyse_buses(&BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &[],
        labels: &labels,
    });
    assert_eq!(a.groups.len(), N);
    assert_eq!(a.entries.len(), N);
    assert!(a.entries.iter().all(|e| e.member_labels.len() == 1));
}

#[test]
fn a_point_lookup_tests_only_the_segments_in_its_cell() {
    // Segments spaced 30 mm apart: no cell holds more than one of them, so a
    // lookup tests at most that one, however many segments the sheet has.
    use super::index::SegmentIndex;
    let segs: Vec<_> = (0..2000)
        .map(|i| {
            let y = 30.0 * i as f64;
            (pt(0.0, y), pt(0.0, y + 20.0))
        })
        .collect();
    let index = SegmentIndex::new(&segs);
    for (i, (a, b)) in segs.iter().enumerate() {
        for p in [a, b] {
            assert!(index.candidates(pt_key(p)) <= 2, "segment {i}");
        }
    }
    assert_eq!(index.containing(pt_key(&pt(0.0, 15.0))).count(), 1);
    assert_eq!(index.containing(pt_key(&pt(5.0, 15.0))).count(), 0);
}

#[test]
fn a_very_long_segment_is_kept_out_of_the_grid() {
    // A 10 m diagonal would register in 10^6 cells; it goes to the short
    // linear list instead and is still found.
    use super::index::SegmentIndex;
    let segs = [(pt(0.0, 0.0), pt(10_000.0, 10_000.0))];
    let index = SegmentIndex::new(&segs);
    assert_eq!(index.containing(pt_key(&pt(5_000.0, 5_000.0))).count(), 1);
    assert_eq!(index.candidates(pt_key(&pt(5_000.0, 5_000.0))), 1);
}

/// Wall-clock check of the 2000-run sheet; run with
/// `cargo test -p signex-net -- --ignored big_sheet_benchmark`.
#[test]
#[ignore = "benchmark"]
fn big_sheet_benchmark() {
    const N: usize = 20_000;
    let (mut buses, mut entries, mut wires) = (vec![], vec![], vec![]);
    let mut texts = vec![];
    for i in 0..N {
        let y = 30.0 * i as f64;
        buses.push((pt(0.0, y), pt(0.0, y + 20.0)));
        entries.push((pt(0.0, y + 5.0), (2.54, 2.54)));
        wires.push((pt(2.54, y + 7.54), pt(12.54, y + 7.54)));
        texts.push(("D[0..7]", pt(0.0, y + 15.0)));
        texts.push(("D3", pt(6.0, y + 7.54)));
    }
    let labels: Vec<_> = texts.iter().map(|(t, p)| (*p, *t)).collect();
    let start = std::time::Instant::now();
    let a = analyse_buses(&BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &[],
        labels: &labels,
    });
    eprintln!("{N} runs analysed in {:?}", start.elapsed());
    assert_eq!(a.groups.len(), N);
}
