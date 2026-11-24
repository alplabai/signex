//! #2549: which net names `assign_net_class` takes. The fixture is the agent
//! project `P` whose root `a` places the closed child `power.snxsch` twice:
//! the child's one wire carries two net names, and its global label `LONELY`
//! names a net that owns no wire.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::net_class::NetClass;
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};

use super::{SHEET_A, app_in, call_settled};
use crate::app::Signex;
use crate::app::command::queries::tests::uid;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

const LONELY: &str = "LONELY";
const CHILD_WIRE: u128 = 900;
const CHILD_LABEL: u128 = 901;
const LONELY_LABEL: u128 = 902;
const SECOND_PLACEMENT: u128 = 903;
const CHILD_PART: u128 = 904;
const LONELY_PART: u128 = 905;

/// A one-pin part, the pin's tip at the symbol's position: a net exists only
/// where a terminal lands.
fn one_pin() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "P" })).unwrap();
    lib.pins = vec![LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: "1".into(),
            number: "1".into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }];
    lib
}

fn part(n: u128, reference: &str, at: Point) -> Symbol {
    Symbol {
        uuid: uid(n),
        lib_id: "P".into(),
        reference: reference.into(),
        position: at,
        unit: 1,
        ..Symbol::default()
    }
}

/// The child sheet: a wire to a pin, named by the local label `X`, and the
/// global label `LONELY` on a pin with no wire.
fn child_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: uid(SHEET_A + 50),
        paper_size: "A4".into(),
        symbols: vec![
            part(CHILD_PART, "P1", Point::new(10.0, 0.0)),
            part(LONELY_PART, "P2", Point::new(30.0, 20.0)),
        ],
        wires: vec![Wire {
            uuid: uid(CHILD_WIRE),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            ..Default::default()
        }],
        labels: vec![
            Label {
                uuid: uid(CHILD_LABEL),
                ..Label::new("X", Point::new(0.0, 0.0), LabelType::Net)
            },
            Label {
                uuid: uid(LONELY_LABEL),
                ..Label::new(LONELY, Point::new(30.0, 20.0), LabelType::Global)
            },
        ],
        ..Default::default()
    };
    sheet.lib_symbols.insert("P".into(), one_pin());
    sheet
}

/// The agent project in `dir`, with the class `Power`, the closed child on
/// disk and placed a second time on `a`; the person stays on `b`.
fn app_with_label_only_and_shared_nets(dir: &Path) -> Signex {
    let text = signex_types::format::SnxSchematic::new(child_sheet())
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("power.snxsch"), text).expect("write the child");
    let mut app = app_in(dir);
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    let person = app.document_state.active_path.clone().expect("a sheet");
    fixture::activate(&mut app, &dir.join("a.snxsch"));
    let mut second = app
        .document_state
        .active_engine()
        .expect("a")
        .document()
        .child_sheets[0]
        .clone();
    second.uuid = uid(SECOND_PLACEMENT);
    second.name = "Power2".into();
    second.position = Point::new(80.0, 50.0);
    let placed = signex_engine::Command::PlaceChildSheet {
        child_sheet: second,
    };
    assert!(app.apply_engine_command(placed, true, false));
    fixture::activate(&mut app, &person);
    app
}

/// Every net name of the project netlist rooted at `a`.
fn netlist_names(app: &mut Signex, dir: &Path) -> BTreeSet<String> {
    let person = app.document_state.active_path.clone().expect("a sheet");
    fixture::activate(app, &dir.join("a.snxsch"));
    let stitched = crate::app::stitched_netlist::stitched_for_active_project(&app.document_state);
    fixture::activate(app, &person);
    let netlist = stitched.netlist.expect("the project stitches");
    netlist.nets.into_iter().map(|net| net.name).collect()
}

/// Characterisation of the GUI cache's `knows_net` against the project
/// netlist: the cache knows only nets that own a wire or a junction, and of a
/// shared wire only one of its names, so a label-only net and the second name
/// are netlist nets it does not know.
#[test]
fn the_gui_cache_knows_fewer_nets_than_the_project_netlist() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_label_only_and_shared_nets(dir.path());
    app.settle_net_class_styles_blocking();
    let project = app.document_state.projects[0].id;

    let names = netlist_names(&mut app, dir.path());
    let known: BTreeSet<String> = names
        .iter()
        .filter(|name| app.ui_state.net_class_styles.knows_net(project, name))
        .cloned()
        .collect();
    let unknown: BTreeSet<&String> = names.difference(&known).collect();
    let shared: Vec<&String> = names.iter().filter(|n| n.ends_with('X')).collect();

    assert!(names.contains(LONELY), "{names:?}");
    assert_eq!(shared.len(), 2, "one wire, two placements: {names:?}");
    assert!(known.contains("VCC"), "{known:?} of {names:?}");
    assert!(unknown.contains(&LONELY.to_string()), "{unknown:?}");
    assert_eq!(
        shared.iter().filter(|n| unknown.contains(**n)).count(),
        1,
        "the cache keeps one name of the shared wire: {unknown:?}"
    );
    assert_eq!(unknown.len(), 2, "{unknown:?}");
}

/// The agent's `assign_net_class` of `net` into `Power` on sheet `a`.
fn agents_pick(app: &mut Signex, dir: &Path, net: &str) -> Value {
    let document = dir.join("a.snxsch").display().to_string();
    call_settled(
        app,
        "assign_net_class",
        json!({"document": document, "net_name": net, "class": "Power"}),
    )
}

/// The same pick run in the app's own process (the GUI's dispatch of the id).
fn persons_pick(app: &mut Signex, dir: &Path, net: &str) -> CommandOutcome {
    let id = AppCommandId::new("assign_net_class").unwrap();
    let document = format!("document={}", dir.join("a.snxsch").display());
    let net = format!("net_name={net}");
    let args = CommandArgs::parse_operands(&id, &[&document, &net, "class=Power"]).unwrap();
    app.run_operands_settled(&id, &args).0
}

fn members(app: &Signex) -> Vec<String> {
    app.document_state.projects[0].data.net_classes[0]
        .nets
        .clone()
}

/// Decision (Hakan, 2026-11-19): any net of the project netlist is taken, a
/// net only a label names and the second name of a shared wire included,
/// with no style resolve run at all.
#[test]
fn a_label_only_net_and_every_name_of_a_shared_wire_are_assignable() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_label_only_and_shared_nets(dir.path());
    let names = netlist_names(&mut app, dir.path());
    let mut wanted: Vec<String> = names.iter().filter(|n| n.ends_with('X')).cloned().collect();
    wanted.push(LONELY.into());

    for net in &wanted {
        let reply = agents_pick(&mut app, dir.path(), net);
        assert_eq!(reply["status"], "ran", "{net}: {reply}");
    }

    let mut held = members(&app);
    held.sort();
    wanted.sort();
    assert_eq!(held, wanted);
}

/// The pick no longer races the background resolve of the canvas styles: right
/// after a load, before any resolve, a net of the documents is taken.
#[test]
fn an_assignment_right_after_a_load_does_not_wait_for_the_style_resolve() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    let project = app.document_state.projects[0].id;
    assert!(
        !app.ui_state.net_class_styles.knows_net(project, "VCC"),
        "the styles are not resolved yet"
    );

    let reply = agents_pick(&mut app, dir.path(), "VCC");

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(members(&app), ["VCC"]);
}

/// A name no document gives a net is still refused, with that reason only.
#[test]
fn a_net_in_no_document_is_refused_with_its_reason() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_label_only_and_shared_nets(dir.path());
    app.settle_net_class_styles_blocking();

    let reply = agents_pick(&mut app, dir.path(), "NOPE");

    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(
        reply["reason"], "\"NOPE\" is not a net of the project netlist",
        "{reply}"
    );
    assert!(members(&app).is_empty());
}

/// The membership check only asks: a child reference that escapes the project
/// folder, which the style job reports, adds no Messages line per call, as the
/// cache lookup it replaced added none.
#[test]
fn the_membership_check_logs_no_refused_child_reference() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_label_only_and_shared_nets(dir.path());
    let reference = format!("../escape-{}.snxsch", uuid::Uuid::now_v7().simple());
    let person = app.document_state.active_path.clone().expect("a sheet");
    fixture::activate(&mut app, &dir.path().join("a.snxsch"));
    let mut escaping = app
        .document_state
        .active_engine()
        .expect("a")
        .document()
        .child_sheets[0]
        .clone();
    escaping.uuid = uuid::Uuid::now_v7();
    escaping.filename = reference.clone();
    escaping.position = Point::new(150.0, 50.0);
    let placed = signex_engine::Command::PlaceChildSheet {
        child_sheet: escaping,
    };
    assert!(app.apply_engine_command(placed, true, false));
    // The whole project in memory, so the check stitches in place; the
    // background jobs a settled request runs log the reference on their own.
    let child = dir.path().join("power.snxsch");
    fixture::add_sheet_with(&mut app, &child, child_sheet());
    app.document_state.mark_document_clean(&child);
    fixture::activate(&mut app, &person);
    let mark = crate::diagnostics::newest_entry_id();

    let has = app.project_has_net(&dir.path().join("a.snxsch"), LONELY);

    assert_eq!(has, Ok(true));
    let logged = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .filter(|entry| entry.message.contains(&reference))
        .count();
    assert_eq!(logged, 0, "the check logged the refused reference");
}

/// Trunk behaviour kept, not decided by #2549: the Properties label picker
/// still reads the style cache through the label's wire, so a label-only net
/// gets no picker, while the `assign_net_class` id takes that net.
#[test]
fn the_label_picker_still_offers_no_class_for_a_label_only_net() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_label_only_and_shared_nets(dir.path());
    let child = dir.path().join("power.snxsch");
    fixture::add_sheet_with(&mut app, &child, child_sheet());
    app.document_state.mark_document_clean(&child);
    app.interaction_state.active_canvas_mut().selected =
        vec![signex_types::schematic::SelectedItem::new(
            uid(LONELY_LABEL),
            signex_types::schematic::SelectedKind::Label,
        )];
    app.update_selection_info();
    app.settle_net_class_styles_blocking();

    assert!(app.selected_label_class_pick().is_none());
    let reply = agents_pick(&mut app, dir.path(), LONELY);
    assert_eq!(reply["status"], "ran", "{reply}");
}

/// One command id, one behaviour: every run of `assign_net_class` (the app's
/// own dispatch of the id and the agent's) answers the same for a label-only
/// net, a net in no document and a wired one. This is the id, not the
/// Properties label picker, which still reads the style cache.
#[test]
fn every_run_of_the_assign_net_class_id_gives_the_same_answer() {
    let dir = tempfile::tempdir().unwrap();
    let mut gui = app_with_label_only_and_shared_nets(dir.path());
    let mut agent = app_with_label_only_and_shared_nets(dir.path());

    for net in [LONELY, "NOPE", "VCC"] {
        let reply = agents_pick(&mut agent, dir.path(), net);
        let (status, reason) = match persons_pick(&mut gui, dir.path(), net) {
            CommandOutcome::Ran => ("ran", Value::Null),
            CommandOutcome::Disabled { reason } => ("refused", json!(reason.render_en_us())),
            other => panic!("{net}: {other:?}"),
        };
        assert_eq!(reply["status"], status, "{net}: {reply}");
        if status == "refused" {
            assert_eq!(reply["reason"], reason, "{net}");
        }
    }
    assert_eq!(members(&gui), members(&agent));
    assert_eq!(members(&gui), [LONELY, "VCC"]);
}
