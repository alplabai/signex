//! Net navigation (#1270, #1271, #1272): the hover preference and the
//! highlight state through the public dispatcher.

use signex_app::app::{Message, NetNavMsg, Signex, UiMsg};
use std::fs;
use tempfile::TempDir;

#[test]
fn the_net_hover_preference_defaults_on_and_round_trips_through_prefs_json() {
    assert!(signex_app::fonts::read_net_hover_pref_from(None));
    let key_missing = serde_json::json!({ "theme": "signex" });
    assert!(signex_app::fonts::read_net_hover_pref_from(Some(
        &key_missing
    )));

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("prefs.json");
    signex_app::fonts::write_net_hover_pref_at(&path, false);
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(!signex_app::fonts::read_net_hover_pref_from(Some(&saved)));
}

#[test]
fn a_new_session_has_the_hover_on_and_no_highlight_and_clearing_changes_nothing() {
    let (mut app, _boot) = Signex::new();
    assert!(app.ui_state.net_hover_enabled);
    assert!(app.document_view.net_nav.highlight.is_none());

    let _ = app.update(Message::Ui(UiMsg::NetNav(
        NetNavMsg::HighlightClearRequested,
    )));

    assert!(app.document_view.net_nav.highlight.is_none());
}

// ── update-level flows over real sheets ──────────────────────────────

pub(crate) mod flows {
    use super::*;
    use signex_app::app::{CaseFold, FileMsg, LoadedProject, Signex};
    use signex_app::tab_bar::TabMessage;
    use signex_types::project::{ProjectData, SheetEntry};
    use signex_types::schematic::{
        HAlign, Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind, VAlign, Wire,
    };
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    const LABEL_A: u128 = 1;
    const LABEL_B: u128 = 2;

    /// Two sheets of one project, each a wire with the global label `VCC`,
    /// both open as tabs with `a` focused.
    pub(crate) fn open_project(dir: &Path) -> (Signex, PathBuf, PathBuf) {
        let sheet = |label: u128, wire: u128| {
            let mut s = SchematicSheet {
                uuid: Uuid::now_v7(),
                version: 1,
                generator: "signex-test".into(),
                generator_version: "0.9".into(),
                paper_size: "A4".into(),
                ..Default::default()
            };
            s.wires.push(Wire {
                uuid: Uuid::from_u128(wire),
                start: Point::new(0.0, 0.0),
                end: Point::new(5.0, 0.0),
                ..Default::default()
            });
            s.labels.push(Label {
                uuid: Uuid::from_u128(label),
                justify: HAlign::default(),
                justify_v: VAlign::default(),
                ..Label::new("VCC", Point::new(0.0, 0.0), LabelType::Global)
            });
            s
        };
        let write = |name: &str, s: SchematicSheet| {
            let path = dir.join(name);
            let text = signex_types::format::SnxSchematic::new(s)
                .write_string()
                .expect("encode");
            fs::write(&path, text).expect("write");
            path
        };
        let a = write("a.snxsch", sheet(LABEL_A, 11));
        let b = write("b.snxsch", sheet(LABEL_B, 12));

        let (mut app, _boot) = Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: dir.join("P.snxprj"),
            data: ProjectData {
                document_id: None,
                description: None,
                name: "P".into(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: Some("a.snxsch".into()),
                pcb_file: None,
                sheets: ["a.snxsch", "b.snxsch"]
                    .iter()
                    .map(|f| SheetEntry {
                        name: (*f).into(),
                        filename: (*f).into(),
                        symbols_count: 0,
                        wires_count: 0,
                        labels_count: 0,
                        unknown: Default::default(),
                    })
                    .collect(),
                variant_definitions: Vec::new(),
                active_variant: None,
                variant_settings: Default::default(),
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                schematic_defaults: Default::default(),
                project_defaults: Default::default(),
                agent_journal: Default::default(),
                schematic_design: Default::default(),
                fab_outputs: None,
                router_tool_diameter: None,
                bom_alternates: Vec::new(),
                requirements: Default::default(),
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: HashMap::new(),
            case_fold: CaseFold::default(),
            case_probed: false,
            case_decided: false,
        });
        app.document_state.active_project = Some(id);
        let window = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(window);
        for path in [&b, &a] {
            let text = fs::read_to_string(path).expect("read");
            let sheet = signex_types::format::SnxSchematic::parse(&text)
                .expect("parse")
                .sheet;
            let task = app.update(Message::File(FileMsg::SchematicOpenFinished {
                path: path.clone(),
                title: "t".into(),
                result: Ok(Box::new(sheet)),
                disk: None,
            }));
            land_index_builds(&mut app, task);
        }
        assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
        (app, a, b)
    }

    /// Land the net index builds `task` started, and only those: opening a
    /// sheet with a Global label starts the build its cross references read
    /// (#1273). A build left in flight holds the project's one worker slot,
    /// so every later build would wait on it forever.
    fn land_index_builds(app: &mut Signex, task: iced::Task<Message>) {
        let mut driver = signex_app::task_driver::TaskDriver::new();
        let mut queue = std::collections::VecDeque::from([task]);
        while let Some(task) = queue.pop_front() {
            for message in driver.outputs(task).expect("the task chain settled") {
                if matches!(
                    message,
                    Message::Ui(UiMsg::NetNav(NetNavMsg::IndexResolved(_)))
                ) {
                    queue.push_back(app.update(message));
                }
            }
        }
    }

    fn nav(app: &mut Signex, msg: NetNavMsg) {
        let _ = app.update(Message::Ui(UiMsg::NetNav(msg)));
        app.settle_net_index_blocking();
    }

    fn item(label: u128) -> SelectedItem {
        SelectedItem::new(Uuid::from_u128(label), SelectedKind::Label)
    }

    /// #2106: Select > Connection takes the first wire-like item as its seed,
    /// so a symbol ahead of the wire in the selection does not block it.
    #[test]
    fn select_connection_seeds_from_the_wire_after_a_symbol() {
        use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
        let tmp = TempDir::new().unwrap();
        let (mut app, _a, _b) = open_project(tmp.path());
        let symbol = SelectedItem::new(Uuid::from_u128(99), SelectedKind::Symbol);
        let wire = SelectedItem::new(Uuid::from_u128(11), SelectedKind::Wire);
        app.interaction_state.active_canvas_mut().selected = vec![symbol, wire];

        let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
            ActiveBarAction::SelectConnection,
        )));

        assert_eq!(
            app.interaction_state.active_canvas().selected,
            vec![wire, item(LABEL_A)]
        );
    }

    fn run_command(app: &mut Signex, id: &str) {
        let _ = app.update(Message::Menu(signex_app::menu_bar::MenuMessage::Command(
            signex_app::keymap::AppCommandId::new(id).expect("a catalog id"),
        )));
    }

    fn cursor_at(app: &mut Signex, x: f32, y: f32) {
        let _ = app.update(Message::CanvasEvent(
            signex_app::canvas::CanvasEvent::CursorAt {
                x,
                y,
                zoom_pct: 100.0,
            },
        ));
    }

    /// #1668: `select_expand_connection` is Select > Connection under the
    /// Classic profile's name: the selected wire grows to its net.
    #[test]
    fn select_expand_connection_expands_the_selected_wire_to_its_net() {
        let tmp = TempDir::new().unwrap();
        let (mut app, _a, _b) = open_project(tmp.path());
        let wire = SelectedItem::new(Uuid::from_u128(11), SelectedKind::Wire);
        app.interaction_state.active_canvas_mut().selected = vec![wire];

        run_command(&mut app, "select_expand_connection");

        assert_eq!(
            app.interaction_state.active_canvas().selected,
            vec![wire, item(LABEL_A)]
        );
    }

    const FRAMED_WIRE: u128 = 21;
    const NOTE_AT_40: u128 = 23;
    const NODE_MISS: &str = "Select node / connection item: no wire, bus, junction or label";

    /// One sheet: a wire from (0, 0) to (20, 0) inside a rectangle drawing,
    /// joined at (20, 0) by a second wire of the same net down to (20, 10),
    /// and a text note at (40, 40).
    fn open_framed_wire() -> Signex {
        use signex_types::schematic::{SchDrawing, TextNote};
        let mut sheet = SchematicSheet {
            uuid: Uuid::now_v7(),
            version: 1,
            generator: "signex-test".into(),
            generator_version: "0.9".into(),
            paper_size: "A4".into(),
            ..Default::default()
        };
        sheet.wires.push(Wire {
            uuid: Uuid::from_u128(FRAMED_WIRE),
            start: Point::new(0.0, 0.0),
            end: Point::new(20.0, 0.0),
            ..Default::default()
        });
        // Same net, so expanding the connection would select it too.
        sheet.wires.push(Wire {
            uuid: Uuid::from_u128(24),
            start: Point::new(20.0, 0.0),
            end: Point::new(20.0, 10.0),
            ..Default::default()
        });
        sheet.drawings.push(SchDrawing::Rect {
            uuid: Uuid::from_u128(22),
            start: Point::new(-5.0, -5.0),
            end: Point::new(25.0, 5.0),
            width: 0.25,
            fill: Default::default(),
            stroke_color: None,
            locked: false,
        });
        sheet.text_notes.push(TextNote {
            uuid: Uuid::from_u128(NOTE_AT_40),
            text: "NOTE".into(),
            position: Point::new(40.0, 40.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::default(),
            justify_v: VAlign::default(),
            locked: false,
            format: Default::default(),
            rich: Default::default(),
        });
        let (mut app, _boot) = Signex::new();
        app.ui_state.main_window_id = Some(iced::window::Id::unique());
        let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
            path: PathBuf::from("framed.snxsch"),
            title: "framed".into(),
            result: Ok(Box::new(sheet)),
            disk: None,
        }));
        app
    }

    fn said(fragment: &str) -> usize {
        signex_app::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains(fragment))
            .count()
    }

    fn select_node_at(app: &mut Signex, x: f32, y: f32) -> Vec<SelectedItem> {
        cursor_at(app, x, y);
        run_command(app, "select_node_or_connection_item");
        app.interaction_state.active_canvas().selected.clone()
    }

    /// #1668: `select_node_or_connection_item` selects the wire under the
    /// cursor alone, even inside a drawing that is hit first.
    #[test]
    fn select_node_or_connection_item_selects_the_wire_inside_a_drawing() {
        let mut app = open_framed_wire();

        assert_eq!(
            select_node_at(&mut app, 10.0, 0.0),
            vec![SelectedItem::new(
                Uuid::from_u128(FRAMED_WIRE),
                SelectedKind::Wire
            )]
        );
    }

    /// #1668: over a non-connection item, or over nothing, the command
    /// selects nothing new and says so in Messages.
    #[test]
    fn select_node_or_connection_item_over_no_connection_item_says_so() {
        let _ = signex_app::diagnostics::init_logging();
        let mut app = open_framed_wire();
        let note = SelectedItem::new(Uuid::from_u128(NOTE_AT_40), SelectedKind::TextNote);
        let _ = app.update(Message::CanvasEvent(
            signex_app::canvas::CanvasEvent::Clicked {
                world_x: 40.5,
                world_y: 40.0,
            },
        ));
        assert_eq!(
            app.interaction_state.active_canvas().selected,
            vec![note],
            "precondition: the text note is under (40.5, 40)"
        );
        let kept = SelectedItem::new(Uuid::from_u128(99), SelectedKind::Symbol);
        app.interaction_state.active_canvas_mut().selected = vec![kept];

        for (x, y, what) in [(40.5, 40.0, "a text note"), (80.0, 80.0, "nothing")] {
            let before = said(NODE_MISS);
            assert_eq!(select_node_at(&mut app, x, y), vec![kept], "over {what}");
            assert!(said(NODE_MISS) > before, "over {what}: Messages says why");
        }
    }

    #[test]
    fn go_to_net_partner_jumps_to_the_other_sheet() {
        let tmp = TempDir::new().unwrap();
        let (mut app, _a, b) = open_project(tmp.path());
        app.interaction_state.active_canvas_mut().selected = vec![item(LABEL_A)];

        nav(&mut app, NetNavMsg::PartnerJumpRequested);

        assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
        assert_eq!(
            app.interaction_state.active_canvas().selected,
            vec![item(LABEL_B)]
        );
    }

    #[test]
    fn the_highlight_survives_switching_tabs_and_clears_in_one_step() {
        let tmp = TempDir::new().unwrap();
        let (mut app, _a, _b) = open_project(tmp.path());
        app.interaction_state.active_canvas_mut().selected = vec![item(LABEL_A)];
        nav(&mut app, NetNavMsg::RelatedNetHighlightRequested);
        let on_a = app
            .interaction_state
            .active_canvas()
            .net_highlight()
            .cloned();
        assert!(on_a.is_some_and(|s| s.contains(&Uuid::from_u128(LABEL_A))));

        let window = app.ui_state.main_window_id.expect("window");
        let _ = app.update(Message::Tab {
            window_id: window,
            msg: TabMessage::Select(0),
        });

        let on_b = app
            .interaction_state
            .active_canvas()
            .net_highlight()
            .cloned();
        assert!(
            on_b.is_some_and(|s| s.contains(&Uuid::from_u128(LABEL_B))),
            "the other sheet shows the same net bright after the tab switch"
        );
        nav(&mut app, NetNavMsg::HighlightClearRequested);
        assert!(
            app.interaction_state
                .active_canvas()
                .net_highlight()
                .is_none()
        );
    }

    #[test]
    fn resting_on_a_label_shows_the_net_card_after_the_dwell() {
        let tmp = TempDir::new().unwrap();
        let (mut app, _a, _b) = open_project(tmp.path());
        app.observe_net_hover(Some(item(LABEL_A)), (0.0, 0.0));
        let candidate = app
            .document_view
            .net_nav
            .hover
            .candidate
            .as_mut()
            .expect("a candidate");
        candidate.since = std::time::Instant::now() - std::time::Duration::from_secs(5);

        nav(&mut app, NetNavMsg::HoverDwellTicked);

        let card = app
            .document_view
            .net_nav
            .hover
            .card
            .as_ref()
            .expect("a card");
        assert_eq!(card.name, "VCC");
    }
}
