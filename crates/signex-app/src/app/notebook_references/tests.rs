//! Notebook references in the app (mockup rows 196 and 197): the commands
//! read first and answer from that read, the cards and the picker lay out
//! and send their messages, the badges switch, and a fix waits for the
//! notebook tab.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use signex_i18n::{I18n, RenderTranslatable};
use signex_notebook::{
    BadgeTarget, CardAction, DesignIndex, DesignNet, DesignPart, DesignPin, PinName, Reference,
    badge_of, picker_entries, reference_card,
};
use signex_types::theme::{ThemeId, theme_tokens};

use super::commands::reference_operand;
use super::views::{badge_card, picker_list, reference_card as card_view, summary_line};
use super::{NotebookReferenceMsg, of_text};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::layout_probe::{find, lay_out_texts, messages_on_click};
use crate::panels::annotations::NotebookScan;

const WINDOW: iced::Size = iced::Size::new(420.0, 600.0);

/// R7 was renamed R17; U7 is gone.
const PAGE: &str = "+++\n[[references]]\nkind = \"component\"\ncomponent = \"R7\"\nid = \"00000000-0000-0000-0000-000000000007\"\n+++\n#component(\"L1\") on #net(\"SW\"), set by #component(\"R7\"); #component(\"U7\") was removed.\n#annotation(kind: \"issue\", status: \"open\", title: \"Warm\")[#component(\"L1\") runs warm]\n";

fn design() -> DesignIndex {
    let part = |designator: &str, id: u128, value: &str, pins: Vec<DesignPin>| DesignPart {
        designator: designator.into(),
        id: uuid::Uuid::from_u128(id),
        value: value.into(),
        description: String::new(),
        footprint: "IND_4040".into(),
        sheet: "power.snxsch".into(),
        fitted_in: Vec::new(),
        variant_count: 0,
        pins,
    };
    let pin = |number: &str, net: &str| DesignPin {
        number: number.into(),
        name: String::new(),
        net: Some(net.into()),
    };
    DesignIndex::new(
        vec![
            part("L1", 1, "4.7µH", vec![pin("1", "SW")]),
            part("R17", 7, "100k", vec![pin("1", "FB")]),
        ],
        vec![DesignNet {
            name: "SW".into(),
            pins: vec![PinName {
                component: "L1".into(),
                pin: "1".into(),
            }],
            sheets: vec!["power.snxsch".into()],
        }],
    )
}

fn scan() -> NotebookScan {
    let path = Path::new("/p/notebook/design.snxnbk");
    let (annotations, problems) = signex_notebook::scan(PAGE);
    NotebookScan {
        folder_found: true,
        files: vec![crate::panels::annotations::NotebookFile {
            path: path.to_path_buf(),
            label: "notebook/design.snxnbk".into(),
            annotations,
            problems,
            read_error: None,
        }],
        design: Some(Arc::new(design())),
        references: vec![of_text(path, PAGE, &design())],
        ..NotebookScan::default()
    }
}

fn app() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-nb-refs-{}", uuid::Uuid::now_v7()));
    app_workspace(&dir.to_string_lossy(), &["power.snxsch"])
}

fn args(app: &Signex, id: &str, operands: &[&str]) -> (AppCommandId, CommandArgs) {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    (command, args)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let (command, args) = args(app, id, operands);
    app.run_operands(&command, &args).0
}

/// The answer once the read landed: the run again answers from it.
fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    let project = app.document_state.projects[0].id;
    let dir = app.document_state.projects[0].dir().to_path_buf();
    let state = &mut app.ui_state.annotations;
    state.target = Some((project, dir));
    state.scan = Some(Arc::new(scan()));
    state.badges = Arc::new(crate::panels::annotations::notebook_badges(&scan()));
    state.answering = true;
    let outcome = run(app, id, operands);
    app.ui_state.annotations.answering = false;
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn i18n() -> Arc<I18n> {
    I18n::en_us()
}

#[test]
fn a_reference_is_named_by_kind_and_name() {
    let app = app();
    let parsed = |operands: &[&str]| {
        let (_, args) = args(&app, "notebook_reference_card", operands);
        reference_operand(&args)
    };
    assert_eq!(
        parsed(&["kind=component", "name=R7"]),
        Ok(Reference::Component("R7".into()))
    );
    assert_eq!(
        parsed(&["kind=pin", "name=U3.3"]),
        Ok(Reference::Pin {
            component: "U3".into(),
            pin: "3".into()
        })
    );
    assert_eq!(
        parsed(&["kind=net", "name=+3V3"]),
        Ok(Reference::Net("+3V3".into()))
    );
    assert!(parsed(&["kind=pin", "name=U3"]).is_err(), "a pin is dotted");
    assert!(parsed(&["kind=sheet", "name=x"]).is_err());
}

#[test]
fn a_command_reads_the_notebook_first_and_runs_again_when_it_lands() {
    let mut app = app();
    let generation = app.ui_state.annotations.generation;
    let outcome = run(&mut app, "notebook_references", &[]);
    assert!(
        matches!(outcome, CommandOutcome::Deferred { .. }),
        "{outcome:?}"
    );
    let state = &app.ui_state.annotations;
    assert_eq!(state.generation, generation + 1, "a read started");
    assert_eq!(state.reruns.len(), 1, "the command waits for it");
    assert_eq!(state.reruns[0].command.as_str(), "notebook_references");
    let _ = app.rerun_waiting_notebook_commands();
    assert!(app.ui_state.annotations.reruns.is_empty());
    assert!(!app.ui_state.annotations.answering);
}

#[test]
fn notebook_references_answers_each_reference_its_state_and_fixes() {
    let mut app = app();
    let value = answer(&mut app, "notebook_references", &[]);
    let file = &value["files"][0];
    assert_eq!(file["file"], "design.snxnbk");
    assert_eq!(file["stale"], 2, "R7 renamed, U7 deleted");
    let refs = file["references"].as_array().expect("a list");
    let named = |name: &str| {
        refs.iter()
            .find(|r| r["reference"]["name"] == name)
            .unwrap_or_else(|| panic!("{name} in {refs:?}"))
    };
    let r7 = named("R7");
    assert_eq!(r7["state"], "renamed");
    assert_eq!(r7["now"]["name"], "R17");
    assert_eq!(r7["uses"], 1);
    assert_eq!(r7["fixes"][0]["fix"], "update");
    let u7 = named("U7");
    assert_eq!(u7["state"], "deleted");
    assert_eq!(file["update_all"][0]["fix"]["fix"], "update_to");
}

#[test]
fn notebook_reference_card_answers_the_hover_card() {
    let mut app = app();
    let value = answer(
        &mut app,
        "notebook_reference_card",
        &["kind=component", "name=L1"],
    );
    assert_eq!(value["facts"]["kind"], "part");
    assert_eq!(value["facts"]["footprint"], "IND_4040");
    assert_eq!(value["blocks"][0]["title"], "Warm");
    assert_eq!(value["actions"][0], "cross_probe");
}

#[test]
fn notebook_reference_picker_answers_what_at_lists() {
    let mut app = app();
    let value = answer(&mut app, "notebook_reference_picker", &["text=r1"]);
    assert_eq!(value["entries"][0]["insert"], "#component(\"R17\")");
    let (_, args) = args(&app, "notebook_reference_picker", &["limit=0"]);
    let (outcome, _) = super::commands::notebook_reference_picker(&mut app, &args);
    assert!(matches!(outcome, CommandOutcome::Disabled { .. }));
}

#[test]
fn notebook_badges_answers_every_badge_or_one_parts_design_notes() {
    let mut app = app();
    let all = answer(&mut app, "notebook_badges", &[]);
    assert_eq!(all["shown"], true, "on by default");
    assert_eq!(all["badges"][0]["target"]["name"], "L1");
    assert_eq!(all["badges"][0]["tone"], "open_issue");
    let one = answer(&mut app, "notebook_badges", &["part=L1"]);
    assert_eq!(one["badge"]["blocks"][0]["title"], "Warm");
}

#[test]
fn a_fix_waits_for_the_notebook_tab_and_says_so() {
    let mut app = app();
    let (command, args) = args(
        &app,
        "notebook_fix_reference",
        &["fix=update", "kind=component", "name=R7"],
    );
    app.ui_state.annotations.answering = true;
    app.ui_state.annotations.scan = Some(Arc::new(scan()));
    let project = app.document_state.projects[0].id;
    let dir = app.document_state.projects[0].dir().to_path_buf();
    app.ui_state.annotations.target = Some((project, dir));
    let (outcome, _) = app.run_operands(&command, &args);
    app.ui_state.annotations.answering = false;
    match outcome {
        CommandOutcome::Disabled { reason } => {
            assert!(reason.render_en_us().contains("Design Notebook first"));
        }
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn b_shows_and_hides_the_badges() {
    let mut app = app();
    assert!(!app.ui_state.annotations.badges_hidden);
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::NotebookReferences(NotebookReferenceMsg::BadgesToggled),
    )));
    assert!(app.ui_state.annotations.badges_hidden);
    let _ = app.handle_notebook_reference_message(NotebookReferenceMsg::BadgesToggled);
    assert!(!app.ui_state.annotations.badges_hidden);
}

fn card(reference: Reference) -> signex_notebook::ReferenceCard {
    let scan = scan();
    let report = &scan.references[0].report;
    let entry = report.get(&reference).expect("used").clone();
    let badges = crate::panels::annotations::notebook_badges(&scan);
    reference_card(&entry, &design(), &badges, report.stale_count())
}

#[test]
fn a_part_card_lays_out_its_facts_and_actions() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let card = card(Reference::Component("L1".into()));
    let texts = lay_out_texts(
        card_view(&card, "design.snxnbk", 2, &i18n(), &tokens),
        WINDOW,
    );
    find(&texts, "L1");
    find(&texts, "Footprint");
    find(&texts, "IND_4040");
    find(&texts, "Cross-probe");
    find(&texts, "Open sheet");
    find(&texts, "Timeline");
    let pressed = messages_on_click(
        card_view(&card, "design.snxnbk", 2, &i18n(), &tokens),
        WINDOW,
        "Cross-probe",
    );
    assert_eq!(
        pressed,
        [NotebookReferenceMsg::CardActionPicked {
            reference: Reference::Component("L1".into()),
            action: CardAction::CrossProbe,
        }]
    );
}

/// A message with arguments, as the view lays it out (Fluent isolates the
/// arguments).
fn shown(id: signex_i18n::MessageId, args: &[(&str, signex_i18n::Arg)]) -> String {
    i18n().message(id, args).into_string()
}

/// `text` without Fluent's isolation marks.
fn plain(text: String) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_renamed_reference_card_says_so_and_offers_update() {
    use signex_i18n::ids;
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let card = card(Reference::Component("R7".into()));
    let texts = lay_out_texts(
        card_view(&card, "design.snxnbk", 2, &i18n(), &tokens),
        WINDOW,
    );
    let reason = shown(
        ids::NOTEBOOK_CARD_STALE_RENAMED,
        &[("old", "R7".into()), ("new", "R17".into())],
    );
    assert_eq!(
        plain(reason.clone()),
        "R7 was renamed to R17 in the schematic"
    );
    find(&texts, &reason);
    let update = shown(ids::NOTEBOOK_ACTION_UPDATE_TO, &[("name", "R17".into())]);
    assert_eq!(plain(update.clone()), "Update to R17");
    find(&texts, &update);
    let all = shown(ids::NOTEBOOK_ACTION_UPDATE_ALL, &[("count", 2usize.into())]);
    assert_eq!(plain(all.clone()), "Update all 2 stale…");
    find(&texts, &all);
    let pressed = messages_on_click(
        card_view(&card, "design.snxnbk", 2, &i18n(), &tokens),
        WINDOW,
        &update,
    );
    assert_eq!(
        pressed,
        [NotebookReferenceMsg::FixPicked {
            file: "design.snxnbk".into(),
            reference: Reference::Component("R7".into()),
            fix: signex_notebook::Fix::UpdateTo {
                to: Reference::Component("R17".into())
            },
        }]
    );
}

#[test]
fn a_deleted_reference_card_offers_remove_and_keep_as_text() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let card = card(Reference::Component("U7".into()));
    let texts = lay_out_texts(
        card_view(&card, "design.snxnbk", 2, &i18n(), &tokens),
        WINDOW,
    );
    let reason = shown(
        signex_i18n::ids::NOTEBOOK_CARD_STALE_DELETED,
        &[("name", "U7".into())],
    );
    assert_eq!(plain(reason.clone()), "U7 is not in the design");
    find(&texts, &reason);
    find(&texts, "Remove reference");
    find(&texts, "Keep as plain text");
}

#[test]
fn a_badge_card_lists_its_blocks_and_opens_the_first() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let badges = crate::panels::annotations::notebook_badges(&scan());
    let badge = badge_of(&badges, &BadgeTarget::Part("L1".into())).expect("L1");
    let texts = lay_out_texts(badge_card(badge, &i18n(), &tokens), WINDOW);
    let title = shown(
        signex_i18n::ids::NOTEBOOK_BADGE_CARD_TITLE,
        &[("name", "L1".into()), ("count", 1usize.into())],
    );
    assert_eq!(plain(title.clone()), "L1 · 1 block");
    find(&texts, &title);
    find(&texts, "An issue is open");
    find(&texts, "Warm");
    let pressed = messages_on_click(badge_card(badge, &i18n(), &tokens), WINDOW, "Warm");
    assert_eq!(
        pressed,
        [NotebookReferenceMsg::BlockOpened {
            file: "design.snxnbk".into(),
            line: 8
        }]
    );
}

#[test]
fn the_picker_lists_entries_and_a_click_picks_one() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let entries = picker_entries(&design(), "", 10);
    let texts = lay_out_texts(picker_list(&entries, 0, &i18n(), &tokens), WINDOW);
    find(&texts, "L1");
    find(&texts, "4.7µH");
    let picked = messages_on_click(picker_list(&entries, 0, &i18n(), &tokens), WINDOW, "R17");
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0].insert, "#component(\"R17\")");
    let none = lay_out_texts(picker_list(&[], 0, &i18n(), &tokens), WINDOW);
    find(&none, "No part, pin or net matches");
}

#[test]
fn the_status_line_counts_references_and_stale_ones() {
    let scan = scan();
    let line = summary_line(&i18n(), &scan.references[0].report).into_string();
    assert_eq!(
        plain(line),
        "5 references · 2 stale",
        "the block's use counts"
    );
    let clean = of_text(
        &PathBuf::from("/p/notebook/a.snxnbk"),
        "#component(\"L1\")",
        &design(),
    );
    assert_eq!(
        plain(summary_line(&i18n(), &clean.report).into_string()),
        "1 reference · all resolve"
    );
}
