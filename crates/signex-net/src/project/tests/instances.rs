//! Per-placement designators (#829): one child file placed twice.
use super::*;
use signex_types::schematic::SymbolInstance;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn entry(chain: &[Uuid], reference: &str) -> SymbolInstance {
    SymbolInstance {
        project: "p".to_string(),
        path: InstancePath::from_chain(chain.to_vec()).to_string(),
        reference: reference.to_string(),
        unit: 1,
    }
}

/// Root with two placements ("A" = uuid 1, "B" = uuid 2) of `child.sch`, whose
/// one resistor `R1` sits on the net bound to sheet pin `P`.
fn twice_placed(with_entries: bool) -> (SchematicSheet, HashMap<String, SchematicSheet>) {
    let mut root = empty_sheet();
    let mut a = child_sheet("A", "child.sch", vec![sheet_pin("P", pt(0.0, 0.0))]);
    a.uuid = id(1);
    let mut b = child_sheet("B", "child.sch", vec![sheet_pin("P", pt(20.0, 0.0))]);
    b.uuid = id(2);
    root.child_sheets.push(a);
    root.child_sheets.push(b);

    let mut child = empty_sheet();
    child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    child
        .labels
        .push(label("P", pt(0.0, 0.0), LabelType::Hierarchical));
    add_lib(&mut child, "R");
    place(&mut child, "R1", "R", pt(10.0, 0.0));
    if with_entries {
        child.symbols[0].instances = vec![
            entry(&[id(1)], "R1"),
            entry(&[id(2)], "R101"),
            // Matches no placement and is unparseable: neither is honoured.
            entry(&[id(9)], "R999"),
            SymbolInstance {
                path: "not a path".to_string(),
                reference: "R998".to_string(),
                ..Default::default()
            },
        ];
    }
    let mut map = HashMap::new();
    map.insert("child.sch".to_string(), child);
    (root, map)
}

fn shared_reference_issues(p: &ProjectNetlist) -> Vec<&StitchIssue> {
    p.issues
        .iter()
        .filter(|i| matches!(i, StitchIssue::SharedReferenceAcrossInstances { .. }))
        .collect()
}

#[test]
fn placements_with_entries_get_distinct_terminal_designators() {
    let (root, map) = twice_placed(true);
    let p = stitch(&root, "root", &map);
    assert!(
        shared_reference_issues(&p).is_empty(),
        "no real collision: {:?}",
        p.issues
    );
    let mut refs: Vec<&str> = p
        .netlist
        .nets
        .iter()
        .flat_map(|n| &n.terminals)
        .map(|t| t.reference.as_str())
        .collect();
    refs.sort_unstable();
    assert_eq!(refs, ["R1", "R101"]);
    let occurrences: std::collections::HashSet<_> = p
        .netlist
        .nets
        .iter()
        .flat_map(|n| &n.terminals)
        .map(|t| t.occurrence.clone())
        .collect();
    assert_eq!(occurrences.len(), 2, "two placements are two terminals");
}

#[test]
fn placements_without_entries_still_report_the_shared_reference() {
    let (root, map) = twice_placed(false);
    let p = stitch(&root, "root", &map);
    assert_eq!(
        shared_reference_issues(&p),
        [&StitchIssue::SharedReferenceAcrossInstances {
            filename: "child.sch".to_string(),
            reference: "R1".to_string(),
        }]
    );
}

#[test]
fn an_entry_for_only_one_placement_leaves_the_other_on_the_fallback() {
    let (root, mut map) = twice_placed(true);
    map.get_mut("child.sch").unwrap().symbols[0].instances = vec![entry(&[id(2)], "R101")];
    let p = stitch(&root, "root", &map);
    assert!(shared_reference_issues(&p).is_empty(), "{:?}", p.issues);
    let mut refs: Vec<&str> = p
        .netlist
        .nets
        .iter()
        .flat_map(|n| &n.terminals)
        .map(|t| t.reference.as_str())
        .collect();
    refs.sort_unstable();
    assert_eq!(refs, ["R1", "R101"]);
}

fn graph_of<'a>(
    sheets: &'a HashMap<SheetKey, SchematicSheet>,
    resolved: &'a HashMap<SheetKey, HashMap<String, SheetKey>>,
    roots: &'a [ProjectRoot],
) -> ProjectGraph<'a> {
    ProjectGraph {
        sheets,
        resolved,
        roots,
    }
}

#[test]
fn the_occurrence_list_and_the_load_note_read_the_same_walk() {
    let (root, map) = twice_placed(true);
    let mut sheets: HashMap<SheetKey, SchematicSheet> = map
        .iter()
        .map(|(k, v)| (SheetKey::new(k.clone()), v.clone()))
        .collect();
    sheets.insert(SheetKey::new("root"), root.clone());
    let mut resolved: HashMap<SheetKey, HashMap<String, SheetKey>> = HashMap::new();
    resolved.insert(
        SheetKey::new("root"),
        HashMap::from([("child.sch".to_string(), SheetKey::new("child.sch"))]),
    );
    let roots = [ProjectRoot {
        key: SheetKey::new("root"),
    }];
    let graph = graph_of(&sheets, &resolved, &roots);

    let occs = project_occurrences(&graph);
    let shape: Vec<(String, String, String)> = occs
        .iter()
        .map(|o| {
            (
                o.key.as_str().to_string(),
                o.instance_path.to_string(),
                o.name_chain.join("/"),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            ("root".to_string(), "/".to_string(), String::new()),
            (
                "child.sch".to_string(),
                format!("/{}", id(1)),
                "A".to_string()
            ),
            (
                "child.sch".to_string(),
                format!("/{}", id(2)),
                "B".to_string()
            ),
        ]
    );
    // Only B's entry changes a designator (R101); A's entry mirrors R1, and
    // the stale and the malformed entry match no placement.
    assert_eq!(instance_entry_designators(&graph), ["R101"]);
}
