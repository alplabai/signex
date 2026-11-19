//! What the Remove from Project modal shows (#2262): the cards it offers,
//! the one selected, and the notes around them. The view draws it and the
//! window sizing measures it, so the two cannot disagree. A pure read of the
//! Model, like the view (`preselect_lone_delete` aside, run by the opener).

use signex_i18n::{I18n, Localized, MessageId, ids, t};

use crate::app::handlers::dock::project_navigation::sheet_users::RemoveBlockers;
use crate::app::path_ops::PlacingSymbol;
use crate::app::{RemoveChoice, RemoveDialogState, Signex};
use crate::styles::DialogButtonKind;

/// The cards a Remove offers. Exclude is withheld for a sheet that is not a
/// page of the project; Delete for a target outside the project folder
/// (#139) and for a sheet another sheet places, or may place (#1618).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RemoveOffer {
    pub(crate) exclude: bool,
    pub(crate) delete: bool,
}

impl RemoveOffer {
    pub(crate) fn of(state: &RemoveDialogState, blockers: &RemoveBlockers) -> Self {
        Self {
            exclude: !blockers.unlisted,
            delete: !state.is_outside_project
                && !blockers.blocks_delete(is_library(&state.target_path)),
        }
    }

    pub(crate) fn offers(self, choice: RemoveChoice) -> bool {
        match choice {
            RemoveChoice::ExcludeFromProject => self.exclude,
            RemoveChoice::DeleteFile => self.delete,
        }
    }

    /// The selected card: `picked` while it is offered, else Exclude (the
    /// safe choice). Delete is never selected unless picked: a dialog that
    /// opens with Delete alone picks it at open (`preselect_lone_delete`),
    /// and when Exclude stops being offered after open the footer has no
    /// action until the user picks Delete. `None`: no footer action.
    pub(crate) fn selected(self, picked: Option<RemoveChoice>) -> Option<RemoveChoice> {
        picked
            .filter(|choice| self.offers(*choice))
            .or_else(|| self.exclude.then_some(RemoveChoice::ExcludeFromProject))
    }
}

/// The footer button that carries out `choice`: its label and its role.
pub(crate) fn footer_action(choice: RemoveChoice) -> (MessageId, DialogButtonKind) {
    match choice {
        RemoveChoice::ExcludeFromProject => {
            (ids::DIALOG_REMOVE_ACTION_EXCLUDE, DialogButtonKind::Primary)
        }
        RemoveChoice::DeleteFile => (
            ids::DIALOG_REMOVE_ACTION_DELETE,
            DialogButtonKind::DangerSolid,
        ),
    }
}

/// A note between the intro and the cards.
#[derive(Debug, Clone)]
pub(crate) enum RemoveNote {
    /// Catalog text: why a card is not offered.
    Said(Localized),
    /// A library's full path (names repeat across folders) and, when it is
    /// mounted, how its parts are used. The usage line is English, built by
    /// the handler its own tests read (#2061).
    Library { path: String, usage: Option<String> },
}

impl RemoveNote {
    /// The note's text, for measuring how many lines it wraps to.
    pub(crate) fn plain(&self) -> String {
        match self {
            Self::Said(text) => text.to_string(),
            Self::Library { path, usage } => match usage {
                Some(usage) => format!("{path}\n{usage}"),
                None => path.clone(),
            },
        }
    }
}

/// One radio card.
#[derive(Debug, Clone)]
pub(crate) struct RemoveCard {
    pub(crate) choice: RemoveChoice,
    pub(crate) title: MessageId,
    pub(crate) description: Localized,
    /// What this choice leaves behind, shown as a warning inside the card.
    pub(crate) warning: Option<Localized>,
}

/// Everything the first stage of the Remove modal shows below its intro.
#[derive(Debug, Clone)]
pub(crate) struct RemoveContent {
    pub(crate) notes: Vec<RemoveNote>,
    /// Exclude first, then Delete; a card a blocker rules out is left out.
    pub(crate) cards: Vec<RemoveCard>,
    pub(crate) selected: Option<RemoveChoice>,
}

impl RemoveContent {
    /// The footer button for the selected card; `None` when no card is
    /// offered, which leaves Cancel alone.
    pub(crate) fn footer_action(&self) -> Option<(MessageId, DialogButtonKind)> {
        self.selected.map(footer_action)
    }
}

fn is_library(path: &std::path::Path) -> bool {
    crate::app::file_extension::has_extension(path, "snxlib")
}

impl Signex {
    /// The cards the Remove modal offers for `state` right now.
    pub(crate) fn remove_offer(&self, state: &RemoveDialogState) -> RemoveOffer {
        RemoveOffer::of(state, &self.remove_blockers(state))
    }

    /// Just after the Remove modal opens: when Delete is the only card, it
    /// is shown selected. Only at open, so a Delete that becomes the only
    /// card later is never selected without a click.
    pub(crate) fn preselect_lone_delete(&mut self) {
        let Some(state) = self.ui_state.remove_dialog.as_ref() else {
            return;
        };
        let offer = self.remove_offer(state);
        if offer.delete
            && !offer.exclude
            && let Some(state) = self.ui_state.remove_dialog.as_mut()
        {
            state.choice = Some(RemoveChoice::DeleteFile);
        }
    }

    /// What the first stage of the Remove modal shows for `state`.
    pub(crate) fn remove_content(&self, state: &RemoveDialogState, i18n: &I18n) -> RemoveContent {
        let blockers = self.remove_blockers(state);
        let offer = RemoveOffer::of(state, &blockers);
        let library = is_library(&state.target_path);
        let symbols = || PlacingSymbol::describe_all(&blockers.symbols);

        let mut notes = Vec::new();
        if !blockers.other_parents.is_empty() {
            notes.push(RemoveNote::Said(
                t!(i18n, ids::DIALOG_REMOVE_PLACED_BY, "parents" => blockers.parent_names()),
            ));
        }
        if let Some(why) = blockers.unknown.as_deref().filter(|_| !library) {
            notes.push(RemoveNote::Said(
                t!(i18n, ids::DIALOG_REMOVE_PLACEMENT_UNKNOWN, "reason" => why),
            ));
        }
        if blockers.unlisted {
            notes.push(RemoveNote::Said(t!(i18n, ids::DIALOG_REMOVE_NOT_A_PAGE)));
        }
        if library {
            notes.push(RemoveNote::Library {
                path: state.target_path.display().to_string(),
                usage: self.library_usage_line(state),
            });
        }

        let mut cards = Vec::new();
        if offer.exclude {
            cards.push(RemoveCard {
                choice: RemoveChoice::ExcludeFromProject,
                title: ids::DIALOG_REMOVE_EXCLUDE,
                description: t!(i18n, self.exclude_description(state)),
                warning: blockers
                    .placed
                    .then(|| t!(i18n, ids::DIALOG_REMOVE_STILL_PLACED, "symbols" => symbols())),
            });
        }
        if offer.delete {
            let description = if state.is_directory_package {
                ids::DIALOG_REMOVE_DELETE_PACKAGE
            } else if library {
                ids::DIALOG_REMOVE_DELETE_LIBRARY
            } else {
                ids::DIALOG_REMOVE_DELETE_FILE
            };
            // Delete is only offered when no other sheet places the file, so
            // every symbol it leaves dangling sits on the row's own parent.
            cards.push(RemoveCard {
                choice: RemoveChoice::DeleteFile,
                title: ids::DIALOG_REMOVE_DELETE,
                description: t!(i18n, description),
                warning: blockers.own_parent.is_some().then(|| {
                    t!(
                        i18n,
                        ids::DIALOG_REMOVE_DANGLING,
                        "count" => blockers.symbols.len(),
                        "symbols" => symbols()
                    )
                }),
            });
        }
        RemoveContent {
            notes,
            cards,
            selected: offer.selected(state.choice),
        }
    }

    /// What the Exclude card says the file keeps: a shared or outside
    /// library is not deleted; a file in a project folder that is a Git
    /// repository stays in Git too. Read from the probed Model fact, never
    /// the disk; until the probe answers, the plainer wording (true either
    /// way) is shown.
    fn exclude_description(&self, state: &RemoveDialogState) -> MessageId {
        if state.is_outside_project {
            return ids::DIALOG_REMOVE_EXCLUDE_OUTSIDE;
        }
        let in_git = state
            .tree_path
            .first()
            .and_then(|i| self.document_state.projects.get(*i))
            .is_some_and(|project| {
                self.document_state
                    .project_fs
                    .facts
                    .knows_present(&crate::app::state::project_fs::git_marker(project.dir()))
            });
        if in_git {
            ids::DIALOG_REMOVE_EXCLUDE_INSIDE_GIT
        } else {
            ids::DIALOG_REMOVE_EXCLUDE_INSIDE
        }
    }
}
