//! The catalog words of the Design Checklist tab: templates, stages, states,
//! links, results and the reason an edit was refused.

use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::project::{
    ChecklistLink, ChecklistProblem, ChecklistStage, ChecklistTemplate, ItemState, LinkResult,
};
use signex_widgets::Label;

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

/// What a link is to, as the table shows it: catalog words, or the design
/// object as it was typed.
pub(super) fn link_words(link: Option<&ChecklistLink>, i18n: &I18n) -> Label<'static> {
    match link {
        None => t!(i18n, ids::CHECKLIST_LINK_NONE).into(),
        Some(ChecklistLink::Erc) => t!(i18n, ids::CHECKLIST_LINK_ERC).into(),
        Some(ChecklistLink::Drc) => t!(i18n, ids::CHECKLIST_LINK_DRC).into(),
        Some(ChecklistLink::TitleBlock) => t!(i18n, ids::CHECKLIST_LINK_TITLE_BLOCK).into(),
        Some(ChecklistLink::Requirement { id }) => {
            t!(i18n, ids::CHECKLIST_LINK_REQUIREMENT, "id" => id.as_str()).into()
        }
        Some(ChecklistLink::Object { target }) => UserText::new(target.clone()).into(),
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
