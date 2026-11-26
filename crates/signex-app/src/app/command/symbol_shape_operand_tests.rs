//! #2838: the symbol editor's curve tools with operands. `symbol_draw_*`
//! place the shape the tool's clicks place, as one undo step of the symbol's
//! editor, answer with the graphic, and refuse a shape that draws nothing.
//! The person's way is the Shapes tools, armed from the Active Bar.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_i18n::RenderTranslatable as _;
use signex_library::{Symbol, SymbolFile, SymbolGraphicKind};
use signex_types::theme::{ThemeId, theme_tokens};

use super::enablement::{disabled_reason, is_enabled};
use super::{CommandArgs, CommandContext, CommandOutcome};
use crate::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::keymap::AppCommandId;
use crate::library::editor::symbol::{SymActiveBarMenu, SymbolTool};
use crate::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg};

const IDS: [&str; 4] = [
    "symbol_draw_elliptical_arc",
    "symbol_draw_bezier",
    "symbol_draw_round_rectangle",
    "symbol_draw_pie",
];

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a catalog id")
}

/// The main window on a `.snxsym` tab of an empty symbol.
fn symbol_app() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = std::env::temp_dir().join("shape-operands.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("T"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "shapes".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, path)
}

fn editor<'a>(app: &'a Signex, path: &Path) -> &'a SymbolDocument {
    &app.document_state.symbol_editors[path]
}

fn graphics(app: &Signex, path: &Path) -> Vec<SymbolGraphicKind> {
    editor(app, path)
        .primitive()
        .graphics
        .iter()
        .map(|g| g.kind.clone())
        .collect()
}

/// `name` with `document` and `operands`, as the CLI runs it.
fn headless(app: &mut Signex, path: &Path, name: &str, operands: &[&str]) -> CommandOutcome {
    let command = id(name);
    let document = format!("document={}", path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

fn send(app: &mut Signex, path: &Path, msg: SymbolEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Symbol(msg),
    }));
}

/// One shape: its operands, the Active Bar row and tool a person picks, and
/// the clicks the tool turns into the same shape.
struct Case {
    id: &'static str,
    operands: [&'static str; 5],
    row: &'static str,
    tool: SymbolTool,
    kind: &'static str,
    clicks: fn() -> Vec<SymbolEditorMsg>,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "symbol_draw_elliptical_arc",
            operands: [
                "center=1.27,2.54",
                "radius_x=5.08",
                "radius_y=2.54",
                "start_angle=10",
                "end_angle=100",
            ],
            row: "Elliptical Arc",
            tool: SymbolTool::PlaceEllipticalArc,
            kind: "elliptical_arc",
            clicks: || {
                vec![SymbolEditorMsg::AddEllipticalArc {
                    cx: 1.27,
                    cy: 2.54,
                    radius_x: 5.08,
                    radius_y: 2.54,
                    start_deg: 10.0,
                    end_deg: 100.0,
                }]
            },
        },
        Case {
            id: "symbol_draw_bezier",
            operands: [
                "start=0,0",
                "control_1=2.54,2.54",
                "control_2=5.08,2.54",
                "end=7.62,0",
                "",
            ],
            row: "Bezier",
            tool: SymbolTool::PlaceBezier,
            kind: "bezier",
            clicks: || {
                [(0.0, 0.0), (2.54, 2.54), (5.08, 2.54), (7.62, 0.0)]
                    .into_iter()
                    .map(|(x, y)| SymbolEditorMsg::BezierClick { x, y })
                    .collect()
            },
        },
        Case {
            id: "symbol_draw_round_rectangle",
            operands: ["start=0,0", "end=5.08,2.54", "", "", ""],
            row: "Round Rectangle",
            tool: SymbolTool::PlaceRoundRectangle,
            kind: "round_rectangle",
            clicks: || {
                vec![SymbolEditorMsg::AddRoundRectangle {
                    from_x: 0.0,
                    from_y: 0.0,
                    to_x: 5.08,
                    to_y: 2.54,
                }]
            },
        },
        Case {
            id: "symbol_draw_pie",
            operands: [
                "center=1.27,1.27",
                "radius=2.54",
                "start_angle=30",
                "end_angle=120",
                "",
            ],
            row: "Pie Chart",
            tool: SymbolTool::PlacePie,
            kind: "pie",
            clicks: || {
                vec![SymbolEditorMsg::AddPie {
                    cx: 1.27,
                    cy: 1.27,
                    radius: 2.54,
                    start_deg: 30.0,
                    end_deg: 120.0,
                }]
            },
        },
    ]
}

impl Case {
    fn operands(&self) -> Vec<&'static str> {
        self.operands
            .iter()
            .copied()
            .filter(|op| !op.is_empty())
            .collect()
    }
}

/// The person's way, through `Signex::update`: open the Shapes menu, click
/// the shape's row in the laid-out menu, then the tool's clicks.
fn drawn_by_hand(case: &Case) -> (Signex, PathBuf) {
    let (mut app, path) = symbol_app();
    send(
        &mut app,
        &path,
        SymbolEditorMsg::ToggleActiveBarMenu(SymActiveBarMenu::Shapes),
    );
    let published = {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let i18n = signex_i18n::I18n::en_us();
        let menu = crate::library::editor::symbol::active_bar::dropdown_overlay(
            editor(&app, &path),
            ThemeId::GraphiteDark,
            &tokens,
            0,
            &i18n,
        )
        .expect("the Shapes menu is open");
        crate::layout_probe::messages_on_click(menu, Size::new(900.0, 700.0), case.row)
    };
    assert!(!published.is_empty(), "{}: the row publishes", case.row);
    for msg in published {
        let _ = app.update(Message::Library(msg));
    }
    assert_eq!(editor(&app, &path).tool, case.tool, "{}", case.row);
    for click in (case.clicks)() {
        send(&mut app, &path, click);
    }
    (app, path)
}

#[test]
fn every_shape_command_draws_what_the_tool_draws_by_hand() {
    for case in cases() {
        let (by_hand, hand_path) = drawn_by_hand(&case);
        let (mut app, path) = symbol_app();
        let outcome = headless(&mut app, &path, case.id, &case.operands());
        let CommandOutcome::Answered(answer) = outcome else {
            panic!("{}: {outcome:?}", case.id);
        };
        assert_eq!(answer["kind"], case.kind, "{}", case.id);
        assert_eq!(answer["index"], 0, "{}", case.id);
        assert_eq!(
            answer["document"],
            path.display().to_string().as_str(),
            "{}",
            case.id
        );
        let placed = graphics(&app, &path);
        assert_eq!(placed.len(), 1, "{}", case.id);
        assert_eq!(placed, graphics(&by_hand, &hand_path), "{}", case.id);
        let (a, b) = (
            &editor(&app, &path).primitive().graphics[0],
            &editor(&by_hand, &hand_path).primitive().graphics[0],
        );
        assert_eq!(a, b, "{}: stroke, fill, part and body", case.id);
    }
}

#[test]
fn a_shape_from_operands_is_one_undo_step_and_marks_the_symbol_unsaved() {
    for case in cases() {
        let (mut app, path) = symbol_app();
        assert!(!app.document_state.is_document_dirty(&path));
        let outcome = headless(&mut app, &path, case.id, &case.operands());
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{outcome:?}"
        );
        assert_eq!(editor(&app, &path).history.undo_len(), 1, "{}", case.id);
        assert!(app.document_state.is_document_dirty(&path), "{}", case.id);
        let (undone, _task) =
            app.dispatch_command_reporting(&id("undo"), CommandArgs::none(), None);
        assert_eq!(undone, CommandOutcome::Ran, "{}", case.id);
        assert!(graphics(&app, &path).is_empty(), "{}", case.id);
    }
}

#[test]
fn a_shape_from_operands_saves_and_reloads_as_drawn() {
    for case in cases() {
        let (mut app, path) = symbol_app();
        let _ = headless(&mut app, &path, case.id, &case.operands());
        let text = editor(&app, &path).file.to_toml_string().expect("saves");
        let back = SymbolFile::from_toml_str(&text).expect("reloads");
        let kinds: Vec<SymbolGraphicKind> = back.symbols[0]
            .graphics
            .iter()
            .map(|g| g.kind.clone())
            .collect();
        assert_eq!(kinds, graphics(&app, &path), "{}", case.id);
    }
}

#[test]
fn the_shape_lands_on_the_part_the_editor_shows() {
    let (mut app, path) = symbol_app();
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .expect("editor")
        .active_part = 2;
    let outcome = headless(
        &mut app,
        &path,
        "symbol_draw_pie",
        &["center=0,0", "radius=2.54", "start_angle=0", "end_angle=90"],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(answer["part"], 2);
    assert_eq!(editor(&app, &path).primitive().graphics[0].part_number, 2);
}

/// A sweep that wraps through 0 is kept as asked: counter-clockwise from the
/// start to the end, never swapped into the other arc.
#[test]
fn a_sweep_through_zero_keeps_its_start_and_end() {
    let (mut app, path) = symbol_app();
    let _ = headless(
        &mut app,
        &path,
        "symbol_draw_pie",
        &[
            "center=0,0",
            "radius=2.54",
            "start_angle=300",
            "end_angle=-330",
        ],
    );
    let SymbolGraphicKind::Pie {
        start_deg, end_deg, ..
    } = graphics(&app, &path)[0]
    else {
        panic!("a pie");
    };
    assert_eq!((start_deg, end_deg), (300.0, 30.0));
}

#[test]
fn a_corner_radius_is_held_to_half_the_short_side() {
    let (mut app, path) = symbol_app();
    let _ = headless(
        &mut app,
        &path,
        "symbol_draw_round_rectangle",
        &["start=5.08,2.54", "end=0,0", "corner_radius=10"],
    );
    assert_eq!(
        graphics(&app, &path),
        [SymbolGraphicKind::RoundRectangle {
            from: [0.0, 0.0],
            to: [5.08, 2.54],
            corner_radius: 1.27,
        }]
    );
    let (mut app, path) = symbol_app();
    let _ = headless(
        &mut app,
        &path,
        "symbol_draw_round_rectangle",
        &["start=0,0", "end=5.08,2.54", "corner_radius=0.5"],
    );
    assert!(matches!(
        graphics(&app, &path)[0],
        SymbolGraphicKind::RoundRectangle { corner_radius, .. } if corner_radius == 0.5
    ));
}

#[test]
fn a_shape_that_draws_nothing_is_refused_with_nothing_changed() {
    for (name, operands) in [
        (
            "symbol_draw_pie",
            vec![
                "center=0,0",
                "radius=2.54",
                "start_angle=30",
                "end_angle=30",
            ],
        ),
        (
            "symbol_draw_elliptical_arc",
            vec![
                "center=0,0",
                "radius_x=2",
                "radius_y=1",
                "start_angle=45",
                "end_angle=-315",
            ],
        ),
        (
            "symbol_draw_bezier",
            vec!["start=1,1", "control_1=1,1", "control_2=1,1", "end=1,1"],
        ),
        ("symbol_draw_round_rectangle", vec!["start=0,0", "end=5,0"]),
    ] {
        let (mut app, path) = symbol_app();
        let outcome = headless(&mut app, &path, name, &operands);
        let CommandOutcome::Disabled { reason } = outcome else {
            panic!("{name}: {outcome:?}");
        };
        assert!(
            reason.render_en_us().contains("no visible size"),
            "{name}: {}",
            reason.render_en_us()
        );
        assert!(graphics(&app, &path).is_empty(), "{name}");
        assert_eq!(editor(&app, &path).history.undo_len(), 0, "{name}");
        assert!(!app.document_state.is_document_dirty(&path), "{name}");
    }
}

#[test]
fn a_pin_reference_or_another_document_is_refused() {
    let (mut app, path) = symbol_app();
    let outcome = headless(
        &mut app,
        &path,
        "symbol_draw_pie",
        &[
            "center=R1.2",
            "radius=2.54",
            "start_angle=0",
            "end_angle=90",
        ],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("not a pin reference")),
        "{outcome:?}"
    );
    let elsewhere = std::env::temp_dir().join("not-open.snxsym");
    let outcome = headless(
        &mut app,
        &elsewhere,
        "symbol_draw_pie",
        &["center=0,0", "radius=2.54", "start_angle=0", "end_angle=90"],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("not an open or referenced document")),
        "{outcome:?}"
    );
    assert!(graphics(&app, &path).is_empty());
}

/// The gate: a symbol editor tab. A key, the palette or a menu row gives no
/// operands, so the command says where the tools are.
#[test]
fn the_commands_need_a_symbol_editor_and_say_where_the_tools_are() {
    let off = CommandContext::default();
    let on = CommandContext {
        has_symbol_editor: true,
        ..off
    };
    let en = signex_i18n::I18n::en_us();
    for name in IDS {
        assert!(!is_enabled(&id(name), &off), "{name}");
        assert!(is_enabled(&id(name), &on), "{name}");
        let reason = disabled_reason(&id(name), &off).expect("a reason");
        assert_eq!(en.message_plain(reason, &[]), "No symbol editor is open");
    }
    let (mut app, _path) = symbol_app();
    for name in IDS {
        let (outcome, _task) = app.dispatch_command_reporting(&id(name), CommandArgs::none(), None);
        let CommandOutcome::Disabled { reason } = outcome else {
            panic!("{name}: {outcome:?}");
        };
        assert!(
            reason
                .render_en_us()
                .contains("use the Shapes tools of the symbol editor"),
            "{name}"
        );
    }
}
