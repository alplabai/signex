//! The user's own checklist templates (mockup row 206, empty state: "Your
//! templates"): a checklist saved under a name with Save as template, kept in
//! one file of the user's config folder, [`TEMPLATE_LIBRARY_FILE`], and
//! started from like a built-in template. A template keeps each item's
//! stage, title and link, never its state, owner or notes. A checklist names
//! the template it started from as [`TemplateRef::key`]. Keys this build
//! does not know are kept when the file is written again; a file this build
//! cannot read is never written over.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::edit::{ChecklistProblem, NewChecklistItem, optional_text};
use super::link::ChecklistLink;
use super::template::ChecklistTemplate;
use super::{ChecklistStage, DesignChecklist};
use crate::project::UnknownJson;

/// The library's file name in the user's config folder.
pub const TEMPLATE_LIBRARY_FILE: &str = "checklist-templates.json";

/// How a checklist names a saved template it started from: `user:<name>`.
const SAVED_PREFIX: &str = "user:";

/// A template a checklist starts from: built in, or one the user saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRef {
    Builtin(ChecklistTemplate),
    Saved(String),
}

impl TemplateRef {
    /// The template `text` names: a built-in key, or `user:<name>`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Some(name) = text.strip_prefix(SAVED_PREFIX) {
            return optional_text(name).map(Self::Saved);
        }
        ChecklistTemplate::parse(text).map(Self::Builtin)
    }

    /// The text [`Self::parse`] reads back, as a checklist stores it.
    pub fn key(&self) -> String {
        match self {
            Self::Builtin(template) => template.key().to_string(),
            Self::Saved(name) => format!("{SAVED_PREFIX}{name}"),
        }
    }
}

/// One item of a saved template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTemplateItem {
    pub stage: ChecklistStage,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<ChecklistLink>,
    /// Keys written by a newer build that this one does not know.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// A template the user saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTemplate {
    pub name: String,
    /// The project it was saved from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_from: Option<String>,
    #[serde(default)]
    pub items: Vec<SavedTemplateItem>,
    /// Keys written by a newer build that this one does not know.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl SavedTemplate {
    /// The items a start from it adds.
    pub fn new_items(&self) -> Vec<NewChecklistItem> {
        self.items
            .iter()
            .map(|item| NewChecklistItem {
                stage: item.stage,
                title: item.title.clone(),
                link: item.link.clone(),
            })
            .collect()
    }

    /// How many of its items are of `stage`.
    pub fn count(&self, stage: ChecklistStage) -> usize {
        self.items.iter().filter(|item| item.stage == stage).count()
    }

    /// How a checklist started from it names it.
    pub fn key(&self) -> String {
        TemplateRef::Saved(self.name.clone()).key()
    }
}

/// The user's saved templates, as the library file holds them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateLibrary {
    #[serde(default)]
    pub templates: Vec<SavedTemplate>,
    /// Keys written by a newer build that this one does not know.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl TemplateLibrary {
    /// The library in `bytes`.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|error| format!("not a template library: {error}"))
    }

    /// The library as the file holds it.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// The library file at `path`; an empty library when there is no file.
    /// Reads the disk: run it off the UI thread.
    pub fn read(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => Self::parse(&bytes).map_err(|why| format!("{}: {why}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    }

    /// The template called `name`.
    pub fn find(&self, name: &str) -> Option<&SavedTemplate> {
        let name = name.trim();
        self.templates.iter().find(|template| template.name == name)
    }

    /// Add `template`, in place of one with the same name.
    pub fn upsert(&mut self, template: SavedTemplate) {
        match self
            .templates
            .iter_mut()
            .find(|held| held.name == template.name)
        {
            Some(held) => *held = template,
            None => self.templates.push(template),
        }
    }

    /// Add `template` to the library file at `path` and write it back
    /// atomically; the library as written. A file that does not read as a
    /// library is refused, never written over. Reads and writes the disk:
    /// run it off the UI thread.
    pub fn save_into(path: &Path, template: SavedTemplate) -> Result<Self, String> {
        let mut library = Self::read(path)?;
        library.upsert(template);
        let bytes = library.encode()?;
        crate::atomic_io::atomic_write(path, &bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(library)
    }
}

impl DesignChecklist {
    /// Its items as another checklist starts from them: stage, title and
    /// link, in list order.
    pub fn new_items(&self) -> Vec<NewChecklistItem> {
        self.items
            .iter()
            .map(|item| NewChecklistItem {
                stage: item.stage,
                title: item.title.clone(),
                link: item.link.clone(),
            })
            .collect()
    }

    /// The checklist as a template called `name`, saved from `project`.
    pub fn to_saved_template(
        &self,
        name: &str,
        project: Option<&str>,
    ) -> Result<SavedTemplate, ChecklistProblem> {
        let name = optional_text(name).ok_or(ChecklistProblem::EmptyName)?;
        let items = self
            .items
            .iter()
            .map(|item| SavedTemplateItem {
                stage: item.stage,
                title: item.title.clone(),
                link: item.link.clone(),
                unknown: UnknownJson::default(),
            })
            .collect();
        Ok(SavedTemplate {
            name,
            saved_from: project.map(str::to_string),
            items,
            unknown: UnknownJson::default(),
        })
    }
}
