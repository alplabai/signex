//! The project-wide `NetMultipleNames` pass of the ERC job (#1061): which
//! per-sheet findings it replaces, where its findings are filed, how the
//! severity overrides apply to them, and how `run_erc_job` wires it up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_erc::{RuleId, RuleKind, Severity, Violation};
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, Point, SchematicSheet, SheetPin, Wire,
};
use uuid::Uuid;

use super::{ErcRunInputs, ViolationsByPath, add_project_net_names, run_erc_job};
use crate::app::project_sheets::project_graph;
use crate::app::state::scope::CaseFold;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn label(text: &str, at: Point, label_type: LabelType) -> Label {
    Label {
        ..Label::new(text, at, label_type)
    }
}

/// A pin-less wire `(0,0)-(10,0)` carrying the given labels at its two ends.
fn named_wire(left: (&str, LabelType), right: (&str, LabelType)) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(label(left.0, pt(0.0, 0.0), left.1));
    sheet.labels.push(label(right.0, pt(10.0, 0.0), right.1));
    sheet
}

fn child_with_pin(filename: &str, pin: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::nil(),
        name: "child".to_string(),
        filename: filename.to_string(),
        position: pt(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::nil(),
            name: pin.to_string(),
            direction: String::new(),
            position: pt(0.0, 0.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    }
}

fn finding(rule: RuleKind, severity: Severity, message: &str) -> Violation {
    Violation::new(RuleId::builtin(rule), rule, severity, message, pt(0.0, 0.0))
}

fn multi_names(list: &[Violation]) -> Vec<&Violation> {
    list.iter()
        .filter(|v| v.builtin_kind() == Some(RuleKind::NetMultipleNames))
        .collect()
}

struct Fixture {
    dir: PathBuf,
    graph: crate::app::project_sheets::AssembledGraph,
    by_path: ViolationsByPath,
}

impl Fixture {
    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn run(&mut self, roots: &[&str], overrides: &HashMap<RuleKind, Severity>) {
        let roots: Vec<signex_net::ProjectRoot> = roots
            .iter()
            .map(|r| signex_net::ProjectRoot {
                key: signex_net::SheetKey::new(*r),
            })
            .collect();
        add_project_net_names(
            &mut self.by_path,
            &self.graph,
            &roots,
            Some(&self.dir),
            CaseFold::host_default(),
            overrides,
            &Default::default(),
        );
    }
}

/// `root.snxsch` (Global `G`, sheet pin `SDA` to `child.snxsch`), a child that
/// calls its side Hierarchical `SDA`, and `lone.snxsch`, which the root never
/// reaches. Each sheet starts with a per-sheet `NetMultipleNames` finding and
/// an Info finding of another rule.
fn fixture() -> Fixture {
    let dir = Path::new("/proj").to_path_buf();
    let mut root = named_wire(("G", LabelType::Global), ("G", LabelType::Global));
    root.child_sheets
        .push(child_with_pin("child.snxsch", "SDA"));
    let child = named_wire(
        ("SDA", LabelType::Hierarchical),
        ("SDA", LabelType::Hierarchical),
    );
    let lone = named_wire(
        ("A", LabelType::Hierarchical),
        ("B", LabelType::Hierarchical),
    );
    let sheets: HashMap<PathBuf, SchematicSheet> = [
        (dir.join("root.snxsch"), root),
        (dir.join("child.snxsch"), child),
        (dir.join("lone.snxsch"), lone),
    ]
    .into_iter()
    .collect();
    let graph = project_graph(&sheets, Some(&dir), Some(&dir.join("root.snxsch")));
    let by_path = sheets
        .keys()
        .map(|p| {
            (
                p.clone(),
                vec![
                    finding(RuleKind::UnusedPin, Severity::Info, "other"),
                    finding(RuleKind::NetMultipleNames, Severity::Warning, "per-sheet"),
                ],
            )
        })
        .collect();
    Fixture {
        dir,
        graph,
        by_path,
    }
}

#[test]
fn a_sheet_outside_the_roots_keeps_its_own_findings() {
    let mut f = fixture();
    f.run(&["root.snxsch"], &HashMap::new());

    let lone = &f.by_path[&f.path("lone.snxsch")];
    let kept = multi_names(lone);
    assert_eq!(kept.len(), 1, "{lone:?}");
    assert_eq!(kept[0].message, "per-sheet");
}

#[test]
fn a_reached_sheet_trades_its_findings_for_the_project_ones_filed_under_the_winner() {
    let mut f = fixture();
    // Through sheet pin `SDA` the net already has two names, the root's `G`
    // and the child's `SDA`; `H` gives the root wire a second name of its own.
    let key = signex_net::SheetKey::new("root.snxsch");
    f.graph.sheets.get_mut(&key).unwrap().labels.push(label(
        "H",
        pt(5.0, 0.0),
        LabelType::Hierarchical,
    ));
    f.run(&["root.snxsch"], &HashMap::new());

    let root = multi_names(&f.by_path[&f.path("root.snxsch")]);
    assert_eq!(root.len(), 1, "{root:?}");
    assert!(root[0].message.starts_with("Net has multiple names"));
    // The fixture's wires carry no part, so no net is exported.
    assert!(root[0].message.contains("it would be named 'G'"));
    assert!(
        multi_names(&f.by_path[&f.path("child.snxsch")]).is_empty(),
        "the child's per-sheet finding is replaced, not duplicated"
    );
}

#[test]
fn the_appended_findings_are_sorted_in() {
    let mut f = fixture();
    f.run(&["root.snxsch"], &HashMap::new());

    // The appended finding sorts ahead of the UnusedPin finding that was first.
    let root = &f.by_path[&f.path("root.snxsch")];
    assert_eq!(root[0].builtin_kind(), Some(RuleKind::NetMultipleNames));
    assert_eq!(root[1].builtin_kind(), Some(RuleKind::UnusedPin));
}

#[test]
fn a_severity_override_applies_to_project_findings() {
    let mut f = fixture();
    let off = HashMap::from([(RuleKind::NetMultipleNames, Severity::Off)]);
    f.run(&["root.snxsch"], &off);
    assert!(multi_names(&f.by_path[&f.path("root.snxsch")]).is_empty());

    let mut f = fixture();
    let error = HashMap::from([(RuleKind::NetMultipleNames, Severity::Error)]);
    f.run(&["root.snxsch"], &error);
    let root = &f.by_path[&f.path("root.snxsch")];
    assert!(
        multi_names(root)
            .iter()
            .all(|v| v.severity == Severity::Error)
    );
    assert_eq!(root[0].severity, Severity::Error, "errors sort first");
}

#[test]
fn no_roots_leaves_every_per_sheet_finding_alone() {
    let mut f = fixture();
    f.run(&[], &HashMap::new());
    for list in f.by_path.values() {
        assert_eq!(multi_names(list).len(), 1);
    }
}

#[test]
fn a_per_sheet_finding_no_emitted_project_finding_covers_is_kept() {
    // A No-ERC on the root's `G` leaves the root's part out; the child alone
    // has one name, so no project finding is emitted and nothing replaces the
    // per-sheet findings of either reached sheet.
    let mut f = fixture();
    let key = signex_net::SheetKey::new("root.snxsch");
    f.graph
        .sheets
        .get_mut(&key)
        .unwrap()
        .no_erc_directives
        .push(signex_types::schematic::NoErcDirective {
            uuid: Uuid::now_v7(),
            position: pt(0.0, 0.0),
            rules: Vec::new(),
        });
    f.run(&["root.snxsch"], &HashMap::new());

    for sheet in ["root.snxsch", "child.snxsch"] {
        let kept = multi_names(&f.by_path[&f.path(sheet)]);
        assert_eq!(kept.len(), 1, "{sheet}: {kept:?}");
        assert_eq!(kept[0].message, "per-sheet");
    }
}

/// `root.snxsch` (local `ROOT`, sheet pin `SDA` to `child.snxsch`); the child
/// names its side Hierarchical `SDA` and Global `BUS`, which wins; the open
/// tab `Child.snxsch` is a different file whose key folds onto the child's.
fn colliding_job() -> (ErcRunInputs, PathBuf, PathBuf, PathBuf) {
    let dir = Path::new("/proj").to_path_buf();
    let (root_path, child_path, loose_path) = (
        dir.join("root.snxsch"),
        dir.join("child.snxsch"),
        dir.join("Child.snxsch"),
    );
    let mut root = named_wire(("ROOT", LabelType::Net), ("ROOT", LabelType::Net));
    root.child_sheets
        .push(child_with_pin("child.snxsch", "SDA"));
    let child = named_wire(("SDA", LabelType::Hierarchical), ("BUS", LabelType::Global));
    let loose = named_wire(
        ("A", LabelType::Hierarchical),
        ("B", LabelType::Hierarchical),
    );
    let open: crate::app::project_sheets::OpenSheets = [
        (root_path.clone(), root),
        (child_path.clone(), child),
        (loose_path.clone(), loose),
    ]
    .into_iter()
    .collect();
    let inputs = ErcRunInputs {
        tab_paths: open.keys().cloned().collect(),
        open,
        active: None,
        pages: Vec::new(),
        root: Some(root_path.clone()),
        project_dir: Some(dir),
        overrides: HashMap::new(),
        rule_set: signex_output::ErcReportRuleSet {
            profile_name: None,
            sha256: String::new(),
        },
        config: signex_erc::ErcConfig::default(),
        net_waivers: Default::default(),
        violation_waivers: Vec::new(),
        channel_naming: Default::default(),
        fold: CaseFold::FOLD,
    };
    (inputs, root_path, child_path, loose_path)
}

/// A one-pin part `R1` at the origin, for a net the netlist exports.
fn with_one_pin_part(mut sheet: SchematicSheet) -> SchematicSheet {
    use signex_types::schematic::{LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Symbol};
    let lib: LibSymbol = serde_json::from_value(serde_json::json!({"id": "P"})).expect("lib");
    let pin = Pin {
        direction: PinDirection::Passive,
        shape_style: PinShapeStyle::Plain,
        position: pt(0.0, 0.0),
        rotation: 0.0,
        length: 0.0,
        name: String::new(),
        number: "1".to_string(),
        visible: true,
        name_visible: true,
        number_visible: true,
    };
    let pins = vec![LibPin {
        unit: 0,
        body_style: 1,
        pin,
    }];
    sheet
        .lib_symbols
        .insert("P".to_string(), LibSymbol { pins, ..lib });
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "P".to_string(),
        reference: "R1".to_string(),
        position: pt(0.0, 0.0),
        ..Default::default()
    });
    sheet
}

/// A root placing `child.snxsch` as `Repeat(CH, 1, 2)`; the child's one wire
/// has a part and the Hierarchical names `FB` and `SENSE`.
fn repeated_job() -> (ErcRunInputs, PathBuf) {
    let (mut inputs, ..) = colliding_job();
    let dir = Path::new("/proj");
    let (root_path, child_path) = (dir.join("root.snxsch"), dir.join("child.snxsch"));
    let mut placement = child_with_pin("child.snxsch", "UNUSED");
    placement.name = "Repeat(CH, 1, 2)".to_string();
    placement.uuid = Uuid::from_u128(0x2740);
    placement.pins.clear();
    let root = SchematicSheet {
        child_sheets: vec![placement],
        ..Default::default()
    };
    let child = with_one_pin_part(named_wire(
        ("FB", LabelType::Hierarchical),
        ("SENSE", LabelType::Hierarchical),
    ));
    let open: crate::app::project_sheets::OpenSheets =
        [(root_path.clone(), root), (child_path.clone(), child)]
            .into_iter()
            .collect();
    inputs.tab_paths = open.keys().cloned().collect();
    inputs.open = open;
    inputs.root = Some(root_path);
    (inputs, child_path)
}

/// #2740: the run reads the project's saved channel naming. The net count
/// does not depend on a name; it is pinned so the named stitch counts each
/// channel's net.
#[test]
fn the_job_reports_channel_nets_by_the_saved_naming() {
    let (mut inputs, child_path) = repeated_job();
    inputs.channel_naming = signex_types::project::ChannelNaming {
        net_format: "${Channel}_${Net}".to_string(),
        ..Default::default()
    }
    .into();

    let outcome = run_erc_job(inputs);

    let filed = multi_names(&outcome.by_path[&child_path]);
    assert_eq!(filed.len(), 1, "{filed:?}");
    assert!(
        filed[0].message.contains("the netlist uses 'CH1_FB'"),
        "{}",
        filed[0].message
    );
    assert_eq!(outcome.checked[2], 2, "one net per channel");
}

/// #2740: with the saved naming unreadable the run still checks the sheets,
/// says why, and names no exported channel net it cannot know.
#[test]
fn an_unreadable_naming_claims_no_exported_channel_net_name() {
    let (mut inputs, child_path) = repeated_job();
    inputs.channel_naming = crate::app::saved_channel_naming::SavedChannelNaming::unreadable();

    let outcome = run_erc_job(inputs);

    let filed = multi_names(&outcome.by_path[&child_path]);
    assert_eq!(filed.len(), 1, "{filed:?}");
    assert!(
        !filed[0].message.contains("the netlist uses"),
        "{}",
        filed[0].message
    );
    let banner = signex_i18n::I18n::en_us_ref()
        .message_plain(signex_i18n::ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE, &[]);
    assert!(
        outcome.warnings.iter().any(|w| w.contains(&banner)),
        "{:?}",
        outcome.warnings
    );
    assert_eq!(outcome.checked[2], 2, "the count needs no name");
}

#[test]
fn the_job_files_the_project_finding_under_the_path_the_graph_read() {
    let (inputs, root, child, loose) = colliding_job();

    let outcome = run_erc_job(inputs);

    let filed = multi_names(&outcome.by_path[&child]);
    assert_eq!(filed.len(), 1, "{filed:?}");
    assert!(filed[0].message.contains("'BUS'"), "{}", filed[0].message);
    assert!(
        filed[0].message.contains("'ROOT'"),
        "the root's name is listed"
    );
    assert!(multi_names(&outcome.by_path[&root]).is_empty());
    // The colliding tab is not the sheet the graph holds: its own finding stays.
    let kept = multi_names(&outcome.by_path[&loose]);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(
        kept[0].message.contains("on this sheet"),
        "{}",
        kept[0].message
    );
}

/// A sheet holding unit `unit` of `U1`, a two-unit `LM358`.
fn sheet_with_unit(unit: u32) -> signex_types::schematic::SchematicSheet {
    use signex_types::schematic::{FillType, Graphic, LibGraphic, LibSymbol, Symbol};
    let lib: LibSymbol =
        serde_json::from_value(serde_json::json!({"id": "Amplifier:LM358"})).expect("lib symbol");
    let graphic = |unit| LibGraphic {
        unit,
        body_style: 1,
        graphic: Graphic::Polyline {
            points: Vec::new(),
            width: 0.0,
            fill: FillType::default(),
        },
    };
    let mut sheet = signex_types::schematic::SchematicSheet {
        paper_size: "A4".into(),
        symbols: vec![Symbol {
            uuid: Uuid::now_v7(),
            lib_id: "Amplifier:LM358".into(),
            reference: "U1".into(),
            unit,
            position: pt(25.4, 25.4),
            ..Default::default()
        }],
        ..Default::default()
    };
    sheet.lib_symbols.insert(
        "Amplifier:LM358".into(),
        LibSymbol {
            graphics: vec![graphic(1), graphic(2)],
            ..lib
        },
    );
    sheet
}

#[test]
fn the_units_pass_spans_the_project_only_not_loose_tabs() {
    let root_path = Path::new("/proj/root.snxsch").to_path_buf();
    let loose_path = Path::new("/elsewhere/other.snxsch").to_path_buf();
    let open: crate::app::project_sheets::OpenSheets = [
        (root_path.clone(), sheet_with_unit(1)),
        (loose_path.clone(), sheet_with_unit(2)),
    ]
    .into_iter()
    .collect();
    let (mut inputs, ..) = colliding_job();
    inputs.tab_paths = open.keys().cloned().collect();
    inputs.open = open;
    inputs.root = Some(root_path.clone());
    let outcome = run_erc_job(inputs);
    let units = |p: &PathBuf| -> Vec<String> {
        outcome.by_path[p]
            .iter()
            .filter(|v| v.builtin_kind() == Some(RuleKind::UnitsNotPlaced))
            .map(|v| v.message.clone())
            .collect()
    };
    // The loose tab's unit 2 belongs to another design: it neither completes
    // the project's U1 nor loses its own per-sheet finding.
    assert_eq!(units(&root_path).len(), 1, "{:?}", units(&root_path));
    assert!(units(&root_path)[0].ends_with("unit 2"));
    assert_eq!(units(&loose_path).len(), 1);
    assert!(units(&loose_path)[0].ends_with("unit 1"));
}

/// A wire `(0,0)-(10,0)` with uuid `wire`, Global `VBUS` and a one-pin part,
/// carrying `directives`.
fn vbus_sheet(
    wire: u128,
    directives: Vec<signex_types::schematic::SchDirective>,
) -> SchematicSheet {
    let mut sheet = with_one_pin_part(named_wire(
        ("VBUS", LabelType::Global),
        ("VBUS", LabelType::Global),
    ));
    sheet.wires[0].uuid = Uuid::from_u128(wire);
    sheet.directives = directives;
    sheet
}

fn parameters(n: u128, class: &str) -> signex_types::schematic::SchDirective {
    signex_types::schematic::SchDirective::NetParameters {
        uuid: Uuid::from_u128(n),
        position: pt(5.0, 0.0),
        net_class: Some(class.into()),
        diff_pair_class: None,
        rules: Default::default(),
        show_on_flag: false,
    }
}

fn class_conflicts(list: &[Violation]) -> Vec<&Violation> {
    list.iter()
        .filter(|v| v.builtin_kind() == Some(RuleKind::NetClassConflict))
        .collect()
}

/// #2656: the child gives global `VBUS` class POWER, the root SIGNAL, with a
/// SW_NODE Blanket under its flag. The netlist reads the sheets in key order,
/// so the child's POWER wins. The job reports both root directives as
/// overridden by it, the Blanket once (the root's own finding against its
/// SIGNAL flag is replaced, not doubled), and nothing on the child.
#[test]
fn the_job_reports_a_class_conflict_between_two_sheets() {
    let (mut inputs, ..) = colliding_job();
    let dir = Path::new("/proj");
    let (root_path, child_path) = (dir.join("root.snxsch"), dir.join("child.snxsch"));
    let blanket = signex_types::schematic::SchDirective::Blanket {
        uuid: Uuid::from_u128(0x2656_11),
        start: pt(-1.0, -1.0),
        end: pt(11.0, 1.0),
        net_class: Some("SW_NODE".into()),
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: Default::default(),
        include_edge_nets: false,
    };
    let mut root = vbus_sheet(0x2656_01, vec![parameters(0x2656_12, "SIGNAL"), blanket]);
    let mut placement = child_with_pin("child.snxsch", "UNUSED");
    placement.pins.clear();
    root.child_sheets.push(placement);
    let child = vbus_sheet(0x2656_02, vec![parameters(0x2656_13, "POWER")]);
    let open: crate::app::project_sheets::OpenSheets =
        [(root_path.clone(), root), (child_path.clone(), child)]
            .into_iter()
            .collect();
    inputs.tab_paths = open.keys().cloned().collect();
    inputs.open = open;
    inputs.root = Some(root_path.clone());

    let outcome = run_erc_job(inputs);

    let on_child = class_conflicts(&outcome.by_path[&child_path]);
    assert!(
        on_child.is_empty(),
        "the winner is not overridden: {on_child:?}"
    );
    let on_root = class_conflicts(&outcome.by_path[&root_path]);
    let mut primaries: Vec<Uuid> = on_root
        .iter()
        .filter_map(|v| v.primary.map(|p| p.uuid))
        .collect();
    primaries.sort();
    assert_eq!(
        primaries,
        [Uuid::from_u128(0x2656_11), Uuid::from_u128(0x2656_12)],
        "{on_root:?}"
    );
    for v in &on_root {
        assert!(
            v.message
                .contains("takes class POWER from a Net Parameters directive on sheet"),
            "{}",
            v.message
        );
        assert_eq!(v.peer, None, "the winner is on the child's canvas");
    }
}
