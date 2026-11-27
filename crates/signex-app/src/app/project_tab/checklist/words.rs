//! The catalog words of the Design Checklist tab: templates, stages, states,
//! links, results, notices and the reason an edit was refused.

use signex_i18n::{I18n, Localized, MessageId, Translatable, ids, t};
use signex_types::project::{
    ChecklistLink, ChecklistProblem, ChecklistStage, ChecklistTemplate, ItemState, LinkResult,
    ObjectTarget,
};

use super::ChecklistNotice;

/// The name of a template.
pub(super) fn template_words(template: ChecklistTemplate) -> MessageId {
    match template {
        ChecklistTemplate::ReleaseReview => ids::CHECKLIST_TEMPLATE_RELEASE_REVIEW,
        ChecklistTemplate::PrototypeBuild => ids::CHECKLIST_TEMPLATE_PROTOTYPE_BUILD,
        ChecklistTemplate::Blank => ids::CHECKLIST_TEMPLATE_BLANK,
    }
}

/// The line under a template's name.
pub(super) fn template_help(template: ChecklistTemplate) -> MessageId {
    match template {
        ChecklistTemplate::ReleaseReview => ids::CHECKLIST_TEMPLATE_RELEASE_REVIEW_HELP,
        ChecklistTemplate::PrototypeBuild => ids::CHECKLIST_TEMPLATE_PROTOTYPE_BUILD_HELP,
        ChecklistTemplate::Blank => ids::CHECKLIST_TEMPLATE_BLANK_HELP,
    }
}

/// The title a template item starts with, by its key.
pub(super) fn template_item_words(key: &str) -> MessageId {
    match key {
        "erc_clean" => ids::CHECKLIST_ITEM_ERC_CLEAN,
        "title_block" => ids::CHECKLIST_ITEM_TITLE_BLOCK,
        "parts_have_mpn" => ids::CHECKLIST_ITEM_PARTS_HAVE_MPN,
        "fitted_parts_reviewed" => ids::CHECKLIST_ITEM_FITTED_PARTS_REVIEWED,
        "requirements_met" => ids::CHECKLIST_ITEM_REQUIREMENTS_MET,
        "drc_clean" => ids::CHECKLIST_ITEM_DRC_CLEAN,
        "board_outline" => ids::CHECKLIST_ITEM_BOARD_OUTLINE,
        "placement_reviewed" => ids::CHECKLIST_ITEM_PLACEMENT_REVIEWED,
        "fab_outputs_written" => ids::CHECKLIST_ITEM_FAB_OUTPUTS_WRITTEN,
        "gerbers_viewed" => ids::CHECKLIST_ITEM_GERBERS_VIEWED,
        "bom_released" => ids::CHECKLIST_ITEM_BOM_RELEASED,
        _ => ids::CHECKLIST_ITEM_ASSEMBLY_DRAWINGS,
    }
}

pub(super) fn stage_words(stage: ChecklistStage) -> MessageId {
    match stage {
        ChecklistStage::Schematic => ids::CHECKLIST_STAGE_SCHEMATIC,
        ChecklistStage::Layout => ids::CHECKLIST_STAGE_LAYOUT,
        ChecklistStage::Fabrication => ids::CHECKLIST_STAGE_FABRICATION,
    }
}

pub(super) fn state_words(state: ItemState) -> MessageId {
    match state {
        ItemState::Open => ids::CHECKLIST_STATE_OPEN,
        ItemState::Done => ids::CHECKLIST_STATE_DONE,
        ItemState::Waived => ids::CHECKLIST_STATE_WAIVED,
    }
}

/// What a link is to, as the table shows it: catalog words, with the part,
/// pin, net or sheet it names, or the design object as it was typed.
pub(super) fn link_words(link: Option<&ChecklistLink>, i18n: &I18n) -> Localized {
    match link {
        None => t!(i18n, ids::CHECKLIST_LINK_NONE),
        Some(ChecklistLink::Erc) => t!(i18n, ids::CHECKLIST_LINK_ERC),
        Some(ChecklistLink::Drc) => t!(i18n, ids::CHECKLIST_LINK_DRC),
        Some(ChecklistLink::TitleBlock) => t!(i18n, ids::CHECKLIST_LINK_TITLE_BLOCK),
        Some(ChecklistLink::Requirement { id }) => {
            t!(i18n, ids::CHECKLIST_LINK_REQUIREMENT, "id" => id.as_str())
        }
        Some(ChecklistLink::Object { target }) => object_words(&ObjectTarget::parse(target), i18n),
    }
}

/// What a design-object link names.
fn object_words(target: &ObjectTarget, i18n: &I18n) -> Localized {
    match target {
        ObjectTarget::Part(part) => t!(i18n, ids::CHECKLIST_LINK_PART, "part" => part.as_str()),
        ObjectTarget::Pin { part, pin } => t!(
            i18n,
            ids::CHECKLIST_LINK_PIN,
            "part" => part.as_str(),
            "pin" => pin.as_str()
        ),
        ObjectTarget::Net(net) => t!(i18n, ids::CHECKLIST_LINK_NET, "net" => net.as_str()),
        ObjectTarget::Sheet(sheet) => {
            t!(i18n, ids::CHECKLIST_LINK_SHEET, "sheet" => sheet.as_str())
        }
        ObjectTarget::Board => t!(i18n, ids::CHECKLIST_LINK_BOARD),
        ObjectTarget::Text(what) => t!(i18n, ids::CHECKLIST_LINK_OBJECT, "what" => what.as_str()),
    }
}

/// The words of a notice, and whether it says something failed.
pub(super) fn notice_words(notice: &ChecklistNotice, i18n: &I18n) -> (Localized, bool) {
    use ChecklistNotice as N;
    match notice {
        N::TemplateSaved(name) => (
            t!(i18n, ids::CHECKLIST_NOTICE_TEMPLATE_SAVED, "name" => name.as_str()),
            false,
        ),
        N::TemplateNotSaved(reason) => (
            t!(i18n, ids::CHECKLIST_NOTICE_TEMPLATE_NOT_SAVED, "reason" => reason.as_str()),
            true,
        ),
        N::LibraryUnreadable(reason) => (
            t!(i18n, ids::CHECKLIST_NOTICE_LIBRARY_UNREADABLE, "reason" => reason.as_str()),
            true,
        ),
        N::ImportFailed(reason) => (
            t!(i18n, ids::CHECKLIST_NOTICE_IMPORT_FAILED, "reason" => reason.as_str()),
            true,
        ),
        N::NothingToImport(project) => (
            t!(i18n, ids::CHECKLIST_NOTICE_NOTHING_TO_IMPORT, "project" => project.as_str()),
            true,
        ),
        N::LinkNotFound(what) => (
            t!(i18n, ids::CHECKLIST_NOTICE_LINK_NOT_FOUND, "what" => what.as_str()),
            true,
        ),
    }
}

/// A link's result as its pill says it; `None` when there is nothing to
/// say (no link, or a design object).
pub(super) fn result_words(result: LinkResult, i18n: &I18n) -> Option<Localized> {
    match result {
        LinkResult::Passes => Some(t!(i18n, ids::CHECKLIST_RESULT_PASSES)),
        LinkResult::Fails(count) => Some(t!(i18n, ids::CHECKLIST_RESULT_FAILS, "count" => count)),
        LinkResult::NotRun => Some(t!(i18n, ids::CHECKLIST_RESULT_NOT_RUN)),
        LinkResult::NoCheck => None,
    }
}

/// The words of a refused edit.
pub(super) fn problem_words(problem: &ChecklistProblem, i18n: &I18n) -> Localized {
    use ChecklistProblem as P;
    match problem {
        P::Unreadable => t!(i18n, ids::CHECKLIST_PROBLEM_UNREADABLE),
        P::AlreadyStarted => t!(i18n, ids::CHECKLIST_PROBLEM_ALREADY_STARTED),
        P::NotStarted => t!(i18n, ids::CHECKLIST_PROBLEM_NOT_STARTED),
        P::NoTemplate => t!(i18n, ids::CHECKLIST_PROBLEM_NO_TEMPLATE),
        P::UnknownSavedTemplate(name) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_SAVED_TEMPLATE, "name" => name.as_str())
        }
        P::EmptyName => t!(i18n, ids::CHECKLIST_PROBLEM_EMPTY_NAME),
        P::UnknownTemplate(text) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_TEMPLATE, "text" => text.as_str())
        }
        P::UnknownItem(id) => t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_ITEM, "id" => id.as_str()),
        P::UnknownStage(text) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_STAGE, "text" => text.as_str())
        }
        P::UnknownState(text) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_STATE, "text" => text.as_str())
        }
        P::UnknownField(text) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_FIELD, "text" => text.as_str())
        }
        P::UnknownLink(text) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_UNKNOWN_LINK, "text" => text.as_str())
        }
        P::EmptyTitle => t!(i18n, ids::CHECKLIST_PROBLEM_EMPTY_TITLE),
        P::NeedsReason => t!(i18n, ids::CHECKLIST_PROBLEM_NEEDS_REASON),
        P::NotStored(reason) => {
            t!(i18n, ids::CHECKLIST_PROBLEM_NOT_STORED, "reason" => reason.as_str())
        }
    }
}

/// Why a checklist command was refused, as an agent or the CLI reads it:
/// the problem in the command's words (backquoted values, the choices).
pub(super) fn problem_refusal(problem: &ChecklistProblem) -> Translatable {
    use ChecklistProblem as P;
    match problem {
        P::Unreadable => Translatable::new(ids::CHECKLIST_REFUSAL_UNREADABLE),
        P::AlreadyStarted => Translatable::new(ids::CHECKLIST_REFUSAL_ALREADY_STARTED),
        P::NotStarted => Translatable::new(ids::CHECKLIST_REFUSAL_NOT_STARTED),
        P::NoTemplate => Translatable::new(ids::CHECKLIST_REFUSAL_NO_TEMPLATE),
        P::UnknownTemplate(text) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_TEMPLATE).arg("text", text.as_str())
        }
        P::UnknownSavedTemplate(name) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_SAVED_TEMPLATE)
                .arg("name", name.as_str())
        }
        P::EmptyName => Translatable::new(ids::CHECKLIST_REFUSAL_EMPTY_NAME),
        P::UnknownItem(id) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_ITEM).arg("id", id.as_str())
        }
        P::UnknownStage(text) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_STAGE).arg("text", text.as_str())
        }
        P::UnknownState(text) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_STATE).arg("text", text.as_str())
        }
        P::UnknownField(text) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_FIELD).arg("text", text.as_str())
        }
        P::UnknownLink(text) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_UNKNOWN_LINK).arg("text", text.as_str())
        }
        P::EmptyTitle => Translatable::new(ids::CHECKLIST_REFUSAL_EMPTY_TITLE),
        P::NeedsReason => Translatable::new(ids::CHECKLIST_REFUSAL_NEEDS_REASON),
        P::NotStored(reason) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_NOT_STORED).arg("reason", reason.as_str())
        }
    }
}

/// A refusal another part of the app worded, about the project the command
/// names (`rules_project_named`).
pub(super) fn project_refusal(reason: String) -> Translatable {
    Translatable::new(ids::CHECKLIST_REFUSAL_PROJECT).arg("reason", reason)
}
