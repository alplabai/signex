//! Mockup row 29: the section list's groups, badges and navigation, and the
//! General page's Explore button.

use super::section_list::{GROUPS, SectionBadge, SectionFacts, section_badge};
use super::{ProjectSection, ProjectTabMsg};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};

const WINDOW: iced::Size = iced::Size::new(1200.0, 800.0);

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-list-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch", "b.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn left_click() -> [iced::Event; 2] {
    use iced::mouse::{Button, Event as Mouse};
    [
        iced::Event::Mouse(Mouse::ButtonPressed(Button::Left)),
        iced::Event::Mouse(Mouse::ButtonReleased(Button::Left)),
    ]
}

fn clicking(app: &Signex, id: ProjectId, label: &str) -> Vec<Message> {
    let texts = crate::layout_probe::lay_out_texts(app.view_project_tab(id), WINDOW);
    let at = crate::layout_probe::find(&texts, label).bounds.center();
    crate::layout_probe::messages_for_events(app.view_project_tab(id), WINDOW, at, &left_click())
}

const NO_FACTS: SectionFacts = SectionFacts {
    sheets: 0,
    variants: 0,
    libraries: 0,
    library_missing: false,
    commit_on_save: false,
    requirements_failed: 0,
};

#[test]
fn the_groups_list_every_section_once_in_list_order() {
    let grouped: Vec<ProjectSection> = GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    assert_eq!(grouped, ProjectSection::ALL);
    assert_eq!(
        GROUPS[2],
        [ProjectSection::VersionControl, ProjectSection::Requirements]
    );
}

#[test]
fn each_badge_reads_its_own_fact() {
    let facts = SectionFacts {
        sheets: 4,
        variants: 2,
        libraries: 3,
        ..NO_FACTS
    };
    let badge = |section| section_badge(section, &facts);
    assert_eq!(badge(ProjectSection::Sheets), Some(SectionBadge::Count(4)));
    assert_eq!(
        badge(ProjectSection::Variants),
        Some(SectionBadge::Count(2))
    );
    assert_eq!(
        badge(ProjectSection::Libraries),
        Some(SectionBadge::Count(3))
    );
    for quiet in [
        ProjectSection::General,
        ProjectSection::Parameters,
        ProjectSection::ErcAnnotation,
        ProjectSection::SchematicDefaults,
        ProjectSection::Setup,
        ProjectSection::VersionControl,
        ProjectSection::Requirements,
    ] {
        assert_eq!(badge(quiet), None, "{quiet:?}");
    }
}

#[test]
fn trouble_and_state_change_the_badges() {
    let facts = SectionFacts {
        libraries: 3,
        library_missing: true,
        commit_on_save: true,
        requirements_failed: 2,
        ..NO_FACTS
    };
    assert_eq!(
        section_badge(ProjectSection::Libraries, &facts),
        Some(SectionBadge::CountWithDot(3))
    );
    assert_eq!(
        section_badge(ProjectSection::VersionControl, &facts),
        Some(SectionBadge::On)
    );
    assert_eq!(
        section_badge(ProjectSection::Requirements, &facts),
        Some(SectionBadge::Failed(2))
    );
}

#[test]
fn the_facts_are_read_from_the_project() {
    let (mut app, _) = app();
    app.document_state.projects[0].data.enable_git = true;
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".into()];
    let project = &app.document_state.projects[0];
    let facts = app.section_facts(project);
    assert_eq!(facts.sheets, 2);
    assert_eq!(facts.variants, 1);
    assert_eq!(facts.libraries, 0);
    assert!(!facts.library_missing);
    assert!(facts.commit_on_save);
    assert_eq!(facts.requirements_failed, 0, "no run yet");
}

#[test]
fn clicking_a_row_selects_its_section() {
    let (mut app, id) = app();
    let messages = clicking(&app, id, "Schematic defaults");
    assert!(
        messages.iter().any(|m| matches!(
            m,
            Message::Project(ProjectMsg::Tab(ProjectTabMsg::SectionSelected {
                project_id,
                section: ProjectSection::SchematicDefaults,
            })) if *project_id == id
        )),
        "the row publishes its section"
    );
    for message in messages {
        let _ = app.update(message);
    }
    assert_eq!(
        app.document_state.project_tabs[&id].section,
        ProjectSection::SchematicDefaults
    );
}

#[test]
fn explore_publishes_its_project() {
    let (app, id) = app();
    let messages = clicking(&app, id, "Explore");
    assert!(messages.iter().any(|m| matches!(
        m,
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::ExploreRequested { project_id }))
            if *project_id == id
    )));
}
