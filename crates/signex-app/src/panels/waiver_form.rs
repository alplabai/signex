//! The waiver form under the row being waived, shared by ERC and DRC: one
//! required multi-line Reason, then Cancel and Waive (mockup row 88).
//!
//! The author is the git identity. Reading it means reading git config, so
//! the app resolves it once off the update loop and caches it in the Model
//! as a [`UserIdentity`]; this view only reads the cached value.

use iced::widget::{Column, Space, row, text_editor};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, TextEditorPlaceholder as _, text, theme_ext};

use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Lines of Reason the field shows before it scrolls.
const REASON_LINES: f32 = 3.0;

/// The multi-line Reason of a waiver being written. A clone keeps the cursor
/// and selection, so the copy the panel context holds edits where the user
/// typed.
#[derive(Default)]
pub struct WaiverReason(text_editor::Content);

impl WaiverReason {
    pub fn with_text(text: &str) -> Self {
        Self(text_editor::Content::with_text(text))
    }

    /// The Reason as typed, line breaks kept.
    pub fn text(&self) -> String {
        self.0.text()
    }

    /// Nothing but white space has been typed.
    pub fn is_empty(&self) -> bool {
        self.0.text().trim().is_empty()
    }

    /// Apply one edit, cursor move or selection from the field.
    pub fn perform(&mut self, action: text_editor::Action) {
        self.0.perform(action);
    }

    pub(super) fn content(&self) -> &text_editor::Content {
        &self.0
    }
}

impl Clone for WaiverReason {
    fn clone(&self) -> Self {
        let mut content = text_editor::Content::with_text(&self.0.text());
        content.move_to(self.0.cursor());
        Self(content)
    }
}

impl std::fmt::Debug for WaiverReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("WaiverReason").field(&self.0.text()).finish()
    }
}

impl PartialEq for WaiverReason {
    fn eq(&self, other: &Self) -> bool {
        self.0.text() == other.0.text()
    }
}

impl PartialEq<&str> for WaiverReason {
    fn eq(&self, other: &&str) -> bool {
        self.0.text() == *other
    }
}

/// The git identity a new waiver is authored by, cached in the Model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UserIdentity {
    /// No form has opened yet, so nothing was read.
    #[default]
    NotAsked,
    /// The read is in flight.
    Asking,
    /// The git author name.
    Known(String),
    /// Git has no identity: the author is stored empty.
    Unknown,
}

impl UserIdentity {
    /// The author a new waiver gets: the name, or empty.
    pub fn author(&self) -> String {
        match self {
            Self::Known(name) => name.clone(),
            Self::NotAsked | Self::Asking | Self::Unknown => String::new(),
        }
    }

    /// A form is opening and nobody has asked yet.
    pub fn needs_asking(&self) -> bool {
        matches!(self, Self::NotAsked)
    }

    /// What a finished read found; empty and blank names are none.
    pub fn resolved(name: Option<String>) -> Self {
        match name.map(|n| n.trim().to_string()) {
            Some(name) if !name.is_empty() => Self::Known(name),
            _ => Self::Unknown,
        }
    }
}

/// What the form shows.
pub struct ReasonFormView<'a> {
    pub reason: &'a WaiverReason,
    /// The Reason field's hint, worded by the panel.
    pub placeholder: Localized,
    /// Why the last Waive was refused.
    pub error: Option<Label<'a>>,
}

/// The messages the form sends. `confirm` is `None` while Waive cannot be
/// pressed yet.
pub struct ReasonFormEvents<M, R>
where
    R: Fn(text_editor::Action) -> M,
{
    pub reason: R,
    pub confirm: Option<M>,
    pub cancel: M,
}

/// The form (mockup row 88): one required Reason, then Cancel and Waive.
pub fn reason_form<'a, M, R>(
    view: ReasonFormView<'a>,
    events: ReasonFormEvents<M, R>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, M>
where
    M: Clone + 'a,
    R: Fn(text_editor::Action) -> M + 'a,
{
    let reason = reason_field(view.reason, view.placeholder, events.reason);
    form_column(
        vec![reason],
        view.error,
        events.confirm,
        events.cancel,
        i18n,
        tokens,
    )
}

fn reason_field<'a, M, R>(
    reason: &'a WaiverReason,
    placeholder: Localized,
    on_action: R,
) -> Element<'a, M>
where
    M: Clone + 'a,
    R: Fn(text_editor::Action) -> M + 'a,
{
    text_editor(reason.content())
        .placeholder_label(placeholder)
        .on_action(on_action)
        .size(TS.body)
        .padding(SP.s)
        .height(Length::Fixed(
            REASON_LINES * TS.body * crate::styles::LINE_HEIGHT + 2.0 * SP.s,
        ))
        .boxed()
}

/// The fields, an error line when refused, then Cancel and Waive on the
/// right.
fn form_column<'a, M>(
    fields: Vec<Element<'a, M>>,
    error: Option<Label<'a>>,
    confirm: Option<M>,
    cancel: M,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let mut col: Column<Element<'a, M>> = Column::new().spacing(SP.xs).width(Length::Fill);
    for field in fields {
        col = col.push(field);
    }
    if let Some(error) = error {
        col = col.push(
            text(error)
                .size(TS.caption)
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }
    col.push(
        row![
            Space::new().width(Length::Fill).height(Length::Shrink),
            dialog_button(
                t!(i18n, ids::PANELS_ERC_WAIVER_CANCEL),
                DialogButtonKind::Secondary,
                Some(cancel),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::PANELS_ERC_WAIVE),
                DialogButtonKind::Primary,
                confirm,
                tokens,
            ),
        ]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed(),
    )
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_author_starts_as_the_cached_identity_and_never_reads_on_its_own() {
        assert_eq!(UserIdentity::NotAsked.author(), "");
        assert_eq!(UserIdentity::Asking.author(), "");
        assert_eq!(UserIdentity::Known("C. Alp".into()).author(), "C. Alp");
        assert!(UserIdentity::NotAsked.needs_asking());
        assert!(!UserIdentity::Asking.needs_asking());
        assert!(!UserIdentity::Known("C. Alp".into()).needs_asking());
        assert_eq!(
            UserIdentity::resolved(Some("  C. Alp ".into())),
            UserIdentity::Known("C. Alp".into())
        );
        assert_eq!(
            UserIdentity::resolved(Some("   ".into())),
            UserIdentity::Unknown
        );
        assert_eq!(UserIdentity::resolved(None), UserIdentity::Unknown);
    }

    #[test]
    fn a_reason_keeps_its_lines_and_a_clone_keeps_the_text() {
        let reason = WaiverReason::with_text("Open-drain outputs\nwired-OR by design");
        assert_eq!(reason.text(), "Open-drain outputs\nwired-OR by design");
        assert_eq!(reason.clone(), reason);
        assert!(!reason.is_empty());
        assert!(WaiverReason::with_text("  \n ").is_empty());
        assert_eq!(WaiverReason::default().text(), "");
    }

    #[test]
    fn a_typed_reason_lands_where_the_cursor_is() {
        let mut reason = WaiverReason::default();
        reason.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
            std::sync::Arc::new("by design".to_string()),
        )));
        // The panel context's copy keeps the cursor: typing goes on at the end.
        let mut copy = reason.clone();
        copy.perform(text_editor::Action::Edit(text_editor::Edit::Insert('!')));
        assert_eq!(copy, "by design!");
    }
}
