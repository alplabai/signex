//! How the export diagnoses a listed page it could not put in the netlist.
//!
//! Split out of the parent module only to keep that file under the size cap.

use uuid::Uuid;

use super::{open_with, page_paths, sheet_with_net, workspace};

#[test]
fn a_listed_page_with_no_file_is_not_diagnosed_as_a_graph_problem() {
    // A page that does not exist on disk is not "nothing reaches it from the
    // root by a child-sheet reference" — that sends the user to inspect a
    // hierarchy that is fine. It is also dropped from the exported page set,
    // so the PDF comes out a page short; saying so is the only warning the
    // user gets for that.
    let dir = std::env::temp_dir().join(format!("signex-export-nofile-{}", Uuid::new_v4()));
    let dir_str = dir.to_string_lossy().to_string();
    let mut ds = workspace(&dir_str, &["a.snxsch", "b.snxsch"]);
    let a = dir.join("a.snxsch");
    open_with(&mut ds, &a, sheet_with_net("R_A", "NET_A", &[]));
    ds.active_path = Some(a.clone());

    let (ctx, issues) = crate::app::handlers::menu::export::build_export_scope(&ds).expect("ctx");
    let messages = issues.messages().join("\n");

    assert!(
        messages.contains("no file exists at that path"),
        "an absent page must be diagnosed as absent: {messages}"
    );
    assert!(
        !messages.contains("nothing reaches it"),
        "…and not as a child-sheet-graph problem: {messages}"
    );
    assert_eq!(
        page_paths(&ctx),
        vec![a],
        "precondition: the absent page really is dropped from the exported pages"
    );
    assert!(
        issues.netlist_is_incomplete(),
        "a listed page that is not in the netlist still refuses the .net"
    );
}
