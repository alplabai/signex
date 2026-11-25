//! The Parameters section (mockup row 30): the filter, the folded
//! groups and renaming a variable in place, driven through `Signex::update`.

use std::collections::BTreeMap;

use super::rename::{ParameterNameProblem, RenameNote, checked_rename, rename_note};
use super::view::{Evaluation, evaluation, result_words};
use super::{ParameterGroup, visible_parameters};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};
use crate::panels::TitleBlockField;

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-params-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(msg)));
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn parameters(app: &Signex) -> &BTreeMap<String, String> {
    &app.document_state.projects[0].data.parameters
}

fn undo(app: &mut Signex, id: ProjectId) -> bool {
    app.step_project_tab_history(id, true)
}

/// How many General undo steps the tab holds, counted on copies.
fn undo_steps(app: &Signex, id: ProjectId) -> usize {
    let mut history = app.document_state.project_tabs[&id].general_history.clone();
    let mut data = app.document_state.projects[0].data.clone();
    let mut steps = 0;
    while history.undo(&mut data) {
        steps += 1;
    }
    steps
}

/// Add the variable `name` with `value` through the tab: two undo steps.
fn add_variable(app: &mut Signex, id: ProjectId, name: &str, value: &str) {
    send(
        app,
        ProjectTabMsg::NewParameterNameEdited {
            project_id: id,
            text: name.into(),
        },
    );
    send(app, ProjectTabMsg::ParameterAdded { project_id: id });
    send(
        app,
        ProjectTabMsg::ParameterEdited {
            project_id: id,
            name: name.into(),
            value: value.into(),
        },
    );
}

fn rename(app: &mut Signex, id: ProjectId, from: &str, text: &str) {
    send(
        app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: from.into(),
            text: text.into(),
        },
    );
    send(
        app,
        ProjectTabMsg::ParameterRenameSubmitted { project_id: id },
    );
}

fn english(field: TitleBlockField) -> String {
    field.label().to_string()
}

#[test]
fn the_filter_matches_the_shown_name_or_the_value_ignoring_case() {
    let parameters = BTreeMap::from([
        ("company".to_string(), "Acme Ltd".to_string()),
        ("rev".to_string(), "B".to_string()),
        ("VIN".to_string(), "12V".to_string()),
        ("FSW".to_string(), "500kHz".to_string()),
    ]);

    let all = visible_parameters(&parameters, "", english);
    assert_eq!(all.title_block, TitleBlockField::ALL.to_vec());
    assert_eq!(
        all.variables,
        ["FSW", "VIN"],
        "title-block keys are not variables"
    );
    assert_eq!(visible_parameters(&parameters, "   ", english), all);

    // The shown name of a title-block field, not its key.
    let by_name = visible_parameters(&parameters, "document no", english);
    assert_eq!(by_name.title_block, [TitleBlockField::DocumentNumber]);
    assert!(by_name.variables.is_empty());

    // A value, in either group, whatever the case.
    let by_value = visible_parameters(&parameters, "ACME", english);
    assert_eq!(by_value.title_block, [TitleBlockField::Company]);
    let by_value = visible_parameters(&parameters, "khz", english);
    assert!(by_value.title_block.is_empty());
    assert_eq!(by_value.variables, ["FSW"]);

    // A variable's key.
    let by_key = visible_parameters(&parameters, "vi", english);
    assert_eq!(by_key.variables, ["VIN"]);
}

#[test]
fn a_rename_is_checked_before_the_engine_sees_it() {
    let parameters = BTreeMap::from([
        ("VIN".to_string(), "12V".to_string()),
        ("VOUT".to_string(), "3V3".to_string()),
    ]);
    let check = |text: &str| checked_rename(&parameters, "VIN", text);
    assert_eq!(check("VSUPPLY"), Ok(Some("VSUPPLY".to_string())));
    assert_eq!(check("  VSUPPLY "), Ok(Some("VSUPPLY".to_string())));
    assert_eq!(check("VIN"), Ok(None), "its own name is nothing to do");
    assert_eq!(
        check("vout"),
        Ok(Some("vout".to_string())),
        "case-sensitive"
    );
    assert_eq!(check(""), Err(ParameterNameProblem::Empty));
    assert_eq!(check("   "), Err(ParameterNameProblem::Empty));
    assert_eq!(check("V\nIN"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("V\tIN"), Err(ParameterNameProblem::Invalid));
    // #1987: a parameter is a variable, named like the Add row requires.
    assert_eq!(check("V IN"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("1V"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("V-IN"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("_V1"), Ok(Some("_V1".to_string())));
    assert_eq!(check("company"), Err(ParameterNameProblem::TitleBlock));
    assert_eq!(check("rev"), Err(ParameterNameProblem::TitleBlock));
    assert_eq!(check("VOUT"), Err(ParameterNameProblem::Taken));
}

/// Renaming moves only the project key, so `${VIN}` on a sheet and a sheet
/// value named VIN stop resolving to it: a name that Enter would apply is
/// warned about before Enter; a refusal shows only after Enter.
#[test]
fn an_accepted_rename_draft_warns_that_references_keep_the_old_name() {
    let parameters = BTreeMap::from([
        ("VIN".to_string(), "12V".to_string()),
        ("VOUT".to_string(), "3V3".to_string()),
    ]);
    let note = |text: &str, refused: bool| rename_note(&parameters, "VIN", text, refused);
    assert_eq!(
        note("VSUPPLY", false),
        Some(RenameNote::ReferencesKeepOldName)
    );
    assert_eq!(
        note("VSUPPLY", true),
        Some(RenameNote::ReferencesKeepOldName)
    );
    assert_eq!(note("VIN", false), None, "its own name renames nothing");
    assert_eq!(note(" VIN ", false), None);
    assert_eq!(note("VOUT", false), None, "not refused until Enter");
    assert_eq!(
        note("VOUT", true),
        Some(RenameNote::Refused(ParameterNameProblem::Taken))
    );
}

#[test]
fn a_rename_through_the_tab_is_one_undo_step_and_dirties_the_project() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    rename(&mut app, id, "VIN", "VSUPPLY");
    assert_eq!(
        parameters(&app).get("VSUPPLY").map(String::as_str),
        Some("12V")
    );
    assert!(!parameters(&app).contains_key("VIN"));
    assert!(dirty(&app));
    let tab = &app.document_state.project_tabs[&id];
    assert_eq!(tab.parameter_rename, None, "the draft is spent");
    assert!(!tab.parameter_rename_refused);

    assert!(undo(&mut app, id), "one step takes the rename back");
    assert_eq!(parameters(&app).get("VIN").map(String::as_str), Some("12V"));
    assert!(!parameters(&app).contains_key("VSUPPLY"));
    // Then the value, then the add: back to the file's content.
    assert!(undo(&mut app, id));
    assert!(undo(&mut app, id));
    assert!(parameters(&app).is_empty());
    assert!(!dirty(&app));
}

#[test]
fn a_refused_rename_keeps_the_data_and_the_draft_and_records_nothing() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    add_variable(&mut app, id, "VOUT", "3V3");
    let before = parameters(&app).clone();
    let steps_before = undo_steps(&app, id);
    for text in ["", "  ", "V\nIN", "company", "VOUT"] {
        rename(&mut app, id, "VIN", text);
        assert_eq!(parameters(&app), &before, "{text:?} changes nothing");
        let tab = &app.document_state.project_tabs[&id];
        assert!(tab.parameter_rename_refused, "{text:?} is refused");
        assert_eq!(
            tab.parameter_rename,
            Some(("VIN".to_string(), text.to_string())),
            "the refused name stays in its box"
        );
    }
    assert_eq!(undo_steps(&app, id), steps_before, "no step was recorded");

    // The next keystroke clears the refusal.
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VIN".into(),
            text: "VI".into(),
        },
    );
    assert!(!app.document_state.project_tabs[&id].parameter_rename_refused);
}

#[test]
fn the_rename_draft_is_cleared_on_a_section_switch_and_on_undo() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    rename(&mut app, id, "VIN", "VOUT ");
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VOUT".into(),
            text: "".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameSubmitted { project_id: id },
    );
    assert!(app.document_state.project_tabs[&id].parameter_rename_refused);
    send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Sheets,
        },
    );
    let tab = &app.document_state.project_tabs[&id];
    assert_eq!(tab.parameter_rename, None);
    assert!(!tab.parameter_rename_refused);

    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VOUT".into(),
            text: "VO".into(),
        },
    );
    assert!(undo(&mut app, id));
    assert_eq!(app.document_state.project_tabs[&id].parameter_rename, None);
}

#[test]
fn the_filter_and_the_folded_groups_are_tab_state_only() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    let dirty_before = dirty(&app);
    send(
        &mut app,
        ProjectTabMsg::ParameterFilterEdited {
            project_id: id,
            text: "vin".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::TitleBlock,
        },
    );
    let tab = &app.document_state.project_tabs[&id];
    assert_eq!(tab.parameter_filter, "vin");
    assert!(!tab.is_parameter_group_open(ParameterGroup::TitleBlock));
    assert!(tab.is_parameter_group_open(ParameterGroup::Variables));
    assert_eq!(dirty(&app), dirty_before, "no document edit");

    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::TitleBlock,
        },
    );
    let tab = &app.document_state.project_tabs[&id];
    assert!(tab.is_parameter_group_open(ParameterGroup::TitleBlock));
}

#[test]
fn a_late_message_for_a_closed_project_leaves_no_tab_state() {
    let (mut app, _id) = app();
    let gone = app.document_state.mint_project_id();
    for msg in [
        ProjectTabMsg::ParameterFilterEdited {
            project_id: gone,
            text: "x".into(),
        },
        ProjectTabMsg::ParameterGroupToggled {
            project_id: gone,
            group: ParameterGroup::Variables,
        },
        ProjectTabMsg::ParameterRenameEdited {
            project_id: gone,
            from: "VIN".into(),
            text: "VOUT".into(),
        },
        ProjectTabMsg::ParameterRenameSubmitted { project_id: gone },
    ] {
        send(&mut app, msg);
    }
    assert!(!app.document_state.project_tabs.contains_key(&gone));
}

#[test]
fn the_section_renders_with_a_refused_rename_a_filter_and_a_folded_group() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    add_variable(&mut app, id, "VOUT", "");
    send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Parameters,
        },
    );
    let _ = app.view_project_tab(id);
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VIN".into(),
            text: "VSUPPLY".into(),
        },
    );
    let _ = app.view_project_tab(id);
    rename(&mut app, id, "VIN", "VOUT");
    let _ = app.view_project_tab(id);
    send(
        &mut app,
        ProjectTabMsg::ParameterFilterEdited {
            project_id: id,
            text: "v".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::Variables,
        },
    );
    let _ = app.view_project_tab(id);
}

/// Mockup row 30: an expression that evaluates shows its result (the green
/// "Shows …" pill), one that does not gives the error row, and a literal
/// value shows neither.
#[test]
fn an_expression_shows_its_result_or_its_error_and_a_literal_neither() {
    let parameters: BTreeMap<String, String> = [
        ("VOUT", "3.3V"),
        ("VREF", "= VOUT / 2"),
        ("IOUT_MAX", "= POUT / VOUT"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    assert_eq!(
        evaluation(&parameters, "VREF"),
        Some(Evaluation::Shows("1.65V".to_string()))
    );
    assert!(matches!(
        evaluation(&parameters, "IOUT_MAX"),
        Some(Evaluation::Error(reason)) if !reason.is_empty()
    ));
    assert_eq!(evaluation(&parameters, "VOUT"), None);
    assert_eq!(evaluation(&parameters, "MISSING"), None);
}

/// Mockup row 30: the result pill reads "Shows …" for an expression that
/// evaluates and an error "?" for one that does not; a literal has none.
#[test]
fn the_result_pill_reads_shows_or_an_error_question_mark() {
    use signex_i18n::{ids, t};
    let i18n = signex_i18n::I18n::en_us();
    let words = |evaluated: Option<Evaluation>| {
        result_words(&i18n, evaluated.as_ref())
            .map(|(words, is_error)| (words.to_string(), is_error))
    };
    let shows = t!(i18n, ids::PROJECT_PARAMETER_SHOWS, "value" => "1.65V".to_string());
    assert_eq!(
        words(Some(Evaluation::Shows("1.65V".to_string()))),
        Some((shows.to_string(), false))
    );
    assert_eq!(
        words(Some(Evaluation::Error("POUT is not defined".to_string()))),
        Some(("?".to_string(), true))
    );
    assert_eq!(words(None), None);
}

/// The section draws the result pill, the error row and an error row that
/// also carries a rename note.
#[test]
fn the_section_renders_a_result_pill_and_an_error_row() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VOUT", "3.3V");
    add_variable(&mut app, id, "VREF", "= VOUT / 2");
    add_variable(&mut app, id, "IOUT_MAX", "= POUT / VOUT");
    send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Parameters,
        },
    );
    let _ = app.view_project_tab(id);
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "IOUT_MAX".into(),
            text: "IMAX".into(),
        },
    );
    let _ = app.view_project_tab(id);
}

/// Mockup row 30 (Caner 2026-11-24): the project Title block lists the
/// sheet's Author, Engineer, Organization and Address 1 to 4 too, so project
/// and sheet parameters line up; their keys are title-block rows, never
/// variables.
#[test]
fn the_project_title_block_has_the_sheets_address_and_people_fields() {
    use TitleBlockField::*;
    let extra = [
        Author,
        Engineer,
        Organization,
        Address1,
        Address2,
        Address3,
        Address4,
    ];
    let parameters: BTreeMap<String, String> = extra
        .iter()
        .map(|field| (field.key().to_string(), format!("v-{}", field.key())))
        .collect();
    let all = visible_parameters(&parameters, "", english);
    for field in extra {
        assert!(all.title_block.contains(&field), "{field:?} is listed");
    }
    assert!(all.variables.is_empty(), "none of them is a variable");
    assert_eq!(super::variable_count(&parameters), 0);
    let by_value = visible_parameters(&parameters, "v-address3", english);
    assert_eq!(by_value.title_block, [Address3]);
    let en = signex_i18n::I18n::en_us();
    for field in extra {
        let shown = en.message_plain(super::field_words(field), &[]);
        assert_eq!(shown, field.label(), "{field:?}");
    }
}

/// Mockup row 30: an empty value reads "Not set" in the italic UI face; a
/// set value or name is mono and upright; an empty name box keeps its
/// placeholder in the plain UI face.
#[test]
fn an_empty_value_reads_not_set_in_italics() {
    use super::view::inline_font;
    use iced::font::Style;
    let not_set = inline_font("", false).expect("a face for Not set");
    assert_eq!(not_set.style, Style::Italic);
    assert_eq!(
        not_set.family,
        crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT).family
    );
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    for (value, is_name) in [("3V3", false), ("VCC", true)] {
        let set = inline_font(value, is_name).expect("a set box is mono");
        assert_eq!((set.family, set.style), (mono.family, Style::Normal));
    }
    assert_eq!(inline_font("", true), None);
}

/// Whether the Parameters section reports a Fill height once its widget
/// tree is reconciled; Fit widgets adopt a Fill child's height there.
fn section_fills_height(app: &Signex, id: ProjectId) -> bool {
    use iced::advanced::Widget as _;
    use iced::advanced::widget::Tree;
    let project = &app.document_state.projects[0];
    let tab = app.document_state.project_tabs.get(&id);
    let i18n = app.ui_state.i18n.as_ref();
    let tokens = &app.document_state.panel_ctx.tokens;
    let mut element = super::view::view(project, tab, i18n, tokens);
    let mut tree = Tree::new(&element);
    tree.diff(&mut element);
    element.size().height.is_fill()
}

/// Mockup row 30 (#2755): with the Title block folded, a filter set, an
/// error row and a rename note, the section keeps its content height. A
/// Fill height made the page scroller Fill, so the spacer under it took
/// half the page and the list was cut at about 360 px.
#[test]
fn a_noted_row_keeps_the_section_height_to_its_content() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VOUT", "3.3V");
    add_variable(&mut app, id, "VREF", "= VOUT / 2");
    add_variable(&mut app, id, "IOUT_MAX", "= POUT / VOUT");
    send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Parameters,
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::TitleBlock,
        },
    );
    assert!(!section_fills_height(&app, id), "Title block folded");
    send(
        &mut app,
        ProjectTabMsg::ParameterFilterEdited {
            project_id: id,
            text: "VOUT".into(),
        },
    );
    assert!(!section_fills_height(&app, id), "filter set");
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "IOUT_MAX".into(),
            text: "IMAX".into(),
        },
    );
    assert!(!section_fills_height(&app, id), "rename note");
}
