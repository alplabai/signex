//! #2549: which net names `assign_net_class` takes. The fixture is the agent
//! project `P` whose root `a` places the closed child `power.snxsch` twice:
//! the child's one wire carries two net names, and its global label `LONELY`
//! names a net that owns no wire.

use std::collections::BTreeSet;
use std::path::Path;

use signex_types::net_class::NetClass;
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};

use super::{SHEET_A, app_in};
use crate::app::Signex;
use crate::app::command::queries::tests::uid;
use crate::app::handlers::editing_commands::net_color_driver as fixture;

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
