//! The templates a checklist starts from (mockup row 206, empty state):
//! Release review, Prototype build and Blank. A template item has a key, not
//! words: the editor that starts the checklist writes each item's title in
//! the person's language, and the item keeps that text.

use self::TemplateLink::{Drc, Erc, TitleBlock};
use super::ChecklistStage::{self, Fabrication, Layout, Schematic};
use super::link::ChecklistLink;

/// One item a template starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateItem {
    /// Stable name of the item; the editor's catalog words it.
    pub key: &'static str,
    pub stage: ChecklistStage,
    /// The check it links to: ERC, DRC or the title block.
    pub link: Option<TemplateLink>,
}

/// The checks a template item can link to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateLink {
    Erc,
    Drc,
    TitleBlock,
}

impl TemplateLink {
    pub fn to_link(self) -> ChecklistLink {
        match self {
            Self::Erc => ChecklistLink::Erc,
            Self::Drc => ChecklistLink::Drc,
            Self::TitleBlock => ChecklistLink::TitleBlock,
        }
    }
}

/// A template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecklistTemplate {
    ReleaseReview,
    PrototypeBuild,
    Blank,
}

/// Every template, in the order the empty tab offers them.
pub const CHECKLIST_TEMPLATES: [ChecklistTemplate; 3] = [
    ChecklistTemplate::ReleaseReview,
    ChecklistTemplate::PrototypeBuild,
    ChecklistTemplate::Blank,
];

const fn at(key: &'static str, stage: ChecklistStage, link: Option<TemplateLink>) -> TemplateItem {
    TemplateItem { key, stage, link }
}

const RELEASE_REVIEW: &[TemplateItem] = &[
    at("erc_clean", Schematic, Some(Erc)),
    at("title_block", Schematic, Some(TitleBlock)),
    at("parts_have_mpn", Schematic, None),
    at("fitted_parts_reviewed", Schematic, None),
    at("requirements_met", Schematic, None),
    at("drc_clean", Layout, Some(Drc)),
    at("board_outline", Layout, None),
    at("placement_reviewed", Layout, None),
    at("fab_outputs_written", Fabrication, None),
    at("gerbers_viewed", Fabrication, None),
    at("bom_released", Fabrication, None),
    at("assembly_drawings", Fabrication, None),
];

const PROTOTYPE_BUILD: &[TemplateItem] = &[
    at("erc_clean", Schematic, Some(Erc)),
    at("title_block", Schematic, Some(TitleBlock)),
    at("drc_clean", Layout, Some(Drc)),
    at("board_outline", Layout, None),
    at("fab_outputs_written", Fabrication, None),
    at("gerbers_viewed", Fabrication, None),
];

impl ChecklistTemplate {
    /// The template's name in the file and in a command.
    pub const fn key(self) -> &'static str {
        match self {
            Self::ReleaseReview => "release_review",
            Self::PrototypeBuild => "prototype_build",
            Self::Blank => "blank",
        }
    }

    /// The template named `key`.
    pub fn parse(key: &str) -> Option<Self> {
        CHECKLIST_TEMPLATES
            .into_iter()
            .find(|template| template.key() == key)
    }

    /// The items it starts with, in list order.
    pub const fn items(self) -> &'static [TemplateItem] {
        match self {
            Self::ReleaseReview => RELEASE_REVIEW,
            Self::PrototypeBuild => PROTOTYPE_BUILD,
            Self::Blank => &[],
        }
    }
}
