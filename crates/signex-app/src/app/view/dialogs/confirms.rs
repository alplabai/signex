//! The unsaved-count sentence of the Close Project confirm. The Quit-app
//! confirm is in `quit_confirm.rs`, Remove from Project in `remove.rs` and
//! the Close Project confirm in `close_project.rs`.

use signex_i18n::{I18n, MessageId, t};

/// The sentence `id` naming the unsaved count: "1 unsaved document",
/// "2 unsaved documents"; with library rows in the mix, "3 unsaved items"
/// (singular "1 unsaved item"). `project` names the project when it says so.
pub(super) fn unsaved_line(
    i18n: &I18n,
    id: MessageId,
    documents: usize,
    rows: usize,
    project: Option<&str>,
) -> String {
    let (count, kind) = if rows == 0 {
        (documents, "documents")
    } else {
        (documents + rows, "items")
    };
    let project = project.unwrap_or_default();
    t!(i18n, id, "count" => count, "kind" => kind, "project" => project).into_string()
}

#[cfg(test)]
mod copy_tests {
    use signex_i18n::{I18n, LanguagePref, ids};

    use super::super::quit_confirm::QuitSummary;
    use super::unsaved_line;

    fn plain(text: String) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

    /// The quit confirm's fact line for `documents` files and `rows`
    /// Component Preview rows in `projects` projects and `libraries`
    /// libraries.
    fn quit_line(i18n: &I18n, documents: usize, rows: usize, places: (usize, usize)) -> String {
        let (projects, libraries) = places;
        QuitSummary {
            count: documents + rows,
            has_rows: rows > 0,
            projects,
            libraries,
        }
        .line(i18n)
        .into_string()
    }

    #[test]
    fn the_unsaved_count_is_singular_or_plural_not_a_parenthesised_s() {
        let en = I18n::en_us();
        let line = |documents, rows, places| plain(quit_line(&en, documents, rows, places));
        assert_eq!(line(1, 0, (0, 0)), "1 unsaved document.");
        assert_eq!(line(2, 0, (1, 0)), "2 unsaved documents in 1 project.");
        assert_eq!(
            line(1, 1, (1, 1)),
            "2 unsaved items in 1 project and 1 library."
        );
        assert_eq!(line(0, 1, (0, 1)), "1 unsaved item in 1 library.");
        assert_eq!(
            line(4, 1, (2, 3)),
            "5 unsaved items in 2 projects and 3 libraries."
        );
        assert_eq!(line(3, 0, (0, 2)), "3 unsaved documents in 2 libraries.");
        // The Close Project intro counts the rows and names project
        // settings apart (mockup row 25).
        assert_eq!(
            plain(unsaved_line(
                &en,
                ids::DIALOG_CLOSE_PROJECT_DETAIL,
                2,
                0,
                Some("Demo")
            )),
            "'Demo' has 2 unsaved documents."
        );
    }

    /// The multi-line dialog texts keep the English the views built before
    /// the catalog, line break included.
    #[test]
    fn multi_line_dialog_texts_keep_their_english() {
        let en = I18n::en_us();
        let undo = || signex_i18n::Arg::from("Ctrl+Z");
        // #76 / AV-05: the reset detail names its scope; undo is stated once,
        // in the confirm's banner, the same way as the Annotate footer.
        let scope = |renumber: &'static str| {
            en.message_plain(
                ids::DIALOG_ANNOTATE_RESET_DETAIL,
                &[
                    ("sheets", signex_i18n::Arg::from(5_usize)),
                    ("symbols", signex_i18n::Arg::from(42_usize)),
                    ("renumber", signex_i18n::Arg::from(renumber)),
                ],
            )
        };
        assert_eq!(
            scope("yes"),
            "Every reference designator on the 5 ticked sheets (42 symbols) is reset to '?', \
             then renumbered from 1."
        );
        assert_eq!(
            scope("no"),
            "Every reference designator on the 5 ticked sheets (42 symbols) is reset to '?'."
        );
        assert_eq!(
            en.message_plain(ids::DIALOG_ANNOTATE_RESET_ONE_STEP, &[("undo", undo())]),
            "One undo step: Ctrl+Z restores every sheet."
        );
        // The recovery hint is split into the callout (Restore) and the
        // caption (Discard, Decide later); together they keep every word.
        let hint = format!(
            "{} {}",
            en.message_plain(ids::DIALOG_RECOVERY_RESTORE_HINT, &[("undo", undo())]),
            en.message_plain(ids::DIALOG_RECOVERY_LATER_HINT, &[])
        );
        assert_eq!(
            hint,
            "Restore loads the copy as unsaved edits; nothing is written to the file until \
             you save. Restoring replaces the open document and cannot be undone with \
             Ctrl+Z. Discard deletes the copy. Decide later keeps it for the next time \
             this file is opened, and pauses autosave for this document for the rest of the \
             session."
        );
    }

    /// #2058: the confirm sentences follow the language in force.
    #[test]
    fn the_unsaved_sentence_follows_the_language() {
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        let en = I18n::en_us();
        let english = quit_line(&en, 2, 0, (1, 0));
        assert_ne!(quit_line(&xa, 2, 0, (1, 0)), english);
        assert_eq!(plain(english), "2 unsaved documents in 1 project.");
        let close = unsaved_line(&en, ids::DIALOG_CLOSE_PROJECT_DETAIL, 2, 0, Some("Demo"));
        assert_ne!(
            unsaved_line(&xa, ids::DIALOG_CLOSE_PROJECT_DETAIL, 2, 0, Some("Demo")),
            close
        );
    }
}
