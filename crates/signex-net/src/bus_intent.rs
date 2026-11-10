//! The intent check of one headless bus run (#2199): what a bus from `start`
//! to `end` pulled onto its run besides what its two ends attach to. Rules:
//! `docs/CLI.md`, "Bus geometry". Built on [`analyse_buses`], the one bus
//! analysis, applied to the sheet before and after the run; there is no second
//! bus connectivity model.

use std::collections::HashSet;

use signex_types::schematic::{LabelType, Point, SchematicSheet};

use crate::join_rule::{NetNames, needs_allow_merge};
use crate::{BusAnalysis, BusGeometry, analyse_buses, point_on_segment, pt_key};

/// What kind of item a bus run pulled onto its own run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BusExtraKind {
    Bus,
    Entry,
    Label,
}

/// An item that was not on a run the ends attach to and is on the new run now.
#[derive(Debug, Clone, PartialEq)]
pub struct BusExtra {
    pub kind: BusExtraKind,
    /// The bus name (or "an unnamed bus"), the entry's uuid, or the label text.
    pub identity: String,
    pub position: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BusIntent {
    /// Items pulled onto the run that the two ends did not name.
    pub extra: Vec<BusExtra>,
    /// The distinct bus names of the runs the ends attach to.
    pub names: Vec<String>,
    /// Whether joining those runs needs `allow_merge` (the join rule).
    pub merge: bool,
    /// The bus names the new run carries after the run.
    pub resulting: Vec<String>,
}

/// One sheet's bus geometry and its analysis.
struct Analysed<'a> {
    sheet: &'a SchematicSheet,
    analysis: BusAnalysis,
    /// Run of every bus segment.
    seg_group: Vec<usize>,
}

impl<'a> Analysed<'a> {
    fn of(sheet: &'a SchematicSheet) -> Self {
        let buses: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
        let entries: Vec<(Point, (f64, f64))> = sheet
            .bus_entries
            .iter()
            .map(|e| (e.position, e.size))
            .collect();
        let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
        let junctions: Vec<Point> = sheet.junctions.iter().map(|j| j.position).collect();
        let labels: Vec<(Point, &str)> = sheet
            .labels
            .iter()
            .map(|l| (l.position, l.text.as_str()))
            .collect();
        let analysis = analyse_buses(&BusGeometry {
            buses: &buses,
            entries: &entries,
            wires: &wires,
            junctions: &junctions,
            labels: &labels,
        });
        let mut seg_group = vec![0; buses.len()];
        for (g, group) in analysis.groups.iter().enumerate() {
            for &s in &group.segments {
                seg_group[s] = g;
            }
        }
        Self {
            sheet,
            analysis,
            seg_group,
        }
    }

    /// The bus names of run `g`.
    fn names_of(&self, g: usize) -> Vec<String> {
        self.analysis.groups[g]
            .names
            .iter()
            .map(|n| self.sheet.labels[n.label].text.clone())
            .collect()
    }

    /// The bus names of run `g` split for the join rule: a hierarchical bus
    /// label is an alias, every other bus-name label an own name.
    fn split_of(&self, g: usize) -> NetNames {
        let mut names = NetNames::default();
        for n in &self.analysis.groups[g].names {
            add_bus_name(&mut names, &self.sheet.labels[n.label]);
        }
        names
    }

    /// The run a label names, if it names one.
    fn group_of_label(&self, label: usize) -> Option<usize> {
        self.analysis
            .groups
            .iter()
            .position(|g| g.names.iter().any(|n| n.label == label))
    }

    fn entry_ends(&self, entry: usize) -> [Point; 2] {
        let e = &self.sheet.bus_entries[entry];
        [
            e.position,
            Point::new(e.position.x + e.size.0, e.position.y + e.size.1),
        ]
    }
}

/// Add a bus-name label's text to `names`: an alias when hierarchical, else
/// an own name.
fn add_bus_name(names: &mut NetNames, label: &signex_types::schematic::Label) {
    let side = match label.label_type {
        LabelType::Hierarchical => &mut names.aliases,
        _ => &mut names.own,
    };
    side.insert(label.text.clone());
}

/// Whether a bus run through `p` on `sheet` can carry names from another
/// sheet: the run (by the bus analysis) has a hierarchical or global bus-name
/// label, or a sheet pin sits on one of its segments. The wire-side twin is
/// `net_reaches_other_sheets`.
pub fn bus_reaches_other_sheets(sheet: &SchematicSheet, p: Point) -> bool {
    let key = pt_key(&p);
    let analysed = Analysed::of(sheet);
    let on = |i: usize, k| {
        point_on_segment(
            k,
            pt_key(&sheet.buses[i].start),
            pt_key(&sheet.buses[i].end),
        )
    };
    let groups: HashSet<usize> = (0..sheet.buses.len())
        .filter(|&i| on(i, key))
        .map(|i| analysed.seg_group[i])
        .collect();
    groups.into_iter().any(|g| {
        let group = &analysed.analysis.groups[g];
        let ported = group.names.iter().any(|n| {
            matches!(
                sheet.labels[n.label].label_type,
                LabelType::Global | LabelType::Hierarchical
            )
        });
        let pinned = || {
            sheet
                .child_sheets
                .iter()
                .flat_map(|c| &c.pins)
                .any(|pin| group.segments.iter().any(|&s| on(s, pt_key(&pin.position))))
        };
        ported || pinned()
    })
}

/// The bus runs whose names change from `before` to `after` (a label or a
/// junction merging two differently named runs, or a named run gaining a
/// name): the bus half of the point placement check.
pub(crate) fn bus_name_changes(
    before: &SchematicSheet,
    after: &SchematicSheet,
) -> Vec<crate::build::NameChange> {
    let (b, a) = (Analysed::of(before), Analysed::of(after));
    let pairs: Vec<(usize, usize)> = before
        .buses
        .iter()
        .enumerate()
        .filter_map(|(i, bus)| {
            let j = after.buses.iter().position(|x| x.uuid == bus.uuid)?;
            Some((b.seg_group[i], a.seg_group[j]))
        })
        .collect();
    crate::build::name_changes(
        &pairs,
        |g| b.split_of(g),
        |g| (a.split_of(g), a.names_of(g)),
        true,
    )
}

/// Judge the bus run from `start` to `end` that turned `before` into `after`
/// (the run's own segments are the ones of `after` that `before` lacks).
pub fn bus_intent(
    before: &SchematicSheet,
    after: &SchematicSheet,
    start: Point,
    end: Point,
) -> BusIntent {
    let (b, a) = (Analysed::of(before), Analysed::of(after));
    let end_keys = [pt_key(&start), pt_key(&end)];

    // The runs an end attaches to: through a segment (end or interior), an
    // entry end, or a bus-name label at the point.
    let mut intended: HashSet<usize> = HashSet::new();
    for key in end_keys {
        for (i, bus) in before.buses.iter().enumerate() {
            if point_on_segment(key, pt_key(&bus.start), pt_key(&bus.end)) {
                intended.insert(b.seg_group[i]);
            }
        }
        for att in &b.analysis.entries {
            if b.entry_ends(att.entry).iter().any(|p| pt_key(p) == key) {
                intended.insert(att.group);
            }
        }
        for (g, group) in b.analysis.groups.iter().enumerate() {
            if group
                .names
                .iter()
                .any(|n| pt_key(&before.labels[n.label].position) == key)
            {
                intended.insert(g);
            }
        }
    }
    let mut ordered: Vec<usize> = intended.iter().copied().collect();
    ordered.sort_unstable();
    let mut names: Vec<String> = Vec::new();
    for &g in &ordered {
        for name in b.names_of(g) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let mut splits: Vec<NetNames> = ordered.iter().map(|&g| b.split_of(g)).collect();
    // A bus-name label at an end that sits on no run yet: the run gives it a
    // run, so it takes part in the merge like a named run.
    for (i, label) in before.labels.iter().enumerate() {
        let lone = end_keys.contains(&pt_key(&label.position))
            && b.group_of_label(i).is_none()
            && a.group_of_label(i).is_some();
        if lone {
            let mut one = NetNames::default();
            add_bus_name(&mut one, label);
            splits.push(one);
            if !names.contains(&label.text) {
                names.push(label.text.clone());
            }
        }
    }
    let refs: Vec<&NetNames> = splits.iter().collect();
    let own = splits.iter().flat_map(|s| s.own.iter().cloned()).collect();
    let merge = needs_allow_merge(&refs, &own);

    let before_ids: HashSet<_> = before.buses.iter().map(|x| x.uuid).collect();
    let new_group = after
        .buses
        .iter()
        .position(|x| !before_ids.contains(&x.uuid))
        .map(|i| a.seg_group[i]);
    let Some(new_group) = new_group else {
        return BusIntent {
            extra: Vec::new(),
            names,
            merge,
            resulting: Vec::new(),
        };
    };

    let at_end = |p: &Point| end_keys.contains(&pt_key(p));
    let mut extra = Vec::new();
    let mut seen_runs: HashSet<usize> = HashSet::new();
    for (i, bus) in before.buses.iter().enumerate() {
        let Some(j) = after.buses.iter().position(|x| x.uuid == bus.uuid) else {
            continue;
        };
        let g = b.seg_group[i];
        if a.seg_group[j] == new_group && !intended.contains(&g) && seen_runs.insert(g) {
            let identity = b
                .names_of(g)
                .into_iter()
                .next()
                .map_or_else(|| "an unnamed bus".to_string(), |n| format!("bus {n}"));
            extra.push(BusExtra {
                kind: BusExtraKind::Bus,
                identity,
                position: bus.start,
            });
        }
    }
    for att in a.analysis.entries.iter().filter(|e| e.group == new_group) {
        let was = b
            .analysis
            .entries
            .iter()
            .find(|e| e.entry == att.entry)
            .is_some_and(|e| intended.contains(&e.group));
        let ends = a.entry_ends(att.entry);
        if !was && !ends.iter().any(at_end) {
            extra.push(BusExtra {
                kind: BusExtraKind::Entry,
                identity: after.bus_entries[att.entry].uuid.to_string(),
                position: ends[0],
            });
        }
    }
    for name in &a.analysis.groups[new_group].names {
        let label = &after.labels[name.label];
        let was = b
            .group_of_label(name.label)
            .is_some_and(|g| intended.contains(&g));
        if !was && !at_end(&label.position) {
            extra.push(BusExtra {
                kind: BusExtraKind::Label,
                identity: label.text.clone(),
                position: label.position,
            });
        }
    }
    extra.sort_by(|x, y| (x.kind, &x.identity).cmp(&(y.kind, &y.identity)));
    extra.dedup_by(|x, y| x.kind == y.kind && x.identity == y.identity);

    let mut resulting: Vec<String> = Vec::new();
    for name in a.names_of(new_group) {
        if !resulting.contains(&name) {
            resulting.push(name);
        }
    }
    BusIntent {
        extra,
        names,
        merge,
        resulting,
    }
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Bus, BusEntry, LabelType};
    use uuid::Uuid;

    use super::*;
    use crate::fixtures::{empty_sheet, label, pt};

    fn bus(a: (f64, f64), b: (f64, f64)) -> Bus {
        Bus {
            uuid: Uuid::now_v7(),
            start: pt(a.0, a.1),
            end: pt(b.0, b.1),
        }
    }

    type Xy = (f64, f64);

    /// `before` with the run's segments added, as `place_bus` makes them.
    fn run(before: &SchematicSheet, segments: &[(Xy, Xy)]) -> SchematicSheet {
        let mut after = before.clone();
        after
            .buses
            .extend(segments.iter().map(|(a, b)| bus(*a, *b)));
        after
    }

    fn judge(before: &SchematicSheet, segments: &[(Xy, Xy)], start: Xy, end: Xy) -> BusIntent {
        let after = run(before, segments);
        bus_intent(before, &after, pt(start.0, start.1), pt(end.0, end.1))
    }

    fn two_runs() -> SchematicSheet {
        let mut sheet = empty_sheet();
        sheet.buses.push(bus((-10.0, 0.0), (0.0, 0.0)));
        sheet.buses.push(bus((20.0, 0.0), (30.0, 0.0)));
        sheet
    }

    #[test]
    fn a_run_between_two_runs_with_one_name_is_intended() {
        let mut before = two_runs();
        before
            .labels
            .push(label("D[0..7]", pt(-10.0, 0.0), LabelType::Net));
        let got = judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0))],
            (0.0, 0.0),
            (20.0, 0.0),
        );
        assert!(got.extra.is_empty(), "{got:?}");
        assert_eq!(got.names, ["D[0..7]"]);
        assert_eq!(got.resulting, ["D[0..7]"]);
    }

    #[test]
    fn two_different_bus_names_are_a_merge() {
        let mut before = two_runs();
        before
            .labels
            .push(label("D[0..7]", pt(-10.0, 0.0), LabelType::Net));
        before
            .labels
            .push(label("A[0..15]", pt(30.0, 0.0), LabelType::Net));
        let got = judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0))],
            (0.0, 0.0),
            (20.0, 0.0),
        );
        assert_eq!(got.names, ["D[0..7]", "A[0..15]"]);
        assert!(got.merge);
        assert!(got.extra.is_empty());
    }

    /// The run between the two runs, the left one named `left`, the right one
    /// `right`, each label of the given kind.
    fn join_named(left: (&str, LabelType), right: (&str, LabelType)) -> BusIntent {
        let mut before = two_runs();
        before.labels.push(label(left.0, pt(-10.0, 0.0), left.1));
        before.labels.push(label(right.0, pt(30.0, 0.0), right.1));
        judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0))],
            (0.0, 0.0),
            (20.0, 0.0),
        )
    }

    #[test]
    fn bus_names_follow_the_net_rule_with_hierarchical_labels_as_aliases() {
        // 6761836527 point 7, by points 2-4: an own-named run with an
        // alias-only one joins freely; two alias-only runs short only when
        // they share no name.
        let (hier, net) = (LabelType::Hierarchical, LabelType::Net);
        assert!(!join_named(("D[0..7]", hier), ("DATA[0..7]", net)).merge);
        assert!(!join_named(("DATA[0..7]", net), ("D[0..7]", hier)).merge);
        assert!(join_named(("D[0..7]", hier), ("A[0..15]", hier)).merge);
        assert!(!join_named(("D[0..7]", hier), ("D[0..7]", hier)).merge);
        assert!(join_named(("D[0..7]", net), ("A[0..15]", LabelType::Global)).merge);
    }

    #[test]
    fn a_lone_bus_name_label_at_an_end_takes_part_in_the_merge() {
        // 6761836527 point 7: a run from the run A[0..15] to the lone label
        // D[0..7] (on no bus yet) joins two own bus names: a merge. To an
        // unnamed run it only names the run.
        for (run_name, merge) in [(Some("A[0..15]"), true), (None, false)] {
            let mut before = empty_sheet();
            before.buses.push(bus((12.7, 25.4), (25.4, 25.4)));
            if let Some(name) = run_name {
                before
                    .labels
                    .push(label(name, pt(12.7, 25.4), LabelType::Net));
            }
            before
                .labels
                .push(label("D[0..7]", pt(50.8, 25.4), LabelType::Net));
            let got = judge(
                &before,
                &[((19.05, 25.4), (50.8, 25.4))],
                (19.05, 25.4),
                (50.8, 25.4),
            );
            assert_eq!(got.merge, merge, "{run_name:?}: {got:?}");
            assert!(got.names.contains(&"D[0..7]".to_string()), "{got:?}");
        }
    }

    #[test]
    fn a_hierarchical_bus_label_on_a_named_run_renames_nothing() {
        // A child's hierarchical D[0..7] on a run labelled DATA[0..7]: an
        // alias, so no change; a second own name is a rename (6761836527).
        let mut before = empty_sheet();
        before.buses.push(bus((0.0, 0.0), (20.0, 0.0)));
        before
            .labels
            .push(label("DATA[0..7]", pt(0.0, 0.0), LabelType::Net));
        let with = |text: &str, ty: LabelType| {
            let mut after = before.clone();
            after.labels.push(label(text, pt(20.0, 0.0), ty));
            after
        };
        let hier = with("D[0..7]", LabelType::Hierarchical);
        assert!(bus_name_changes(&before, &hier).is_empty());
        let own = with("D[0..7]", LabelType::Net);
        assert_eq!(bus_name_changes(&before, &own).len(), 1);
    }

    #[test]
    fn another_run_touched_mid_path_or_at_a_bend_is_extra() {
        // A stub whose end lies on the path joins it.
        let mut before = two_runs();
        before.buses.push(bus((10.0, 0.0), (10.0, 5.0)));
        let got = judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0))],
            (0.0, 0.0),
            (20.0, 0.0),
        );
        assert_eq!(got.extra.len(), 1, "{got:?}");
        assert_eq!(got.extra[0].kind, BusExtraKind::Bus);

        // A bend landing on another run.
        let mut before = empty_sheet();
        before.buses.push(bus((20.0, 10.0), (30.0, 10.0)));
        before.buses.push(bus((-10.0, 0.0), (0.0, 0.0)));
        before.buses.push(bus((20.0, 0.0), (30.0, 0.0)));
        let got = judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0)), ((20.0, 0.0), (20.0, 10.0))],
            (0.0, 0.0),
            (20.0, 10.0),
        );
        assert!(
            got.extra.iter().any(|x| x.kind == BusExtraKind::Bus),
            "the run at the bend is pulled in: {got:?}"
        );
    }

    #[test]
    fn a_run_over_a_bus_entry_or_a_bus_name_label_is_extra() {
        let mut before = two_runs();
        before.bus_entries.push(BusEntry {
            uuid: Uuid::now_v7(),
            position: pt(10.0, 0.0),
            size: (2.54, 2.54),
        });
        let got = judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0))],
            (0.0, 0.0),
            (20.0, 0.0),
        );
        assert_eq!(got.extra.len(), 1, "{got:?}");
        assert_eq!(got.extra[0].kind, BusExtraKind::Entry);

        let mut before = two_runs();
        before
            .labels
            .push(label("X[0..3]", pt(10.0, 0.0), LabelType::Net));
        let got = judge(
            &before,
            &[((0.0, 0.0), (20.0, 0.0))],
            (0.0, 0.0),
            (20.0, 0.0),
        );
        assert!(
            got.extra
                .iter()
                .any(|x| x.kind == BusExtraKind::Label && x.identity == "X[0..3]"),
            "{got:?}"
        );
    }
}
