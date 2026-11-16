//! A pick-list option: the value it stands for and its name, rendered from
//! the catalog when the view is built (#2059). A pick list hands back the
//! whole choice and the caller acts on its `value`, so no code reads the
//! shown text to decide what was picked. Equality (`PartialEq`) compares the
//! value and the label; both come from the same render, so [`chosen`] looks
//! the selection up by value only.

use signex_i18n::{I18n, MessageId};

/// One dropdown choice: the value and how it reads.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Choice<T> {
    pub value: T,
    pub label: String,
}

impl<T: Copy + PartialEq> Choice<T> {
    /// `values`, each named by its `label` id in `i18n`'s language.
    pub(crate) fn named(values: &[T], label: impl Fn(T) -> MessageId, i18n: &I18n) -> Vec<Self> {
        values
            .iter()
            .map(|&value| Self {
                value,
                label: i18n.message_plain(label(value), &[]),
            })
            .collect()
    }
}

/// The choice in `choices` that holds `value`, for the pick list's selection.
pub(crate) fn chosen<T: Clone + PartialEq>(choices: &[Choice<T>], value: &T) -> Option<Choice<T>> {
    choices.iter().find(|c| &c.value == value).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate_undo::AnnotateUndo;
    use crate::paste_prefs::PasteDesignators;

    fn labels<T>(choices: Vec<Choice<T>>) -> Vec<String> {
        choices.into_iter().map(|c| c.label).collect()
    }

    /// The pick lists read as they did when the names came from `Display`.
    #[test]
    fn choices_render_their_names_from_the_catalog() {
        let i18n = I18n::en_us();
        let modes = Choice::named(PasteDesignators::ALL, PasteDesignators::label, &i18n);
        assert_eq!(
            chosen(&modes, &PasteDesignators::Reset).map(|c| c.label),
            Some("Reset (R?, U?, ...)".to_owned())
        );
        assert_eq!(
            labels(modes),
            [
                "Keep unless taken",
                "Keep",
                "Reset (R?, U?, ...)",
                "Next free number"
            ]
        );
        assert_eq!(
            labels(Choice::named(AnnotateUndo::ALL, AnnotateUndo::label, &i18n)),
            ["One step for the whole project", "Per sheet"]
        );
    }
}
