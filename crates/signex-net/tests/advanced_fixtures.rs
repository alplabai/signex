//! Advanced Schematic exit criteria (`docs/ROADMAP.md`, "Advanced Schematic
//! (P1)"): the netlists of the projects on disk in
//! `signex-types/tests/corpus/advanced/`, read with the real parser and
//! stitched as the app stitches them. A nested harness connects through its
//! entries and matches its golden netlist; the classes a Net Blanket and a
//! Net Parameters directive give appear in the netlist. Goldens are in
//! `tests/golden/`; `UPDATE_GOLDEN=1` rewrites them.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use signex_net::{
    ProjectGraph, ProjectNetlist, ProjectRoot, SheetKey, build_netlist, build_project_netlist,
    diff_pairs_with_classes,
};
use signex_types::format::SnxSchematic;
use signex_types::net::{Net, Netlist};
use signex_types::project::parse_project;
use signex_types::schematic::SchematicSheet;

fn fixture_dir(project: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/advanced")
        .join(project)
}

fn read_sheet(project: &str, file: &str) -> SchematicSheet {
    let path = fixture_dir(project).join(file);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (the fixture is generated: SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration advanced_fixtures)",
            path.display()
        )
    });
    SnxSchematic::parse(&text)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .sheet
}

/// The project's sheets keyed by file name, stitched from `root`, each
/// child resolved by the file name its sheet symbol gives.
fn stitch(project: &str, root: &str, files: &[&str]) -> ProjectNetlist {
    let sheets: HashMap<SheetKey, SchematicSheet> = files
        .iter()
        .map(|file| (SheetKey::new(*file), read_sheet(project, file)))
        .collect();
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> = sheets
        .iter()
        .map(|(key, sheet)| {
            let children = sheet
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), SheetKey::new(cs.filename.clone())))
                .collect();
            (key.clone(), children)
        })
        .collect();
    let roots = [ProjectRoot {
        key: SheetKey::new(root),
    }];
    build_project_netlist(&ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    })
}

fn pins(net: &Net) -> Vec<String> {
    let mut pins: Vec<String> = net
        .terminals
        .iter()
        .map(|t| format!("{}.{}", t.reference, t.pin))
        .collect();
    pins.sort();
    pins
}

/// One line per net, sorted: `name`, then its terminals as `reference.pin`.
/// The in-memory harness fixture's golden has this shape.
fn netlist_text(netlist: &Netlist) -> String {
    let mut lines: Vec<String> = netlist
        .nets
        .iter()
        .map(|n| format!("{}\t{}", n.name, pins(n).join(" ")))
        .collect();
    lines.sort();
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// [`netlist_text`] with each net's class between its name and its pins
/// (`-` for none).
fn classed_netlist_text(netlist: &Netlist) -> String {
    let mut lines: Vec<String> = netlist
        .nets
        .iter()
        .map(|n| {
            let class = n.class.as_ref().map_or("-", |c| c.0.as_str());
            format!("{}\t{class}\t{}", n.name, pins(n).join(" "))
        })
        .collect();
    lines.sort();
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn check_golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some_and(|v| v == "1") {
        fs::create_dir_all(path.parent().expect("a folder")).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (run with UPDATE_GOLDEN=1 to write it)",
            path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "{name} differs from its golden (an intentional change: rerun with UPDATE_GOLDEN=1 and review the diff)"
    );
}

/// The net whose terminals include `pin` (`U1.2`).
fn net_on<'n>(netlist: &'n Netlist, pin: &str) -> &'n Net {
    netlist
        .nets
        .iter()
        .find(|n| pins(n).iter().any(|p| p == pin))
        .unwrap_or_else(|| panic!("no net holds {pin}"))
}

const NESTED: [&str; 3] = ["mcu.snxsch", "sensor.snxsch", "connectors.snxsch"];

#[test]
fn the_nested_harness_fixture_matches_its_golden_netlist() {
    let stitched = stitch("nested-harness", "mcu.snxsch", &NESTED);
    assert!(stitched.issues.is_empty(), "{:?}", stitched.issues);
    let text = netlist_text(&stitched.netlist);
    check_golden("nested_harness.netlist.tsv", &text);
    assert_eq!(
        text,
        include_str!("../src/testdata/harness_netlist.golden.tsv"),
        "the fixture on disk connects as the in-memory harness fixture does"
    );
}

/// Every entry of the nested `UART` harness reaches J2 through `DEBUG`.
#[test]
fn the_nested_uart_entries_connect_through_the_debug_harness() {
    let netlist = stitch("nested-harness", "mcu.snxsch", &NESTED).netlist;
    assert_eq!(net_on(&netlist, "U3.7").name, "DEBUG.UART.TX");
    assert_eq!(pins(net_on(&netlist, "U3.7")), ["J2.3", "U3.7"]);
    assert_eq!(pins(net_on(&netlist, "U3.8")), ["J2.4", "U3.8"]);
}

#[test]
#[ignore = "needs batch A's harness carry through a sheet with no connector (#2657); the integrator runs it after that merge"]
fn the_nested_harness_carried_through_a_sheet_matches_the_same_golden_netlist() {
    let stitched = stitch(
        "nested-harness-carry",
        "mcu.snxsch",
        &[
            "mcu.snxsch",
            "sensor.snxsch",
            "bridge.snxsch",
            "connectors.snxsch",
        ],
    );
    assert!(stitched.issues.is_empty(), "{:?}", stitched.issues);
    assert_eq!(
        netlist_text(&stitched.netlist),
        include_str!("golden/nested_harness.netlist.tsv")
    );
}

const CLASSES: &str = "net-class-directives";

#[test]
fn the_classes_the_directives_give_appear_in_the_project_netlist() {
    let netlist = stitch(CLASSES, "main.snxsch", &["main.snxsch"]).netlist;
    check_golden(
        "net_class_directives.netlist.tsv",
        &classed_netlist_text(&netlist),
    );
    let class = |pin: &str| net_on(&netlist, pin).class.as_ref().map(|c| c.0.clone());
    assert_eq!(class("U1.1").as_deref(), Some("POWER"), "the Net Blanket");
    assert_eq!(class("U1.2").as_deref(), Some("HS"), "the Net Parameters");
    assert_eq!(class("U1.5"), None, "SIG has no directive");
}

#[test]
fn a_sheet_netlist_carries_the_same_classes_as_the_project_netlist() {
    let sheet = read_sheet(CLASSES, "main.snxsch");
    let alone = build_netlist(&sheet);
    let project = stitch(CLASSES, "main.snxsch", &["main.snxsch"]).netlist;
    for pin in ["U1.1", "U1.2", "U1.3", "U1.4", "U1.5"] {
        assert_eq!(
            net_on(&alone, pin).class,
            net_on(&project, pin).class,
            "{pin}"
        );
    }
}

#[test]
fn the_net_parameters_diff_pair_class_pairs_usb_p_with_usb_n() {
    let sheet = read_sheet(CLASSES, "main.snxsch");
    let project = parse_project(&fixture_dir(CLASSES).join(format!("{CLASSES}.snxprj")))
        .expect("the project reads");
    let classes = &project.schematic_design.diff_pair_classes;
    assert_eq!(classes.len(), 1);
    let pairs = diff_pairs_with_classes(&sheet, &build_netlist(&sheet), classes);
    assert_eq!(pairs.len(), 1, "{pairs:?}");
    let pair = &pairs[0];
    assert_eq!(pair.name, "USB");
    assert_eq!(pair.positive.as_deref(), Some("USB_P"));
    assert_eq!(pair.negative.as_deref(), Some("USB_N"));
    assert_eq!(pair.class.as_deref(), Some("USB"));
}
