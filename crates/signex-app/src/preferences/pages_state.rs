//! The Preferences dialog's page state beside its setting drafts: the
//! language draft and its preview, the pages with unsaved drafts, and the
//! view state of the Appearance, Editing, Keyboard shortcuts and Component
//! Classes pages. `UiState` holds it boxed, so a closed dialog costs one
//! pointer (#2412).

use std::sync::Arc;

use signex_i18n::{I18n, LanguagePref};

use super::{ClassesViewState, KeymapViewState, PagesViewState, PrefNav};

pub struct PreferencesPagesState {
    /// Draft of the interface language (#144a); Save applies it through
    /// `Signex::set_language`. Equals `i18n.pref()` when clean.
    pub draft_language: LanguagePref,
    /// The drafted language's catalog for the Language page's preview,
    /// built in `update` when the dialog opens and on each draft.
    pub language_preview: Option<Arc<I18n>>,
    /// Name of the language "Follow the system" resolves to, read when
    /// Preferences opens.
    pub system_language_name: String,
    /// The pages with unsaved drafts, for the nav's dots; kept with
    /// `preferences_dirty` by `recompute_preferences_dirty`.
    pub dirty_pages: Vec<PrefNav>,
    /// View state of the Appearance and Editing pages. Reset on open.
    pub pages_view: PagesViewState,
    /// View state of the Keyboard shortcuts page. Reset on open.
    pub keymap_view: KeymapViewState,
    /// View state of the Component Classes page. Reset on open.
    pub classes_view: ClassesViewState,
}

impl PreferencesPagesState {
    /// The state of a dialog not opened yet, its language draft at `language`.
    pub fn new(language: LanguagePref) -> Self {
        Self {
            draft_language: language,
            language_preview: None,
            system_language_name: String::new(),
            dirty_pages: Vec::new(),
            pages_view: PagesViewState::default(),
            keymap_view: KeymapViewState::default(),
            classes_view: ClassesViewState::default(),
        }
    }
}
