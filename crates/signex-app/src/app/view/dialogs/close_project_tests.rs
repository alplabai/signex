//! The Close Project confirm's texts (#2262): the intro counts every unsaved
//! row and names the project settings apart, the list is the Exit confirm's
//! grouped list with a kind cap on each row (mockup row 16), and the kept
//! documents say why they stay open.

use std::path::PathBuf;

use signex_i18n::{I18n, ids};

use super::{close_detail, kept_line};
use crate::app::state::ModalId;
use crate::layout_probe::{find, lay_out_texts};

fn plain(text: String) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|n| PathBuf::from("/p").join(n)).collect()
}

#[test]
fn the_detail_counts_the_documents_and_names_the_project_settings_apart() {
    let en = I18n::en_us_ref();
    let detail = |names: &[&str], rows| plain(close_detail(en, "dcdc-buck", &paths(names), rows));
    assert_eq!(
        detail(
            &[
                "power.snxsch",
                "mcu.snxsch",
                "dcdc-buck.snxpcb",
                "dcdc-buck.snxprj"
            ],
            0
        ),
        "'dcdc-buck' has 3 unsaved documents and unsaved project settings."
    );
    assert_eq!(
        detail(&["power.snxsch", "dcdc-buck.snxprj"], 0),
        "'dcdc-buck' has 1 unsaved document and unsaved project settings."
    );
    assert_eq!(
        detail(&["dcdc-buck.snxprj"], 0),
        "'dcdc-buck' has unsaved project settings."
    );
    assert_eq!(
        detail(&["power.snxsch"], 0),
        "'dcdc-buck' has 1 unsaved document."
    );
    assert_eq!(
        detail(&["power.snxsch", "mcu.snxsch"], 0),
        "'dcdc-buck' has 2 unsaved documents."
    );
}

/// Library rows among them make the count "items", as before.
#[test]
fn library_rows_count_as_items() {
    let en = I18n::en_us_ref();
    let detail = |names: &[&str], rows| plain(close_detail(en, "P", &paths(names), rows));
    assert_eq!(detail(&["power.snxsch"], 1), "'P' has 2 unsaved items.");
    assert_eq!(detail(&[], 1), "'P' has 1 unsaved item.");
    assert_eq!(
        detail(&["P.snxprj"], 2),
        "'P' has 2 unsaved items and unsaved project settings."
    );
}

/// Each row says what it is with the kind cap its tab shows, as in the Exit
/// confirm (mockup row 16): a library reads LIB (DECISIONS 2026-11-24), and
/// a plain file of another kind has no cap.
#[test]
fn each_row_says_what_it_is() {
    let en = I18n::en_us_ref();
    let app = app_closing(
        &[
            "power.snxsch",
            "POWER.SNXSCH",
            "dcdc-buck.snxpcb",
            "dcdc-buck.snxprj",
            "symbols/r.snxsym",
            "footprints/r0402.snxfpt",
            "Stock.snxlib",
            "notes.txt",
        ],
        Default::default(),
    );
    let st = app.ui_state.project_close_confirm.as_ref().expect("open");
    let kinds: Vec<(String, Option<String>)> = app
        .quit_entries(&st.dirty_paths, &st.dirty_rows)
        .into_iter()
        .map(|e| (e.name, e.kind.map(|id| en.message_plain(id, &[]))))
        .collect();
    let row = |name: &str, cap: Option<&str>| (name.to_owned(), cap.map(str::to_owned));
    assert_eq!(
        kinds,
        vec![
            row("power.snxsch", Some("SCH")),
            row("POWER.SNXSCH", Some("SCH")),
            row("dcdc-buck.snxpcb", Some("PCB")),
            row("dcdc-buck.snxprj", Some("PRJ")),
            row("r.snxsym", Some("SYM")),
            row("r0402.snxfpt", Some("FPT")),
            row("Stock.snxlib", Some("LIB")),
            row("notes.txt", None),
        ]
    );
    assert_eq!(en.message_plain(ids::PANELS_TAB_KIND_COMPONENT, &[]), "CMP");
}

/// The closing project's files are grouped under its name in capitals with
/// their count, each row with its kind cap and its folder (mockup row 16,
/// reused for Close Project).
#[test]
fn the_list_is_grouped_under_the_project_with_kind_caps() {
    let mut app = app_closing(&["a.snxsch", "Other.snxprj"], Default::default());
    crate::app::handlers::menu::export::tests::add_project(&mut app, "/p", &["a.snxsch"]);
    if let Some(project) = app.document_state.projects.last_mut() {
        project.data.name = "dcdc-buck".into();
    }
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    let folder = std::path::Path::new("/p").display().to_string();
    for label in ["DCDC-BUCK", "SCH", "PRJ", folder.as_str()] {
        find(&texts, label);
    }
    assert!(
        texts.iter().any(|t| plain(t.text.clone()) == "2"),
        "the group counts its two rows: {texts:#?}"
    );
    assert!(
        !texts.iter().any(|t| t.text == "OTHER FILES"),
        "both files belong to the project: {texts:#?}"
    );
    let heading = find(&texts, "DCDC-BUCK").bounds;
    let sheet = find(&texts, "a.snxsch").bounds;
    let cap = find(&texts, "SCH").bounds;
    assert!(heading.y < sheet.y, "the heading leads its rows");
    assert!(cap.x < sheet.x, "the kind cap leads the row");
}

/// An Export Job in the project's folder is grouped under the project, with
/// its OUT cap, not under Other files (decision 2026-11-24, mockup row 16).
#[test]
fn an_export_job_in_the_project_folder_is_grouped_under_the_project() {
    let mut app = app_closing(&["a.snxsch", "jobs/fab.snxout"], Default::default());
    crate::app::handlers::menu::export::tests::add_project(&mut app, "/p", &["a.snxsch"]);
    if let Some(project) = app.document_state.projects.last_mut() {
        project.data.name = "dcdc-buck".into();
    }
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    for label in ["DCDC-BUCK", "fab.snxout", "OUT"] {
        find(&texts, label);
    }
    assert!(
        !texts.iter().any(|t| t.text == "OTHER FILES"),
        "the Export Job belongs to the project: {texts:#?}"
    );
}

/// The window follows the grouped list: more rows make it taller until the
/// list's cap, then the list scrolls.
#[test]
fn the_window_follows_the_grouped_list_up_to_its_cap() {
    let names: Vec<String> = (0..30).map(|i| format!("sheet{i}.snxsch")).collect();
    let height = |rows: usize| {
        let listed: Vec<&str> = names.iter().take(rows).map(String::as_str).collect();
        app_closing(&listed, Default::default())
            .modal_window_size(ModalId::ProjectCloseConfirm)
            .height
    };
    let (one, two, thirty) = (height(1), height(2), height(30));
    assert!(one < two, "a second row adds height ({one} < {two})");
    assert!(two < thirty, "{two} < {thirty}");
    let cap = crate::app::handlers::modal_sizing::CLOSE_LIST_MAX_HEIGHT;
    assert!(thirty - one <= cap, "30 rows add at most the list cap");
}

fn app_closing(
    dirty: &[&str],
    sheet_refs: crate::app::state::sheet_refs::SheetRefs,
) -> crate::app::Signex {
    let (mut app, _boot) = crate::app::Signex::new();
    let project_id = app.document_state.mint_project_id();
    app.ui_state.project_close_confirm = Some(Box::new(crate::app::ProjectCloseConfirmState {
        project_id,
        project_name: "P".into(),
        dirty_paths: paths(dirty),
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs,
    }));
    app
}

/// The list shows each row's name and kind cap; Discard All stands alone at
/// the left of the footer, then Cancel and Save All on the right.
#[test]
fn rows_show_their_kind_and_discard_all_stands_at_the_left() {
    let app = app_closing(&["a.snxsch", "P.snxprj"], Default::default());
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    for label in ["a.snxsch", "SCH", "P.snxprj", "PRJ"] {
        find(&texts, label);
    }
    let x = |label| find(&texts, label).bounds.x;
    assert!(x("Discard All") < x("Cancel"), "{texts:#?}");
    assert!(x("Cancel") < x("Save All"), "{texts:#?}");
}

/// Each button of the shared footer (mockup row 136) still answers with its
/// own choice.
#[test]
fn each_button_keeps_its_outcome() {
    use crate::app::ProjectCloseChoice;
    use crate::app::contracts::{Message, ProjectMsg};
    let choice = |label: &str| {
        let app = app_closing(&["a.snxsch"], Default::default());
        let messages = crate::layout_probe::messages_on_click(
            app.view_project_close_confirm_body(),
            iced::Size::new(900.0, 700.0),
            label,
        );
        match messages.as_slice() {
            [Message::Project(ProjectMsg::CloseConfirm(choice))] => *choice,
            other => panic!("{label}: {other:?}"),
        }
    };
    assert_eq!(choice("Discard All"), ProjectCloseChoice::DiscardAll);
    assert_eq!(choice("Cancel"), ProjectCloseChoice::Cancel);
    assert_eq!(choice("Save All"), ProjectCloseChoice::SaveAll);
}

/// The shared warn intro (mockup row 136) heads the grouped list: the
/// question, then the count line, then the group heading and its rows.
#[test]
fn the_intro_heads_the_grouped_list() {
    let app = app_closing(&["a.snxsch"], Default::default());
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    let en = I18n::en_us_ref();
    let question = en.message_plain(ids::DIALOG_CLOSE_PROJECT_QUESTION, &[]);
    let question = find(&texts, &question).bounds;
    let detail = texts
        .iter()
        .find(|t| plain(t.text.clone()).contains("unsaved"))
        .expect("the count line")
        .bounds;
    let heading = find(&texts, "OTHER FILES").bounds;
    let row = find(&texts, "a.snxsch").bounds;
    assert!(question.y < detail.y, "{texts:#?}");
    assert!(detail.y < heading.y && heading.y < row.y, "{texts:#?}");
}

/// An unreadable sheet withholds Discard All and says why under the grouped
/// list; Cancel and Save All stay.
#[test]
fn an_unreadable_sheet_withholds_discard_all() {
    let mut refs = crate::app::state::sheet_refs::SheetRefs::default();
    refs.unreadable
        .push(crate::app::state::sheet_refs::UnreadableSheet {
            path: PathBuf::from("/p/broken.snxsch"),
            reason: String::new(),
            whole_path: false,
        });
    let app = app_closing(&["a.snxsch"], refs);
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    assert!(texts.iter().all(|t| t.text != "Discard All"), "{texts:#?}");
    let row = find(&texts, "a.snxsch").bounds;
    let reason = texts
        .iter()
        .find(|t| t.text.contains("broken.snxsch"))
        .expect("the reason")
        .bounds;
    assert!(row.y < reason.y, "the reason sits under the list");
    find(&texts, "Cancel");
    find(&texts, "Save All");
}

#[test]
fn kept_documents_say_why_they_stay_open() {
    let en = I18n::en_us_ref();
    let kept = |names: &[&str]| plain(kept_line(en, &paths(names)).into_string());
    assert_eq!(
        kept(&["Stock.snxlib"]),
        "Stock.snxlib stays open because another open project may use it."
    );
    assert_eq!(
        kept(&["a.snxsch", "b.snxsch"]),
        "a.snxsch, b.snxsch stay open because other open projects may use them."
    );
}

/// The Close Project prompt of the read-only `ProjectA` (#2904) listing its
/// settings and `others` under `/p`, with `sheet_refs`.
fn read_only_closing(
    others: &[&str],
    sheet_refs: crate::app::state::sheet_refs::SheetRefs,
) -> crate::app::Signex {
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/p", &[]);
    let project = &mut app.document_state.projects[0];
    project.data.unknown.newer_minor_source = Some("snxprj/1.2".into());
    let (project_id, prj) = (project.id, project.path.clone());
    let mut dirty_paths = vec![prj];
    dirty_paths.extend(paths(others));
    app.ui_state.project_close_confirm = Some(Box::new(crate::app::ProjectCloseConfirmState {
        project_id,
        project_name: "ProjectA".into(),
        dirty_paths,
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs,
    }));
    app
}

/// The discard button of the read-only `ProjectA`, as drawn.
fn discard_project_a() -> String {
    let projects = ["ProjectA".to_string()];
    crate::app::read_only_project::discard_read_only_label(I18n::en_us_ref(), &projects)
        .into_string()
}

/// A read-only project's unsaved settings alone (#2904): Save All is left
/// out and a warn note says why; the discard names the project, and Cancel
/// stays.
#[test]
fn a_read_only_project_withholds_save_all_and_says_why() {
    let app = read_only_closing(&[], Default::default());
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    assert!(texts.iter().all(|t| t.text != "Save All"), "{texts:#?}");
    assert!(texts.iter().all(|t| t.text != "Discard All"), "{texts:#?}");
    find(&texts, &discard_project_a());
    find(&texts, "Cancel");
    assert!(
        texts.iter().any(|t| {
            let line = plain(t.text.clone());
            line.contains("'ProjectA'") && line.contains("Save All is not offered")
        }),
        "{texts:#?}"
    );
}

/// With a sheet unsaved too, Save All stays for it and the discard names the
/// read-only project alone (Caner, 2026-11-25); Discard All is gone and the
/// note says what each button does.
#[test]
fn a_read_only_project_with_a_dirty_sheet_keeps_save_all_and_discards_only_its_settings() {
    use crate::app::ProjectCloseChoice;
    use crate::app::contracts::{Message, ProjectMsg};
    let app = read_only_closing(&["a.snxsch"], Default::default());
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    find(&texts, "Save All");
    find(&texts, "Cancel");
    assert!(texts.iter().all(|t| t.text != "Discard All"), "{texts:#?}");
    let note = texts
        .iter()
        .map(|t| plain(t.text.clone()))
        .find(|line| line.contains("read-only in this build"))
        .expect("the note");
    assert!(
        note.contains("Save All saves every other file")
            && note.contains("Discard 'ProjectA' drops only")
            && !note.contains("Discard All"),
        "{note}"
    );
    let choice = |label: &str| {
        let messages = crate::layout_probe::messages_on_click(
            app.view_project_close_confirm_body(),
            iced::Size::new(900.0, 700.0),
            label,
        );
        match messages.as_slice() {
            [Message::Project(ProjectMsg::CloseConfirm(choice))] => *choice,
            other => panic!("{label}: {other:?}"),
        }
    };
    assert_eq!(
        choice(&discard_project_a()),
        ProjectCloseChoice::DiscardReadOnly
    );
    assert_eq!(choice("Save All"), ProjectCloseChoice::SaveAll);
}

/// An unreadable sheet blocks Discard All, but not the discard of the
/// read-only project's own settings, whose owner is known; no line offers
/// Discard All.
#[test]
fn an_unreadable_sheet_leaves_the_read_only_discard_and_no_line_offers_discard_all() {
    let mut refs = crate::app::state::sheet_refs::SheetRefs::default();
    refs.unreadable
        .push(crate::app::state::sheet_refs::UnreadableSheet {
            path: PathBuf::from("/p/broken.snxsch"),
            reason: String::new(),
            whole_path: false,
        });
    let app = read_only_closing(&["a.snxsch"], refs);
    let texts = lay_out_texts(
        app.view_project_close_confirm_body(),
        iced::Size::new(900.0, 700.0),
    );
    find(&texts, &discard_project_a());
    find(&texts, "Save All");
    assert!(texts.iter().all(|t| t.text != "Discard All"), "{texts:#?}");
    assert!(
        texts
            .iter()
            .all(|t| !plain(t.text.clone()).contains("Discard All drops")),
        "{texts:#?}"
    );
}
