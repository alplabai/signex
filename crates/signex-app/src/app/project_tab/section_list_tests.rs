//! Mockup row 29: the section list's groups, badges and navigation, and the
//! General page's Explore button.

use super::section_list::{GROUPS, SectionBadge, SectionFacts, section_badge};
use super::version_control::Repository;
use super::{ProjectSection, ProjectTabMsg};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::state::project_fs::git_marker;
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
    repository: Repository::Unknown,
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
        repository: Repository::Present,
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

/// #2995, mockup row 35: the Version control badge follows the repository,
/// green On with one and neutral Off without; nothing while the first
/// probe is out.
#[test]
fn the_version_control_badge_follows_the_repository() {
    let badge = |repository| {
        let facts = SectionFacts {
            repository,
            ..NO_FACTS
        };
        section_badge(ProjectSection::VersionControl, &facts)
    };
    assert_eq!(badge(Repository::Present), Some(SectionBadge::On));
    assert_eq!(badge(Repository::Absent), Some(SectionBadge::Off));
    assert_eq!(badge(Repository::Unknown), None);
}

#[test]
fn the_facts_are_read_from_the_project() {
    let (mut app, _) = app();
    app.document_state.projects[0].data.enable_git = true;
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".into()];
    let folder = app.document_state.projects[0]
        .path
        .parent()
        .expect("the project has a folder")
        .to_path_buf();
    let project = &app.document_state.projects[0];
    let facts = app.section_facts(project);
    assert_eq!(facts.sheets, 2);
    assert_eq!(facts.variants, 1);
    assert_eq!(facts.libraries, 0);
    assert!(!facts.library_missing);
    assert_eq!(facts.repository, Repository::Unknown, "not probed yet");
    assert_eq!(facts.requirements_failed, 0, "no run yet");
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), false);
    let project = &app.document_state.projects[0];
    assert_eq!(
        app.section_facts(project).repository,
        Repository::Absent,
        "Commit every save on does not make a repository"
    );
}

/// #2995: the Off pill shows on the Version control row of the laid-out
/// list once the probe found no repository, and On once it found one,
/// whatever Commit every save says.
#[test]
fn the_version_control_row_shows_off_then_on() {
    let (mut app, id) = app();
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::General,
        },
    )));
    let folder = app.document_state.projects[0]
        .path
        .parent()
        .expect("the project has a folder")
        .to_path_buf();
    let row_badge = |app: &Signex| {
        let texts = crate::layout_probe::lay_out_texts(app.view_project_tab(id), WINDOW);
        let row = crate::layout_probe::find(&texts, "Version control").bounds;
        texts
            .iter()
            .find(|t| {
                (t.text == "Off" || t.text == "On")
                    && t.bounds.x > row.x + row.width
                    && (t.bounds.center_y() - row.center_y()).abs() < row.height
            })
            .map(|t| t.text.clone())
    };
    assert_eq!(row_badge(&app), None, "nothing before the first probe");
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), false);
    assert_eq!(row_badge(&app).as_deref(), Some("Off"));
    app.document_state.projects[0].data.enable_git = false;
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), true);
    assert_eq!(row_badge(&app).as_deref(), Some("On"));
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
        app.document_view.project_tabs[&id].section,
        ProjectSection::SchematicDefaults
    );
}

/// Each group's heading sits above its first row and below the group before.
#[test]
fn the_group_headings_lead_their_groups() {
    let (app, id) = app();
    let texts = crate::layout_probe::lay_out_texts(app.view_project_tab(id), WINDOW);
    let y = |label: &str| crate::layout_probe::find(&texts, label).bounds.y;
    let order = [
        "PROJECT",
        "Parameters",
        "RULES & DEFAULTS",
        "ERC and annotation",
        "TRACKING",
        "Version control",
    ];
    for pair in order.windows(2) {
        assert!(y(pair[0]) < y(pair[1]), "{} above {}", pair[0], pair[1]);
    }
}

/// The General page has its help line and a titled Files card.
#[test]
fn the_general_page_has_its_help_and_files_title() {
    let (app, id) = app();
    let texts = crate::layout_probe::lay_out_texts(app.view_project_tab(id), WINDOW);
    let help = "The project's name, description and the designator standard new parts follow.";
    let _ = crate::layout_probe::find(&texts, help);
    let files = crate::layout_probe::find(&texts, "Files").bounds.y;
    let directory = crate::layout_probe::find(&texts, "Directory").bounds.y;
    assert!(files < directory, "the title heads the card");
}

#[test]
fn show_in_libraries_opens_the_libraries_section() {
    let (mut app, id) = app();
    let messages = clicking(&app, id, "Show in Libraries");
    assert!(messages.iter().any(|m| matches!(
        m,
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::SectionSelected {
            project_id,
            section: ProjectSection::Libraries,
        })) if *project_id == id
    )));
    for message in messages {
        let _ = app.update(message);
    }
    assert_eq!(
        app.document_view.project_tabs[&id].section,
        ProjectSection::Libraries
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
