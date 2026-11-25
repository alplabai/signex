//! Epic #2326 slice 6.P5b (ADR-0006 slice 8, "Every query answers through
//! `core.query` in a test-adapter test"): each query of the catalog asked
//! through `TestAdapter::query` on an open project, its answer read for
//! content. The board queries read the corpus board
//! `tracks-outer-and-inner-layers.snxpcb`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_app::test_adapter::{Origin, Outcome, TestAdapter};
use signex_types::net_class::NetClass;
use signex_types::project::{DrcRuleSet, ProjectData, ProjectRules, SheetEntry};
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};
use uuid::Uuid;

/// Nets VCC and GND; two VCC tracks and one GND track, no via.
const BOARD: &str =
    include_str!("../../../signex-types/tests/corpus/pcb/tracks-outer-and-inner-layers.snxpcb");
/// The first VCC track of [`BOARD`].
const TRACK: &str = "0192a8c0-0000-7000-8000-000000003001";

const SHEET: Uuid = Uuid::from_u128(0x100);
const R1: Uuid = Uuid::from_u128(0x101);
const WIRE: Uuid = Uuid::from_u128(0x102);
const LABEL: Uuid = Uuid::from_u128(0x103);

/// Two rules on, one off, no syntax error. The board's 0.25 mm VCC tracks
/// break `vcc_w`.
const RULES: &str = "rule vcc_a on clearance when net == \"VCC\" require clearance >= 0.3mm then error\ndisabled rule vcc_b on clearance when net == \"VCC\" require clearance >= 0.4mm then warning\nrule vcc_w on track when net == \"VCC\" require width >= 1mm then error\n";
/// [`RULES`] with vcc_b on: two equally specific rules ask different
/// clearances of VCC.
const RULES_CONTESTED: &str = "rule vcc_a on clearance when net == \"VCC\" require clearance >= 0.3mm then error\nrule vcc_b on clearance when net == \"VCC\" require clearance >= 0.4mm then warning\n";

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: format!("N{number}"),
            number: number.into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// R1 (10k, two pins) at (50, 50), placed from no library row, and a wire
/// named GND by a label.
fn sheet_text() -> String {
    let mut body: LibSymbol =
        serde_json::from_value(json!({ "id": "Device:R" })).expect("lib symbol");
    body.pins = vec![lib_pin("1", -2.54), lib_pin("2", 2.54)];
    let mut sheet = SchematicSheet {
        uuid: SHEET,
        paper_size: "A4".into(),
        symbols: vec![Symbol {
            uuid: R1,
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            value: "10k".into(),
            position: Point::new(50.0, 50.0),
            unit: 1,
            ..Default::default()
        }],
        wires: vec![Wire {
            uuid: WIRE,
            start: Point::new(0.0, 0.0),
            end: Point::new(10.16, 0.0),
            ..Default::default()
        }],
        labels: vec![Label {
            uuid: LABEL,
            ..Label::new("GND", Point::new(0.0, 0.0), LabelType::Net)
        }],
        ..Default::default()
    };
    sheet.lib_symbols.insert("Device:R".into(), body);
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode the sheet")
}

/// The project: one sheet, the stored `rules`, net class Power (VCC), one
/// parts requirement and, when `board` is given, that board.
fn project_data(dir: &Path, board: Option<&str>, rules: &str) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("top.snxsch".into()),
        pcb_file: board.map(Into::into),
        sheets: vec![SheetEntry {
            name: "top".into(),
            filename: "top.snxsch".into(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        }],
        variant_definitions: Vec::new(),
        active_variant: None,
        variant_settings: Default::default(),
        libraries: Vec::new(),
        enable_git: false,
        net_classes: vec![NetClass {
            name: "Power".into(),
            nets: vec!["VCC".into()],
            ..NetClass::default()
        }],
        rules: Some(ProjectRules {
            drc: DrcRuleSet {
                dsl: Some(rules.into()),
                ..DrcRuleSet::default()
            },
            ..ProjectRules::default()
        }),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: serde_json::from_value(json!([{ "id": "parts", "kind": "parts_state" }]))
            .expect("a requirements list"),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// The open project and the paths of its project file, its sheet and, with
/// `with_board`, its board, each opened from the tree.
struct Opened {
    project: PathBuf,
    sheet: PathBuf,
    board: PathBuf,
}

fn open_project(adapter: &mut TestAdapter, with_board: bool, rules: &str) -> Opened {
    let dir = adapter.root().join("P");
    std::fs::create_dir_all(&dir).unwrap();
    let sheet = dir.join("top.snxsch");
    std::fs::write(&sheet, sheet_text()).unwrap();
    let board = dir.join("board.snxpcb");
    if with_board {
        std::fs::write(&board, BOARD).unwrap();
    }
    let project = dir.join("P.snxprj");
    let data = project_data(&dir, with_board.then_some("board.snxpcb"), rules);
    signex_types::project::write_project(&project, &data).unwrap();
    let open = format!("path={}", project.display());
    let (outcome, pending) = adapter.execute("open_document", &[&open], Origin::Person);
    assert_eq!(outcome, Outcome::Ran, "the project opens");
    adapter.deliver(pending).unwrap();
    let mut opened = vec![&sheet];
    if with_board {
        opened.push(&board);
    }
    for path in opened {
        let read = adapter.open_from_tree(path).expect("the tree opens it");
        adapter.deliver(read).unwrap();
    }
    Opened {
        project,
        sheet,
        board,
    }
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

fn ask(adapter: &mut TestAdapter, id: &str, args: &[&str]) -> Value {
    adapter
        .query(id, args)
        .unwrap_or_else(|refused| panic!("{id} did not answer: {refused:?}"))
}

#[test]
fn the_document_queries_answer_through_the_adapter() {
    let mut adapter = TestAdapter::new();
    let opened = open_project(&mut adapter, false, RULES);
    let sheet = document(&opened.sheet);

    let listed = ask(&mut adapter, "list_documents", &[]);
    let paths: Vec<&str> = listed["documents"]
        .as_array()
        .expect("documents")
        .iter()
        .filter_map(|doc| doc["path"].as_str())
        .collect();
    assert!(
        paths.contains(&opened.sheet.to_string_lossy().as_ref()),
        "{listed}"
    );

    let info = ask(&mut adapter, "document_info", &[&sheet]);
    assert_eq!(info["id"], SHEET.to_string(), "{info}");
    assert_eq!(info["dirty"], false, "{info}");

    let read = ask(&mut adapter, "read_document", &[&sheet]);
    assert!(read.to_string().contains(&R1.to_string()), "{read}");

    let found = ask(&mut adapter, "resolve", &[&format!("id={R1}")]);
    assert_eq!(found["what"], "element", "{found}");
    assert_eq!(found["element"]["kind"], "symbol", "{found}");
    assert_eq!(found["element"]["designator"], "R1", "{found}");
    assert_eq!(found["element"]["value"], "10k", "{found}");

    let on_gnd = ask(&mut adapter, "find_items", &["net=GND", "kind=wire"]);
    assert_eq!(on_gnd["count"], 1, "{on_gnd}");
    assert_eq!(on_gnd["elements"][0]["id"], WIRE.to_string(), "{on_gnd}");

    let pins = ask(&mut adapter, "query_pins", &[&sheet, "designator=R1"]);
    let listed: Vec<(&str, &str)> = pins["pins"]
        .as_array()
        .expect("pins")
        .iter()
        .map(|pin| {
            (
                pin["number"].as_str().unwrap(),
                pin["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(listed, [("1", "N1"), ("2", "N2")], "{pins}");
}

#[test]
fn the_project_checks_answer_through_the_adapter_once_their_run_lands() {
    let mut adapter = TestAdapter::new();
    let opened = open_project(&mut adapter, false, RULES);

    let erc = ask(&mut adapter, "check_erc", &[&document(&opened.sheet)]);
    assert_eq!(erc["check"], "erc", "{erc}");
    assert_eq!(erc["run"], 1, "{erc}");
    assert!(
        erc["findings"]
            .as_array()
            .is_some_and(|findings| !findings.is_empty()),
        "R1's pins are unconnected: {erc}"
    );
    let mut unused: Vec<(String, &str)> = erc["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == "builtin::unused_pin")
        .map(|f| {
            (
                f["items"][0].to_string(),
                f["evidence"]["pin"].as_str().unwrap(),
            )
        })
        .collect();
    unused.sort_unstable();
    let r1 = json!(R1.to_string()).to_string();
    assert_eq!(
        unused,
        [(r1.clone(), "1@-2540,0"), (r1, "2@2540,0")],
        "both of R1's pins: {erc}"
    );

    let requirements = ask(
        &mut adapter,
        "check_requirements",
        &[&document(&opened.project)],
    );
    assert_eq!(requirements["check"], "requirements", "{requirements}");
    let messages: Vec<&str> = requirements["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|f| f["message"].as_str())
        .collect();
    assert_eq!(
        messages,
        ["R1 was not placed from a library row"],
        "{requirements}"
    );
}

#[test]
fn the_project_and_rule_reads_answer_through_the_adapter() {
    let mut adapter = TestAdapter::new();
    let opened = open_project(&mut adapter, false, RULES);
    let project = document(&opened.project);

    let settings = ask(&mut adapter, "project_settings", &[&project]);
    assert_eq!(
        settings["project"],
        opened.project.display().to_string(),
        "{settings}"
    );
    assert_eq!(settings["sheets"][0]["file"], "top.snxsch", "{settings}");
    assert_eq!(settings["sheets"][0]["page"], 1, "{settings}");

    let list = ask(&mut adapter, "rules_list", &[&project]);
    assert_eq!(list["stored_in"], "project", "{list}");
    assert_eq!(list["syntax_errors"], json!([]), "{list}");
    let rules = list["rules"].as_array().expect("rules");
    let ids: Vec<&str> = rules.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["user::vcc_a", "user::vcc_b", "user::vcc_w"], "{list}");
    let enabled = ask(&mut adapter, "rules_list", &[&project, "enabled=true"]);
    let names: Vec<&str> = enabled["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["vcc_a", "vcc_w"], "{enabled}");

    let got = ask(&mut adapter, "rules_get", &[&project, "id=vcc_b"]);
    assert_eq!(got["name"], "vcc_b", "{got}");
    assert_eq!(got["enabled"], false, "{got}");
    assert_eq!(got["limits"]["min_clearance"], "0.4mm", "{got}");

    let matrix = ask(&mut adapter, "rules_matrix_get", &[&project]);
    assert_eq!(matrix["classes"], json!(["Power"]), "{matrix}");
    assert_eq!(matrix["cells"], json!([]), "{matrix}");

    let diff = ask(
        &mut adapter,
        "rules_preset_diff",
        &[&project, "preset=advanced"],
    );
    assert_eq!(diff["mode"], "merge", "{diff}");
    assert_eq!(diff["removed"], 0, "{diff}");
    // The tier's fab_* rules are new to a project that writes only vcc_*.
    assert_eq!(diff["added"], 7, "{diff}");
    assert_eq!(diff["changed"], 0, "{diff}");
    assert_eq!(diff["changes"][0]["rule"], "fab_clearance", "{diff}");
}

#[test]
fn the_board_queries_answer_through_the_adapter() {
    let mut adapter = TestAdapter::new();
    let opened = open_project(&mut adapter, true, RULES);
    let board = document(&opened.board);
    let project = document(&opened.project);

    let span = ask(
        &mut adapter,
        "measure_board",
        &[&board, "start=0,0", "end=3,4"],
    );
    assert_eq!(span["kind"], "point_to_point", "{span}");
    assert_eq!(span["distance_mm"], 5.0, "{span}");

    let nets = ask(&mut adapter, "query_board_nets", &[&board, "net=VCC"]);
    assert_eq!(nets["count"], 1, "{nets}");
    assert_eq!(nets["nets"][0]["name"], "VCC", "{nets}");
    assert_eq!(nets["nets"][0]["tracks"], 2, "{nets}");
    assert_eq!(nets["nets"][0]["vias"], 0, "{nets}");

    let similar = ask(
        &mut adapter,
        "find_similar_items",
        &[&board, &format!("id={TRACK}")],
    );
    assert_eq!(similar["count"], 3, "every track: {similar}");
    let on_net = ask(
        &mut adapter,
        "find_similar_items",
        &[&board, &format!("id={TRACK}"), "fields=kind,net_name"],
    );
    assert_eq!(on_net["count"], 2, "the VCC tracks: {on_net}");

    let panel = ask(&mut adapter, "board_panel", &[&board]);
    assert_eq!(panel["panel"], Value::Null, "{panel}");
    assert_eq!(
        panel["document"],
        opened.board.display().to_string(),
        "{panel}"
    );
    assert_eq!(panel["revision"], 1, "{panel}");

    let drc = ask(&mut adapter, "check_drc", &[&board]);
    assert_eq!(drc["check"], "drc", "{drc}");
    assert_eq!(drc["run"], 1, "{drc}");
    // Findings are ordered by key, which hashes the path: sort them.
    let mut broken: Vec<(&str, &str)> = drc["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| (f["rule"].as_str().unwrap(), f["items"][0].as_str().unwrap()))
        .collect();
    broken.sort_unstable();
    assert_eq!(
        broken,
        [
            ("user::vcc_w", TRACK),
            ("user::vcc_w", "0192a8c0-0000-7000-8000-000000003003"),
        ],
        "{drc}"
    );

    let effective = ask(&mut adapter, "rules_effective", &[&project, "net=VCC"]);
    assert_eq!(effective["source"]["rule"], "vcc_a", "{effective}");
    assert_eq!(effective["value_mm"], 0.3, "{effective}");

    // vcc_b is off, so the stricter rule on the same scope shadows nothing.
    let flags = ask(&mut adapter, "rules_conflicts", &[&project]);
    assert_eq!(flags, json!({ "flags": [] }), "{flags}");

    let got = ask(&mut adapter, "rules_get", &[&project, "id=vcc_w"]);
    assert_eq!(got["scope_objects"], 2, "the VCC tracks: {got}");

    // `rules_explain` reads the last Run DRC of the board, the open tab.
    let (outcome, pending) = adapter.execute("run_drc", &[], Origin::Person);
    assert_eq!(outcome, Outcome::Ran, "Run DRC starts");
    adapter.deliver(pending).unwrap();
    let why = ask(&mut adapter, "rules_explain", &[&project, "violation=0"]);
    assert_eq!(why["rule"], "user::vcc_w", "{why}");
    assert_eq!(why["kind"], "min_track_width", "{why}");
    assert_eq!(why["required_mm"], 1.0, "{why}");
    assert_eq!(why["sides"][0]["net"], "VCC", "{why}");
}

#[test]
fn rules_conflicts_names_the_contested_rules_through_the_adapter() {
    let mut adapter = TestAdapter::new();
    let opened = open_project(&mut adapter, true, RULES_CONTESTED);

    let flags = ask(
        &mut adapter,
        "rules_conflicts",
        &[&document(&opened.project)],
    );
    assert_eq!(
        flags,
        json!({ "flags": [
            { "rule": "vcc_a", "flag": "shadowed", "by": "vcc_b" },
            { "rule": "vcc_a", "flag": "conflict", "with": "vcc_b" },
            { "rule": "vcc_b", "flag": "conflict", "with": "vcc_a" },
        ] }),
        "the stricter vcc_b wins: {flags}"
    );
}
