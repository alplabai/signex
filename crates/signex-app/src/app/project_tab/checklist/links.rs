//! Where an item's link leads (mockup row 206): the link card's Open shows
//! a requirement in the Project tab's Requirements section, the ERC or DRC
//! run in its panel and the title block in Parameters; a design-object link
//! cross-probes, opening the sheet that holds the part, pin or net and
//! selecting it, or opening the named sheet or the board. The link picker
//! offers the checks, the project's requirements, and the parts, nets and
//! sheets of the project's open documents. Catalog: the
//! `project_checklist_link_choices` query and `project_open_checklist_link`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_types::project::{ChecklistLink, ObjectTarget, sheet_targets};
use signex_types::schematic::SelectedItem;

use super::ChecklistNotice;
use super::commands::{Ran, refused, required};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{FileMsg, Message, OverlayMsg, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};

/// The groups of the link picker, in the order it lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChoiceGroup {
    Check,
    Requirement,
    Part,
    Net,
    Sheet,
    Board,
}

impl ChoiceGroup {
    /// The group's name in a command's answer.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Requirement => "requirement",
            Self::Part => "part",
            Self::Net => "net",
            Self::Sheet => "sheet",
            Self::Board => "board",
        }
    }
}

/// One row of the link picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkChoice {
    pub group: ChoiceGroup,
    pub link: ChecklistLink,
}

fn choice(group: ChoiceGroup, link: ChecklistLink) -> LinkChoice {
    LinkChoice { group, link }
}

fn object(target: &ObjectTarget) -> ChecklistLink {
    ChecklistLink::Object {
        target: target.to_text(),
    }
}

/// Where a link leads.
#[derive(Debug, Clone, PartialEq)]
enum Destination {
    Requirements,
    Erc,
    Drc,
    Parameters,
    Sheet {
        path: PathBuf,
        item: Option<SelectedItem>,
    },
    Board(PathBuf),
    /// The object is in none of the project's open documents.
    NotFound(String),
}

impl Destination {
    fn key(&self) -> &'static str {
        match self {
            Self::Requirements => "requirements",
            Self::Erc => "erc",
            Self::Drc => "drc",
            Self::Parameters => "parameters",
            Self::Sheet { .. } => "sheet",
            Self::Board(_) => "board",
            Self::NotFound(_) => "not_found",
        }
    }
}

/// The project's sheet files, in its list order.
fn sheet_paths(project: &LoadedProject) -> Vec<PathBuf> {
    project
        .data
        .sheets
        .iter()
        .map(|sheet| project.dir().join(&sheet.filename))
        .collect()
}

/// The project's board file, when it lists one.
fn board_path(project: &LoadedProject) -> Option<PathBuf> {
    project
        .data
        .pcb_file
        .as_deref()
        .filter(|file| !file.trim().is_empty())
        .map(|file| project.dir().join(file))
}

/// `project_checklist_link_choices {document}`: the link picker's rows,
/// each with its group and the text `project_set_checklist_item` takes.
pub(crate) fn link_choices_json(choices: &[LinkChoice]) -> Value {
    let rows: Vec<Value> = choices
        .iter()
        .map(|choice| json!({"group": choice.group.key(), "link": choice.link.to_text()}))
        .collect();
    json!({ "choices": rows })
}

/// `project_open_checklist_link {document, item}`: show what item `item`'s
/// link names; answers with where it led.
pub(crate) fn project_open_checklist_link(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let item = match required(args, "item", "an item id") {
        Ok(item) => item.trim().to_string(),
        Err(reason) => return refused(reason),
    };
    match app.open_checklist_link(id, &item) {
        (Ok(answer), task) => (CommandOutcome::Answered(answer), task),
        (Err(reason), task) => (CommandOutcome::disabled(reason), task),
    }
}

impl Signex {
    /// The link picker's rows for project `id`: the checks, the project's
    /// requirements, the parts and nets of its open sheets, its sheets and
    /// its board.
    pub(crate) fn checklist_link_choices(&self, id: ProjectId) -> Vec<LinkChoice> {
        let Some(project) = self.document_state.project_by_id(id) else {
            return Vec::new();
        };
        let mut choices = vec![
            choice(ChoiceGroup::Check, ChecklistLink::Erc),
            choice(ChoiceGroup::Check, ChecklistLink::Drc),
            choice(ChoiceGroup::Check, ChecklistLink::TitleBlock),
        ];
        for entry in project.data.requirements.iter() {
            if let Some(requirement) = entry.id() {
                let link = ChecklistLink::Requirement {
                    id: requirement.to_string(),
                };
                choices.push(choice(ChoiceGroup::Requirement, link));
            }
        }
        let (mut parts, mut nets) = (BTreeSet::new(), BTreeSet::new());
        for path in sheet_paths(project) {
            if let Some(engine) = self.document_state.engines.get(&path) {
                let targets = sheet_targets(engine.document());
                parts.extend(targets.parts);
                nets.extend(targets.nets);
            }
        }
        let parts = parts.into_iter().map(ObjectTarget::Part);
        choices.extend(parts.map(|t| choice(ChoiceGroup::Part, object(&t))));
        let nets = nets.into_iter().map(ObjectTarget::Net);
        choices.extend(nets.map(|t| choice(ChoiceGroup::Net, object(&t))));
        for sheet in &project.data.sheets {
            let target = ObjectTarget::Sheet(sheet.filename.clone());
            choices.push(choice(ChoiceGroup::Sheet, object(&target)));
        }
        if board_path(project).is_some() {
            choices.push(choice(ChoiceGroup::Board, object(&ObjectTarget::Board)));
        }
        choices
    }

    /// `project_checklist_link_choices {document}`.
    pub(crate) fn project_checklist_link_choices(
        &mut self,
        args: &CommandArgs,
    ) -> Result<Value, String> {
        let id = self.rules_project_named(args)?;
        Ok(link_choices_json(&self.checklist_link_choices(id)))
    }

    /// Where `link` of `project` leads.
    fn link_destination(&self, project: &LoadedProject, link: &ChecklistLink) -> Destination {
        let target = match link {
            ChecklistLink::Requirement { .. } => return Destination::Requirements,
            ChecklistLink::Erc => return Destination::Erc,
            ChecklistLink::Drc => return Destination::Drc,
            ChecklistLink::TitleBlock => return Destination::Parameters,
            ChecklistLink::Object { target } => ObjectTarget::parse(target),
        };
        let missing = || Destination::NotFound(target.to_text());
        match &target {
            ObjectTarget::Board => board_path(project).map_or_else(missing, Destination::Board),
            ObjectTarget::Sheet(file) => project
                .data
                .sheets
                .iter()
                .find(|sheet| sheet.filename == *file || sheet.name == *file)
                .map_or_else(missing, |sheet| Destination::Sheet {
                    path: project.dir().join(&sheet.filename),
                    item: None,
                }),
            _ => sheet_paths(project)
                .into_iter()
                .find_map(|path| {
                    let engine = self.document_state.engines.get(&path)?;
                    let item = target.find_on(engine.document())?;
                    Some(Destination::Sheet {
                        path,
                        item: Some(item),
                    })
                })
                .unwrap_or_else(missing),
        }
    }

    /// Show what item `item`'s link names and say where it led; refused
    /// when the item has no link.
    pub(crate) fn open_checklist_link(
        &mut self,
        project_id: ProjectId,
        item: &str,
    ) -> (Result<Value, String>, Task<Message>) {
        let link = self
            .document_state
            .project_by_id(project_id)
            .and_then(|project| project.data.design_checklist().ok().flatten())
            .and_then(|checklist| checklist.item(item).map(|found| found.link.clone()));
        let link = match link {
            None => {
                return (
                    Err(format!("the design checklist has no item `{item}`")),
                    Task::none(),
                );
            }
            Some(None) => return (Err(format!("item `{item}` links to nothing")), Task::none()),
            Some(Some(link)) => link,
        };
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return (Err("the project was closed".into()), Task::none());
        };
        let destination = self.link_destination(project, &link);
        let mut answer = json!({
            "item": item,
            "link": link.to_text(),
            "opens": destination.key(),
        });
        let task = match destination {
            Destination::Requirements => {
                self.open_project_section(project_id, ProjectSection::Requirements)
            }
            Destination::Parameters => {
                self.open_project_section(project_id, ProjectSection::Parameters)
            }
            Destination::Erc => Task::done(Message::Overlay(OverlayMsg::OpenPanel(
                crate::panels::PanelKind::Erc,
            ))),
            Destination::Drc => Task::done(Message::Overlay(OverlayMsg::OpenPanel(
                crate::panels::PanelKind::Drc,
            ))),
            Destination::Sheet { path, item } => {
                answer["document"] = json!(path.display().to_string());
                answer["object"] = json!(item.map(|item| item.uuid.to_string()));
                self.open_sheet(path, SheetOpen::plain().then(AfterOpen::Reveal { item }))
            }
            Destination::Board(path) => {
                answer["document"] = json!(path.display().to_string());
                Task::done(Message::File(FileMsg::Opened(Some(path))))
            }
            Destination::NotFound(what) => {
                self.checklist_tab(project_id).notice = Some(ChecklistNotice::LinkNotFound(what));
                Task::none()
            }
        };
        (Ok(answer), task)
    }

    /// The project's Project tab, opened or focused, on `section`.
    fn open_project_section(
        &mut self,
        project_id: ProjectId,
        section: ProjectSection,
    ) -> Task<Message> {
        let Some(index) = self
            .document_state
            .projects
            .iter()
            .position(|project| project.id == project_id)
        else {
            return Task::none();
        };
        self.open_project_tab(index);
        self.update(Message::Project(ProjectMsg::Tab(
            ProjectTabMsg::SectionSelected {
                project_id,
                section,
            },
        )))
    }
}
