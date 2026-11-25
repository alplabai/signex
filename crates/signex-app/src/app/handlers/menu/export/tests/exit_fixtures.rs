//! Advanced Schematic exit criteria (`docs/ROADMAP.md`, "Advanced Schematic
//! (P1)") on the projects on disk in `signex-types/tests/corpus/advanced/`,
//! read with the real parser and exported through the app's own scope: the
//! 4-channel, two-variant project gives a BOM and a PDF per variant that
//! match their goldens, and the classes and rules the directives give reach
//! Update PCB. Goldens: `crates/signex-app/tests/golden/advanced_schematic/`;
//! `UPDATE_GOLDEN=1 cargo test -p signex-app --lib exit_fixtures` rewrites
//! them (a bare `UPDATE_GOLDEN=1` run rewrites the command golden too).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_output::{BomColumn, BomExporter, BomFormat, BomOptions, Exporter, PdfExporter};
use signex_output::{PdfOptions, bom};
use signex_types::format::{SnxPcb, SnxSchematic};

use super::open_with;
use crate::app::Signex;
use crate::app::handlers::menu::export::{build_bom_export_context, build_export_scope};
use crate::app::project_sheets::plan_project_sheets_for;
use crate::app::saved_channel_naming::SavedChannelNaming;
use crate::app::state::LoadedProject;
use crate::app::update_pcb::design::{DesignRequest, compute};

pub(crate) const FOUR_CHANNEL: &str = "four-channel-variants";
const CLASSES: &str = "net-class-directives";
pub(crate) const VARIANTS: [&str; 2] = ["Full", "Lite"];

pub(crate) fn fixture_dir(project: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder")
        .join("signex-types/tests/corpus/advanced")
        .join(project)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (the fixture is generated: SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration advanced_fixtures)",
            path.display()
        )
    })
}

/// A `Signex` with the fixture `project` loaded and active and every sheet
/// it lists open, its root in view.
pub(crate) fn open_fixture(project: &str) -> Signex {
    let (mut app, _task) = Signex::new();
    let dir = fixture_dir(project);
    let path = dir.join(format!("{project}.snxprj"));
    assert!(
        path.is_file(),
        "{} is missing (the fixture is generated: SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration advanced_fixtures)",
        path.display()
    );
    let data = signex_types::project::parse_project(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let root = dir.join(data.schematic_root.as_deref().expect("a root sheet"));
    let files: Vec<PathBuf> = data.sheets.iter().map(|s| dir.join(&s.filename)).collect();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path,
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    for file in files {
        let sheet = SnxSchematic::parse(&read(&file))
            .unwrap_or_else(|e| panic!("{}: {e}", file.display()))
            .sheet;
        open_with(&mut app.document_state, &file, sheet);
    }
    app.document_state.active_path = Some(root);
    app
}

/// Write `bytes` as the golden `name` under `UPDATE_GOLDEN=1`, else compare.
pub(crate) fn check_golden(name: &str, bytes: &[u8]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/advanced_schematic")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the golden folder");
        std::fs::write(&path, bytes).expect("the golden writes");
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (run with UPDATE_GOLDEN=1 to write it)",
            path.display()
        )
    });
    assert!(
        bytes == expected.as_slice(),
        "{name} differs from its golden (an intentional change: rerun with UPDATE_GOLDEN=1 and review the diff)"
    );
}

fn bom_options(variant: &str) -> BomOptions {
    BomOptions {
        columns: vec![
            BomColumn::Designator,
            BomColumn::Value,
            BomColumn::Footprint,
            BomColumn::LibRef,
            BomColumn::Qty,
        ],
        format: BomFormat::Csv,
        active_variant: Some(variant.to_string()),
        ..BomOptions::default()
    }
}

/// The BOM file of `variant` as text with LF line ends: the file itself
/// starts with a byte order mark and ends its lines with CRLF.
fn bom_csv(app: &Signex, variant: &str) -> String {
    let ctx = build_bom_export_context(&app.document_state).expect("the project's BOM scope");
    let out = BomExporter
        .export(&ctx, &bom_options(variant))
        .expect("the BOM exports");
    let text = String::from_utf8(out.bytes).expect("utf-8");
    text.trim_start_matches('\u{feff}').replace("\r\n", "\n")
}

/// Every designator the BOM of `variant` buys, sorted, with its value.
fn bom_parts(app: &Signex, variant: &str) -> Vec<(String, String)> {
    let ctx = build_bom_export_context(&app.document_state).expect("the project's BOM scope");
    let table = bom::rollup(&ctx, &bom_options(variant));
    let mut parts: Vec<(String, String)> = table
        .rows
        .iter()
        .flat_map(|row| {
            row.references
                .iter()
                .map(|r| (r.clone(), row.value.clone()))
        })
        .collect();
    parts.sort();
    parts
}

pub(crate) fn pdf_options(variant: &str) -> PdfOptions {
    PdfOptions {
        variant: Some(variant.to_string()),
        ..PdfOptions::default()
    }
}

/// The PDF of `variant` and its page count.
pub(crate) fn pdf(app: &Signex, options: &PdfOptions) -> (Vec<u8>, usize) {
    let (ctx, _) = build_export_scope(&app.document_state).expect("the project's page scope");
    let out = PdfExporter.export(&ctx, options).expect("the PDF exports");
    (out.bytes, out.page_count)
}

fn parts(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(r, v)| ((*r).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn the_full_variant_buys_every_channel_with_ch1s_own_value() {
    let app = open_fixture(FOUR_CHANNEL);
    assert_eq!(
        bom_parts(&app, "Full"),
        parts(&[
            ("CH1_C1", "100n"),
            ("CH1_C2", "1u"),
            ("CH1_R1", "4k7"),
            ("CH2_C1", "100n"),
            ("CH2_C2", "1u"),
            ("CH2_R1", "10k"),
            ("CH3_C1", "100n"),
            ("CH3_C2", "1u"),
            ("CH3_R1", "10k"),
            ("CH4_C1", "100n"),
            ("CH4_C2", "1u"),
            ("CH4_R1", "10k"),
            ("U1", "MCU"),
        ])
    );
}

#[test]
fn the_lite_variant_leaves_out_c2_everywhere_and_r1_on_ch3_and_ch4() {
    let app = open_fixture(FOUR_CHANNEL);
    assert_eq!(
        bom_parts(&app, "Lite"),
        parts(&[
            ("CH1_C1", "100n"),
            ("CH1_R1", "10k"),
            ("CH2_C1", "100n"),
            ("CH2_R1", "10k"),
            ("CH3_C1", "100n"),
            ("CH4_C1", "100n"),
            ("U1", "MCU"),
        ])
    );
}

#[test]
fn each_variant_exports_its_golden_bom() {
    let app = open_fixture(FOUR_CHANNEL);
    for variant in VARIANTS {
        let csv = bom_csv(&app, variant);
        check_golden(&format!("{FOUR_CHANNEL}.{variant}.bom.csv"), csv.as_bytes());
    }
}

#[test]
fn each_variant_exports_its_golden_pdf_of_the_top_page_and_four_channels() {
    let app = open_fixture(FOUR_CHANNEL);
    for variant in VARIANTS {
        let options = pdf_options(variant);
        let (bytes, pages) = pdf(&app, &options);
        assert_eq!(pages, 5, "{variant}: the top page and one page per channel");
        assert_eq!(
            bytes,
            pdf(&app, &options).0,
            "{variant}: two exports of the same project give the same file"
        );
        check_golden(&format!("{FOUR_CHANNEL}.{variant}.pdf"), &bytes);
    }
}

/// The net named on `pin` (`U1.2`) in `netlist`, with its class.
fn class_on(netlist: &signex_types::net::Netlist, pin: &str) -> Option<String> {
    let (reference, number) = pin.split_once('.').expect("a pin reference");
    netlist
        .nets
        .iter()
        .find(|n| {
            n.terminals
                .iter()
                .any(|t| t.reference == reference && t.pin == number)
        })
        .unwrap_or_else(|| panic!("no net holds {pin}"))
        .class
        .as_ref()
        .map(|c| c.0.clone())
}

fn classed_nets(netlist: &signex_types::net::Netlist) -> Vec<String> {
    let mut lines: Vec<String> = netlist
        .nets
        .iter()
        .map(|n| {
            let class = n.class.as_ref().map_or("-", |c| c.0.as_str());
            let mut pins: Vec<String> = (n.terminals.iter())
                .map(|t| format!("{}.{}", t.reference, t.pin))
                .collect();
            pins.sort();
            format!("{}\t{class}\t{}", n.name, pins.join(" "))
        })
        .collect();
    lines.sort();
    lines
}

/// Exit criterion 3: Update PCB reads the classes a Net Blanket and a Net
/// Parameters directive give, and its Execute stores the rules the Net
/// Parameters directive asks for in the project's DRC rules, marked as from
/// the schematic. The board's nets are classed by name through the
/// project's net-class table (`docs/SCHEMATIC_DIRECTIVES.md`, Known limits).
#[test]
fn the_directive_classes_and_rules_reach_update_pcb() {
    let mut app = open_fixture(CLASSES);
    let dir = fixture_dir(CLASSES);
    let board = SnxPcb::parse(&read(&dir.join(format!("{CLASSES}.snxpcb"))))
        .expect("the board reads")
        .board;
    let computed = compute(DesignRequest {
        plan: plan_project_sheets_for(&app.document_state, &dir.join("main.snxsch")),
        project_dir: dir.clone(),
        board,
        revision: 0,
        libraries: Vec::new(),
        open_revisions: Vec::new(),
        previous: None,
        naming: SavedChannelNaming::of(&app.document_state.projects[0].data),
    })
    .expect("the design reads");
    let snapshot = &computed.snapshot;
    assert_eq!(
        class_on(&snapshot.netlist, "U1.1").as_deref(),
        Some("POWER")
    );
    assert_eq!(class_on(&snapshot.netlist, "U1.2").as_deref(), Some("HS"));
    assert_eq!(class_on(&snapshot.netlist, "U1.5"), None);
    let names: Vec<&str> = (snapshot.schematic_rules.iter())
        .map(|rule| rule.name.as_str())
        .collect();
    assert_eq!(names, ["Width_HS", "Clearance_HS"]);

    let rows = signex_drc_dsl::schematic_rules::rule_rows(&snapshot.schematic_rules, "");
    let project = app.document_state.projects[0].id;
    app.apply_update_pcb_rules(project, &rows)
        .expect("Execute stores the rules");
    let drc = &(app.document_state.projects[0].data.rules.as_ref())
        .expect("the project has rules now")
        .drc;
    assert_eq!(drc.from_schematic, ["Width_HS", "Clearance_HS"]);
    let dsl = drc.dsl.clone().unwrap_or_default();
    for name in ["Width_HS", "Clearance_HS"] {
        assert!(dsl.contains(name), "{name} in the stored rules:\n{dsl}");
    }

    let mut golden = String::from("# nets (name, class, pins)\n");
    for line in classed_nets(&snapshot.netlist) {
        golden.push_str(&line);
        golden.push('\n');
    }
    golden.push_str("# rules Update PCB lists\n");
    for rule in &snapshot.schematic_rules {
        golden.push_str(&rule.text());
        golden.push('\n');
    }
    golden.push_str("# stored DRC rules after Execute\n");
    golden.push_str(&dsl);
    if !dsl.ends_with('\n') {
        golden.push('\n');
    }
    check_golden(&format!("{CLASSES}.update_pcb.txt"), golden.as_bytes());
}
