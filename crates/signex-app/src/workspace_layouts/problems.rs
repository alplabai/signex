//! The Messages lines of a skipped layout or a refused import, in the app's
//! language. `store` returns the typed problems; this words them.

use signex_i18n::{I18n, Translatable, ids, t};

use super::store::{BodyProblem, ImportRefusal, LayoutProblem};

/// Why one layout was skipped, as a nested message.
fn reason(why: BodyProblem) -> Translatable {
    Translatable::new(match why {
        BodyProblem::NotAnObject => ids::WORKSPACE_LAYOUTS_WHY_NOT_OBJECT,
        BodyProblem::NoName => ids::WORKSPACE_LAYOUTS_WHY_NO_NAME,
        BodyProblem::PanelsUnreadable => ids::WORKSPACE_LAYOUTS_WHY_PANELS_UNREADABLE,
        BodyProblem::SplitUnknown => ids::WORKSPACE_LAYOUTS_WHY_SPLIT_UNKNOWN,
        BodyProblem::NothingStored => ids::WORKSPACE_LAYOUTS_WHY_NOTHING_STORED,
    })
}

/// The Messages line of `problem`.
pub fn problem_line(i18n: &I18n, problem: &LayoutProblem) -> String {
    let line = match problem {
        LayoutProblem::ListUnreadable => t!(i18n, ids::WORKSPACE_LAYOUTS_LIST_UNREADABLE),
        LayoutProblem::UnknownBuiltinEdit { key } => {
            t!(i18n, ids::WORKSPACE_LAYOUTS_SKIPPED_UNKNOWN_BUILTIN, "key" => key.as_str())
        }
        LayoutProblem::BuiltinEditSkipped { key, why } => t!(
            i18n,
            ids::WORKSPACE_LAYOUTS_SKIPPED_BUILTIN_EDIT,
            "key" => key.as_str(),
            "why" => reason(*why),
        ),
        LayoutProblem::BuiltinIdTaken { key } => {
            t!(i18n, ids::WORKSPACE_LAYOUTS_SKIPPED_BUILTIN_ID, "key" => key.as_str())
        }
        LayoutProblem::StoredLayoutSkipped { key, why } => t!(
            i18n,
            ids::WORKSPACE_LAYOUTS_SKIPPED_STORED,
            "key" => key.as_str(),
            "why" => reason(*why),
        ),
        LayoutProblem::ImportEntrySkipped { entry, why } => t!(
            i18n,
            ids::WORKSPACE_LAYOUTS_SKIPPED_IMPORT_ENTRY,
            "entry" => *entry,
            "why" => reason(*why),
        ),
    };
    line.into_string()
}

/// Why an import file was refused, as a nested message.
pub fn refusal(why: &ImportRefusal) -> Translatable {
    match why {
        ImportRefusal::NotALayoutsFile => {
            Translatable::new(ids::WORKSPACE_LAYOUTS_WHY_NOT_LAYOUTS_FILE)
        }
        ImportRefusal::Newer { format } => Translatable::new(ids::WORKSPACE_LAYOUTS_WHY_NEWER)
            .arg("format", *format)
            .arg("reads", super::store::EXPORT_FORMAT),
        ImportRefusal::NoLayoutsList => Translatable::new(ids::WORKSPACE_LAYOUTS_WHY_NO_LIST),
        ImportRefusal::ReadFailed(error) => {
            Translatable::new(ids::WORKSPACE_LAYOUTS_WHY_READ_FAILED).arg("error", error.clone())
        }
        ImportRefusal::NotJson(error) => {
            Translatable::new(ids::WORKSPACE_LAYOUTS_WHY_NOT_JSON).arg("error", error.clone())
        }
    }
}
