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
    assert_eq!(a.groups[0].names, [0]);
    assert_eq!(a.groups[0].members.len(), 8);
    assert!(a.groups[0].carries("D0") && a.groups[0].carries("D7"));
    assert!(!a.groups[0].carries("D8"));
}

#[test]
fn segments_sharing_an_endpoint_are_one_run_and_pool_their_names() {
    let mut f = Fixture::new(None);
    f.buses = vec![(pt(0.0, 0.0), pt(0.0, 10.0)), (pt(0.0, 10.0), pt(0.0, 20.0))];
    f.labels = vec![(pt(0.0, 3.0), "D[0..3]"), (pt(0.0, 15.0), "{SDA SCL}")];
    let a = f.analyse();
    assert_eq!(a.groups.len(), 1);
    assert_eq!(a.groups[0].segments, [0, 1]);
    assert_eq!(a.groups[0].members, ["D0", "D1", "D2", "D3", "SDA", "SCL"]);
}

#[test]
fn a_nameless_run_has_no_members_and_a_non_bus_label_is_rejected_not_adopted() {
    let mut f = Fixture::new(None);
    f.labels = vec![(pt(0.0, 10.0), "VCC")];
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
