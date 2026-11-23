//! #2420: the Blanket, Compile Mask and Differential Pair directives, driven
//! once through `Signex::update` (Active Bar, canvas clicks, Properties) and
//! once through their operands (the agent's and the CLI's path, and an
//! agent's `apply_edits`). Each placement and each edit is one undo step, and
//! the netlist and ERC read what landed.

use serde_json::json;
use signex_types::schematic::{Point, SchDirective};

use super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMsg};
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, doc, grid, sheet_shape};
use crate::app::{Message, Tool};
use crate::keymap::AppCommandId;

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn pt(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = command(id);
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn undo(app: &mut Signex) {
    let _ = app.dispatch_command_reporting(&command("undo"), CommandArgs::none(), None);
}

fn bar(app: &mut Signex, action: ActiveBarAction) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
}

fn tool_app() -> Signex {
    app_with_tool(Tool::Select)
}

/// Each directive's Active Bar action, the tool it arms, its command and the
/// operands that stand for its clicks.
fn cases() -> Vec<(ActiveBarAction, Tool, &'static str, Vec<Point>)> {
    let (a, b) = (grid(8.0, 16.0), grid(24.0, 24.0));
    vec![
        (
            ActiveBarAction::PlaceBlanket,
            Tool::Blanket,
            "place_blanket",
            vec![a, b],
        ),
        (
            ActiveBarAction::PlaceCompileMask,
            Tool::CompileMask,
            "place_compile_mask",
            vec![a, b],
        ),
        (
            ActiveBarAction::PlaceDiffPair,
            Tool::DiffPair,
            "place_diff_pair",
            vec![a],
        ),
    ]
}

fn operands_for(points: &[Point]) -> Vec<String> {
    match points {
        [at] => vec![format!("at={}", pt(*at))],
        [start, end] => vec![format!("start={}", pt(*start)), format!("end={}", pt(*end))],
        _ => unreachable!(),
    }
}

#[test]
fn each_active_bar_tool_arms_and_its_clicks_place_one_undo_step() {
    for (action, tool, id, clicks) in cases() {
        let mut app = tool_app();
        bar(&mut app, action);
        assert_eq!(app.interaction_state.current_tool, tool, "{id}");
        for at in &clicks {
            click(&mut app, (at.x, at.y));
        }
        assert_eq!(doc(&app).directives.len(), 1, "{id}");
        // The tool stays armed for the next one.
        assert_eq!(app.interaction_state.current_tool, tool, "{id}");
        undo(&mut app);
        assert!(doc(&app).directives.is_empty(), "{id}");
    }
}

#[test]
fn the_first_corner_alone_places_nothing() {
    let mut app = tool_app();
    bar(&mut app, ActiveBarAction::PlaceBlanket);
    click(&mut app, (grid(8.0, 16.0).x, grid(8.0, 16.0).y));
    assert!(doc(&app).directives.is_empty());
    assert!(app.interaction_state.shape_anchor.is_some());
}

#[test]
fn each_directive_command_with_operands_makes_the_clicked_document() {
    for (action, _tool, id, clicks) in cases() {
        let mut clicked = tool_app();
        bar(&mut clicked, action);
        for at in &clicks {
            click(&mut clicked, (at.x, at.y));
        }
        let mut app = tool_app();
        let operands = operands_for(&clicks);
        let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
        let answer = answered(run(&mut app, id, &refs));
        assert_eq!(sheet_shape(&app), sheet_shape(&clicked), "{id}");
        assert_eq!(
            answer["id"],
            json!(doc(&app).directives[0].uuid().to_string()),
            "{id}"
        );
        // Nothing click-only: the tool and the anchor stay as they were.
        assert_eq!(app.interaction_state.current_tool, Tool::Select, "{id}");
        assert!(app.interaction_state.shape_anchor.is_none(), "{id}");
        undo(&mut app);
        assert!(doc(&app).directives.is_empty(), "{id}");
    }
}

#[test]
fn place_blanket_takes_its_parameters_and_edit_blanket_changes_them_in_one_step() {
    let mut app = tool_app();
    let (a, b) = (grid(8.0, 16.0), grid(24.0, 24.0));
    let answer = answered(run(
        &mut app,
        "place_blanket",
        &[
            &format!("start={}", pt(a)),
            &format!("end={}", pt(b)),
            "class=HighSpeed",
            "exclude=builtin::unused_pin, builtin::orphan_label",
        ],
    ));
    let id = answer["id"].as_str().expect("id").to_string();
    assert_eq!(answer["directive"]["net_class"], json!("HighSpeed"));
    assert_eq!(
        answer["directive"]["erc_exclusions"],
        json!(["builtin::unused_pin", "builtin::orphan_label"])
    );

    let edited = answered(run(
        &mut app,
        "edit_blanket",
        &[&format!("id={id}"), "class=Fast", "exclude="],
    ));
    assert_eq!(edited["directive"]["net_class"], json!("Fast"));
    let SchDirective::Blanket {
        net_class,
        erc_exclusions,
        ..
    } = &doc(&app).directives[0]
    else {
        panic!("a blanket")
    };
    assert_eq!(net_class.as_deref(), Some("Fast"));
    assert!(erc_exclusions.is_empty());

    // The edit is one step of its own: one undo restores the placed values.
    undo(&mut app);
    assert_eq!(doc(&app).directives[0].net_class(), Some("HighSpeed"));
    undo(&mut app);
    assert!(doc(&app).directives.is_empty());
}

#[test]
fn an_unknown_rule_id_or_a_flat_region_is_refused_with_the_reason() {
    let mut app = tool_app();
    let (a, b) = (grid(8.0, 16.0), grid(24.0, 24.0));
    let unknown = run(
        &mut app,
        "place_blanket",
        &[
            &format!("start={}", pt(a)),
            &format!("end={}", pt(b)),
            "exclude=builtin::no_such_rule",
        ],
    );
    assert!(
        matches!(&unknown, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("not an ERC rule id")),
        "{unknown:?}"
    );
    let flat = run(
        &mut app,
        "place_compile_mask",
        &[&format!("start={}", pt(a)), &format!("end={},{}", b.x, a.y)],
    );
    assert!(
        matches!(&flat, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("opposite corners")),
        "{flat:?}"
    );
    assert!(doc(&app).directives.is_empty());
}

#[test]
fn edit_blanket_without_operands_says_what_it_needs() {
    let mut app = tool_app();
    let (outcome, _task) =
        app.dispatch_command_reporting(&command("edit_blanket"), CommandArgs::none(), None);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("needs the operand `id`")),
        "{outcome:?}"
    );
}

#[test]
fn typing_a_class_in_properties_is_one_undo_step_and_a_rule_checkbox_another() {
    use crate::panels::{BlanketEdit, PanelMsg};
    let mut app = tool_app();
    let (a, b) = (grid(8.0, 16.0), grid(24.0, 24.0));
    bar(&mut app, ActiveBarAction::PlaceBlanket);
    click(&mut app, (a.x, a.y));
    click(&mut app, (b.x, b.y));
    let id = doc(&app).directives[0].uuid();
    let send = |app: &mut Signex, edit: BlanketEdit| {
        let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
            PanelMsg::EditBlanket(id, edit),
        )));
    };
    for typed in ["H", "Hi", "High"] {
        send(&mut app, BlanketEdit::NetClass(typed.to_string()));
    }
    send(
        &mut app,
        BlanketEdit::Rule {
            rule_id: "builtin::orphan_label".to_string(),
            excluded: true,
        },
    );
    let directive = &doc(&app).directives[0];
    assert_eq!(directive.net_class(), Some("High"));
    assert!(directive.excludes_rule("builtin::orphan_label"));

    undo(&mut app);
    assert!(!doc(&app).directives[0].excludes_rule("builtin::orphan_label"));
    assert_eq!(doc(&app).directives[0].net_class(), Some("High"));
    undo(&mut app);
    assert_eq!(doc(&app).directives[0].net_class(), None);
}

#[test]
fn an_agent_edit_list_places_directives_as_one_step_it_can_revert() {
    use super::queries::tests::{app_with, path};
    let mut app = app_with(&[("directives.snxsch", 300)]);
    app.ui_state.snap_enabled = false;
    let file = path("directives.snxsch");
    let document = format!("document={}", file.display());
    let revision = app.document_state.engines[&file].revision();
    let edits = json!([
        {"id": "place_compile_mask", "args": {"start": [60, 60], "end": [70, 70]}},
        {"id": "place_diff_pair", "args": {"at": [80, 80]}},
    ]);
    let answer = answered(run(
        &mut app,
        "apply_edits",
        &[
            &document,
            &format!("expect_revision={revision}"),
            &format!("edits={edits}"),
        ],
    ));
    assert_eq!(answer["edits"], json!(2));
    assert_eq!(
        app.document_state.engines[&file]
            .document()
            .directives
            .len(),
        2
    );
    let group = answer["group"].as_u64().expect("group");
    answered(run(
        &mut app,
        "revert",
        &[&document, &format!("group={group}")],
    ));
    assert!(
        app.document_state.engines[&file]
            .document()
            .directives
            .is_empty()
    );
}

/// A resistor library entry with one pin at its origin.
fn resistor() -> signex_types::schematic::LibSymbol {
    use signex_types::schematic::{LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle};
    LibSymbol {
        id: "R".into(),
        reference: "R".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// The active sheet holds the net `CLK_P`: a wire from `grid(10, 10)` to
/// `grid(20, 10)` with a resistor on each end.
fn with_clk_p_net(app: &mut Signex) {
    use signex_types::schematic::{Label, LabelType, Symbol, Wire};
    let (start, end) = (grid(10.0, 10.0), grid(20.0, 10.0));
    let engine = app.document_state.active_engine_mut().expect("engine");
    let mut sheet = engine.document().clone();
    sheet.lib_symbols.insert("R".into(), resistor());
    for (n, at) in [(11u128, start), (12, end)] {
        sheet.symbols.push(Symbol {
            uuid: uuid::Uuid::from_u128(n),
            lib_id: "R".into(),
            reference: format!("R{n}"),
            position: at,
            ..Default::default()
        });
    }
    sheet.wires.push(Wire {
        uuid: uuid::Uuid::from_u128(1),
        start,
        end,
        ..Default::default()
    });
    sheet
        .labels
        .push(Label::new("CLK_P", start, LabelType::Net));
    engine.set_document(sheet);
}

#[test]
fn the_netlist_reads_what_the_directive_commands_placed() {
    let mut app = tool_app();
    with_clk_p_net(&mut app);
    let netlist = |app: &Signex| signex_net::build_netlist(doc(app));
    assert_eq!(netlist(&app).nets.len(), 1);
    assert_eq!(netlist(&app).nets[0].name, "CLK_P");

    // A Blanket around the net gives it its class.
    answered(run(
        &mut app,
        "place_blanket",
        &[
            &format!("start={}", pt(grid(8.0, 8.0))),
            &format!("end={}", pt(grid(22.0, 12.0))),
            "class=Clock",
        ],
    ));
    assert_eq!(
        netlist(&app).nets[0].class,
        Some(signex_types::net::NetClassId("Clock".into()))
    );

    // A Differential Pair directive on it names the pair; the `_N` half is
    // not on the sheet, so the pair is incomplete.
    let answer = answered(run(
        &mut app,
        "place_diff_pair",
        &[&format!("at={}", pt(grid(15.0, 10.0)))],
    ));
    assert_eq!(answer["pair"]["name"], json!("CLK"));
    assert_eq!(answer["pair"]["positive"], json!("CLK_P"));
    assert_eq!(answer["pair"]["negative"], json!(null));
    assert_eq!(answer["pair"]["complete"], json!(false));

    // A Compile Mask over all of it takes the net out of the netlist.
    answered(run(
        &mut app,
        "place_compile_mask",
        &[
            &format!("start={}", pt(grid(6.0, 6.0))),
            &format!("end={}", pt(grid(24.0, 14.0))),
        ],
    ));
    assert!(netlist(&app).nets.is_empty());
    undo(&mut app);
    assert_eq!(netlist(&app).nets.len(), 1);
}

/// #1489: `place_parameter_set` puts a Net Parameters flag on the wire under
/// `at` with its class and rules, answers with the net, gives the net its
/// class (over a Blanket's), and is refused off a wire.
#[test]
fn place_parameter_set_takes_its_operands_and_is_refused_off_a_wire() {
    let mut app = tool_app();
    with_clk_p_net(&mut app);
    let off = run(
        &mut app,
        "place_parameter_set",
        &[&format!("at={}", pt(grid(40.0, 40.0)))],
    );
    assert!(
        !matches!(off, CommandOutcome::Answered(_)),
        "refused off a wire: {off:?}"
    );
    assert!(doc(&app).directives.is_empty());

    answered(run(
        &mut app,
        "place_blanket",
        &[
            &format!("start={}", pt(grid(8.0, 8.0))),
            &format!("end={}", pt(grid(22.0, 12.0))),
            "class=Blanketed",
        ],
    ));
    let answer = answered(run(
        &mut app,
        "place_parameter_set",
        &[
            &format!("at={}", pt(grid(15.0, 10.0))),
            "class=POWER_IN",
            "width=0.8",
            "clearance=0.3",
            "via_diameter=0.6",
            "via_hole=0.3",
            "current=3",
        ],
    ));
    assert_eq!(answer["kind"], json!("net_parameters"));
    assert_eq!(answer["net"], json!("CLK_P"));
    let Some(SchDirective::NetParameters {
        net_class, rules, ..
    }) = doc(&app).directives.last().cloned()
    else {
        panic!("a Net Parameters directive: {:?}", doc(&app).directives);
    };
    assert_eq!(net_class.as_deref(), Some("POWER_IN"));
    assert_eq!(
        (rules.width_mm, rules.clearance_mm, rules.current_a),
        (Some(0.8), Some(0.3), Some(3.0))
    );
    assert_eq!(
        signex_net::build_netlist(doc(&app)).nets[0].class,
        Some(signex_types::net::NetClassId("POWER_IN".into())),
        "Net Parameters outrank the Blanket"
    );
    let refused = run(
        &mut app,
        "place_parameter_set",
        &[&format!("at={}", pt(grid(15.0, 10.0))), "width=wide"],
    );
    assert!(
        !matches!(refused, CommandOutcome::Answered(_)),
        "{refused:?}"
    );
    undo(&mut app);
    assert_eq!(doc(&app).directives.len(), 1, "one undo step");
}
