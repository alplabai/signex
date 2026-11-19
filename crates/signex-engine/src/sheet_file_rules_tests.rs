//! The sheet-symbol file rules (`sheet_file_rules`): one rule set for the GUI
//! and the operand forms of Create Sheet Symbol From Sheet.

use std::collections::HashSet;

use crate::{SheetFilePlacement, SheetFileRefusal, SheetGraph, SheetGraphNode, check_sheet_file};

fn read(name: &str, children: &[&str], in_project: bool) -> SheetGraphNode {
    SheetGraphNode::Read {
        name: name.to_string(),
        children: children.iter().map(|c| c.to_string()).collect(),
        reaching_names: false,
        in_project,
    }
}

fn placement<'a>(
    file: &'a str,
    above: &'a HashSet<String>,
    graph: &'a SheetGraph,
    pages: Option<&'a HashSet<String>>,
) -> SheetFilePlacement<'a> {
    SheetFilePlacement {
        edited: "a",
        file,
        escapes_project: false,
        above,
        graph,
        project_pages: pages,
        allow_merge: false,
    }
}

/// A is edited, B places C and C places A; neither B nor C is above A in a
/// hierarchy anyone holds. Placing B on A would close A→B→C→A.
#[test]
fn a_loop_two_levels_down_is_refused() {
    let mut graph = SheetGraph::default();
    graph.insert("a".into(), read("a.snxsch", &[], true));
    graph.insert("b".into(), read("b.snxsch", &["c"], true));
    graph.insert("c".into(), read("c.snxsch", &["a"], true));
    let above = HashSet::new();
    let refused = check_sheet_file(&placement("b", &above, &graph, None));
    assert_eq!(refused, Err(SheetFileRefusal::PlacesEditedSheet));
    // allow_merge never lets a loop through.
    let merged = SheetFilePlacement {
        allow_merge: true,
        ..placement("b", &above, &graph, None)
    };
    assert_eq!(
        check_sheet_file(&merged),
        Err(SheetFileRefusal::PlacesEditedSheet)
    );
}

#[test]
fn the_sheet_itself_an_ancestor_and_an_escape_are_refused_in_that_order() {
    let graph = SheetGraph::default();
    let above: HashSet<String> = HashSet::from(["top".to_string()]);
    assert_eq!(
        check_sheet_file(&placement("a", &above, &graph, None)),
        Err(SheetFileRefusal::IsEditedSheet)
    );
    assert_eq!(
        check_sheet_file(&placement("top", &above, &graph, None)),
        Err(SheetFileRefusal::IsAncestor)
    );
    let escaping = SheetFilePlacement {
        escapes_project: true,
        ..placement("a", &above, &graph, None)
    };
    assert_eq!(
        check_sheet_file(&escaping),
        Err(SheetFileRefusal::EscapesProject)
    );
}

#[test]
fn an_unreadable_subtree_is_refused_unless_merged() {
    let mut graph = SheetGraph::default();
    graph.insert("b".into(), read("b.snxsch", &["broken"], true));
    graph.insert(
        "broken".into(),
        SheetGraphNode::Unreadable {
            name: "broken.snxsch".into(),
            why: "bad header".into(),
        },
    );
    let above = HashSet::new();
    let refused = check_sheet_file(&placement("b", &above, &graph, None));
    let Err(SheetFileRefusal::Unreadable(why)) = &refused else {
        panic!("refused for its unreadable child: {refused:?}");
    };
    assert_eq!(
        why,
        &["broken.snxsch is unreadable (bad header)".to_string()]
    );
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("broken.snxsch is unreadable")
    );
    let merged = SheetFilePlacement {
        allow_merge: true,
        ..placement("b", &above, &graph, None)
    };
    let facts = check_sheet_file(&merged).expect("placed with allow_merge");
    assert_eq!(facts.unreadable.len(), 1);
}

#[test]
fn a_page_or_a_file_placed_on_another_project_sheet_is_placed_once() {
    let mut graph = SheetGraph::default();
    graph.insert("a".into(), read("a.snxsch", &["b"], true));
    graph.insert("z".into(), read("z.snxsch", &["b"], true));
    graph.insert("m".into(), read("m.snxsch", &["b"], true));
    graph.insert("loose".into(), read("loose.snxsch", &["c"], false));
    graph.insert("b".into(), read("b.snxsch", &[], true));
    graph.insert("c".into(), read("c.snxsch", &[], true));
    let above = HashSet::new();
    let pages: HashSet<String> = HashSet::from(["p".to_string()]);
    assert_eq!(
        check_sheet_file(&placement("p", &above, &graph, Some(&pages))),
        Err(SheetFileRefusal::ProjectPage)
    );
    // The first placing sheet by name, whatever the map order.
    assert_eq!(
        check_sheet_file(&placement("b", &above, &graph, Some(&pages))),
        Err(SheetFileRefusal::PlacedOn("m.snxsch".into()))
    );
    // A sheet outside the project does not count, and a loose edit has no
    // placed-once rule.
    assert!(check_sheet_file(&placement("c", &above, &graph, Some(&pages))).is_ok());
    assert!(check_sheet_file(&placement("b", &above, &graph, None)).is_ok());
}
