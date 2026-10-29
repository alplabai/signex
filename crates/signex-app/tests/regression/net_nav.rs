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
    assert!(app.document_state.net_nav.highlight.is_none());

    let _ = app.update(Message::Ui(UiMsg::NetNav(
        NetNavMsg::HighlightClearRequested,
    )));

    assert!(app.document_state.net_nav.highlight.is_none());
}

// ── update-level flows over real sheets ──────────────────────────────

mod flows {
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
    fn open_project(dir: &Path) -> (Signex, PathBuf, PathBuf) {
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
                unknown_columns: Default::default(),
                uuid: Uuid::from_u128(label),
                text: "VCC".into(),
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                label_type: LabelType::Global,
                shape: String::new(),
                font_size: 1.27,
                justify: HAlign::default(),
                justify_v: VAlign::default(),
                font_family: None,
                locked: false,
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
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
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
            let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
                path: path.clone(),
                title: "t".into(),
                result: Ok(Box::new(sheet)),
            }));
        }
        assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
        (app, a, b)
    }

    fn nav(app: &mut Signex, msg: NetNavMsg) {
        let _ = app.update(Message::Ui(UiMsg::NetNav(msg)));
        app.settle_net_index_blocking();
    }

    fn item(label: u128) -> SelectedItem {
        SelectedItem::new(Uuid::from_u128(label), SelectedKind::Label)
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
            .document_state
            .net_nav
            .hover
            .candidate
            .as_mut()
            .expect("a candidate");
        candidate.since = std::time::Instant::now() - std::time::Duration::from_secs(5);

        nav(&mut app, NetNavMsg::HoverDwellTicked);

        let card = app
            .document_state
            .net_nav
            .hover
            .card
            .as_ref()
            .expect("a card");
        assert_eq!(card.name, "VCC");
    }
}
