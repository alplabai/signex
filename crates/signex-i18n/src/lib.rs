//! Message catalogs and locale-aware formatting for the Signex UI.
//!
//! The only crate that imports Fluent or ICU4X; a backend change touches
//! this crate alone. It is a UI-layer crate: the domain crates cannot depend
//! on it (`scripts/check_arch_guard.sh`).
//!
//! An [`I18n`] is plain data held in the app state, not a mutable global; the
//! one shared instance, [`I18n::en_us`], is a process-wide immutable fallback
//! bundle with no app state. [`t!`]
//! renders a message to [`Localized`], which is for display only; state and
//! files keep a [`MessageId`] and its arguments.

mod bundle;
mod catalog;
mod format;
mod i18n;
mod locale;
mod numbers;
mod text;
mod types;

pub use format::{DateLength, ListKind};
pub use i18n::{I18n, LanguageOption};
pub use locale::LanguagePref;
pub use types::{Arg, LocalNumber, Localized, MessageId, UserText};

/// Every message id, defined together with the [`ids::ALL`] list so a new
/// id cannot be left out of the consistency tests. This is the only non-test place
/// `MessageId::new` is called (it is crate-private).
macro_rules! define_ids {
    ($($name:ident = $id:literal;)+) => {
        /// The message ids; each lives in `locales/en-US/`.
        pub mod ids {
            use crate::MessageId;

            $(pub const $name: MessageId = MessageId::new($id);)+

            /// Every id above.
            #[cfg(test)]
            pub(crate) const ALL: &[MessageId] = &[$($name),+];
        }
    };
}

define_ids! {
    DIALOG_AUTOSAVE_RECOVERY_AUTOSAVED_AGE = "dialog-autosave-recovery-autosaved-age";
    DIALOG_NEW_PROJECT_TITLE = "dialog-new-project-title";
    DIALOG_NEW_PROJECT_NAME_LABEL = "dialog-new-project-name-label";
    DIALOG_NEW_PROJECT_LOCATION_LABEL = "dialog-new-project-location-label";
    DIALOG_NEW_PROJECT_BROWSE = "dialog-new-project-browse";
    DIALOG_NEW_PROJECT_MAKE_FOLDER = "dialog-new-project-make-folder";
    DIALOG_NEW_PROJECT_PROFILE_HEADING = "dialog-new-project-profile-heading";
    DIALOG_NEW_PROJECT_PROFILE_HINT = "dialog-new-project-profile-hint";
    DIALOG_NEW_PROJECT_PROFILE_HOBBY_NAME = "dialog-new-project-profile-hobby-name";
    DIALOG_NEW_PROJECT_PROFILE_HOBBY_BLURB = "dialog-new-project-profile-hobby-blurb";
    DIALOG_NEW_PROJECT_PROFILE_STANDARD_NAME = "dialog-new-project-profile-standard-name";
    DIALOG_NEW_PROJECT_PROFILE_STANDARD_BLURB = "dialog-new-project-profile-standard-blurb";
    DIALOG_NEW_PROJECT_PROFILE_HDI_NAME = "dialog-new-project-profile-hdi-name";
    DIALOG_NEW_PROJECT_PROFILE_HDI_BLURB = "dialog-new-project-profile-hdi-blurb";
    DIALOG_NEW_PROJECT_PROFILE_CUSTOM_NAME = "dialog-new-project-profile-custom-name";
    DIALOG_NEW_PROJECT_PROFILE_CUSTOM_BLURB = "dialog-new-project-profile-custom-blurb";
    DIALOG_NEW_PROJECT_SUMMARY_CLEARANCE = "dialog-new-project-summary-clearance";
    DIALOG_NEW_PROJECT_SUMMARY_TRACKS = "dialog-new-project-summary-tracks";
    DIALOG_NEW_PROJECT_SUMMARY_CUSTOM_LAYERS = "dialog-new-project-summary-custom-layers";
    DIALOG_NEW_PROJECT_SUMMARY_CUSTOM_NOTE = "dialog-new-project-summary-custom-note";
    DIALOG_NEW_PROJECT_CUSTOMIZE = "dialog-new-project-customize";
    DIALOG_NEW_PROJECT_CUSTOMIZE_OPTIONAL = "dialog-new-project-customize-optional";
    DIALOG_NEW_PROJECT_PCB_HEADING = "dialog-new-project-pcb-heading";
    DIALOG_NEW_PROJECT_SHEET_HEADING = "dialog-new-project-sheet-heading";
    DIALOG_NEW_PROJECT_FAB_LABEL = "dialog-new-project-fab-label";
    DIALOG_NEW_PROJECT_FAB_STANDARD = "dialog-new-project-fab-standard";
    DIALOG_NEW_PROJECT_FAB_ADVANCED = "dialog-new-project-fab-advanced";
    DIALOG_NEW_PROJECT_FAB_HDI = "dialog-new-project-fab-hdi";
    DIALOG_NEW_PROJECT_FAB_IMPORT = "dialog-new-project-fab-import";
    DIALOG_NEW_PROJECT_FAB_NOTE = "dialog-new-project-fab-note";
    DIALOG_NEW_PROJECT_FAB_NOTE_IMPORTED = "dialog-new-project-fab-note-imported";
    DIALOG_NEW_PROJECT_IMPORT_FAILED = "dialog-new-project-import-failed";
    DIALOG_NEW_PROJECT_LAYERS_LABEL = "dialog-new-project-layers-label";
    DIALOG_NEW_PROJECT_FIELD_CLEARANCE = "dialog-new-project-field-clearance";
    DIALOG_NEW_PROJECT_FIELD_TRACK = "dialog-new-project-field-track";
    DIALOG_NEW_PROJECT_FIELD_VIA = "dialog-new-project-field-via";
    DIALOG_NEW_PROJECT_FIELD_DRILL = "dialog-new-project-field-drill";
    DIALOG_NEW_PROJECT_UNIT_MM = "dialog-new-project-unit-mm";
    DIALOG_NEW_PROJECT_SCH_SIZE = "dialog-new-project-sch-size";
    DIALOG_NEW_PROJECT_SCH_TITLE_BLOCK = "dialog-new-project-sch-title-block";
    DIALOG_NEW_PROJECT_SCH_TITLE_BLOCK_DEFAULT = "dialog-new-project-sch-title-block-default";
    DIALOG_NEW_PROJECT_SCH_TITLE_BLOCK_NONE = "dialog-new-project-sch-title-block-none";
    DIALOG_NEW_PROJECT_SCH_DESIGNATORS = "dialog-new-project-sch-designators";
    DIALOG_NEW_PROJECT_SHEET_LANDSCAPE = "dialog-new-project-sheet-landscape";
    DIALOG_NEW_PROJECT_PREVIEW_HEADING = "dialog-new-project-preview-heading";
    DIALOG_NEW_PROJECT_PREVIEW_LAYERS = "dialog-new-project-preview-layers";
    DIALOG_NEW_PROJECT_PREVIEW_BOARD = "dialog-new-project-preview-board";
    DIALOG_NEW_PROJECT_PREVIEW_SHEET = "dialog-new-project-preview-sheet";
    DIALOG_NEW_PROJECT_PREVIEW_NOTE = "dialog-new-project-preview-note";
    DIALOG_NEW_PROJECT_STACK_SILK_TOP = "dialog-new-project-stack-silk-top";
    DIALOG_NEW_PROJECT_STACK_MASK_TOP = "dialog-new-project-stack-mask-top";
    DIALOG_NEW_PROJECT_STACK_COPPER_TOP = "dialog-new-project-stack-copper-top";
    DIALOG_NEW_PROJECT_STACK_COPPER_INNER = "dialog-new-project-stack-copper-inner";
    DIALOG_NEW_PROJECT_STACK_COPPER_BOTTOM = "dialog-new-project-stack-copper-bottom";
    DIALOG_NEW_PROJECT_STACK_PREPREG = "dialog-new-project-stack-prepreg";
    DIALOG_NEW_PROJECT_STACK_CORE = "dialog-new-project-stack-core";
    DIALOG_NEW_PROJECT_STACK_MASK_BOTTOM = "dialog-new-project-stack-mask-bottom";
    DIALOG_NEW_PROJECT_FOOTER_HINT = "dialog-new-project-footer-hint";
    DIALOG_NEW_PROJECT_CANCEL = "dialog-new-project-cancel";
    DIALOG_NEW_PROJECT_CREATE = "dialog-new-project-create";
    DIALOG_NEW_PROJECT_ERR_NAME_EMPTY = "dialog-new-project-err-name-empty";
    DIALOG_NEW_PROJECT_ERR_NAME_INVALID = "dialog-new-project-err-name-invalid";
    DIALOG_NEW_PROJECT_ERR_LOCATION_EMPTY = "dialog-new-project-err-location-empty";
    DIALOG_NEW_PROJECT_ERR_LOCATION_MISSING = "dialog-new-project-err-location-missing";
    DIALOG_NEW_PROJECT_ERR_EXISTS = "dialog-new-project-err-exists";
    DIALOG_NEW_PROJECT_ERR_FOLDER_NOT_EMPTY = "dialog-new-project-err-folder-not-empty";
    DIALOG_NEW_PROJECT_ERR_NOT_NUMBER = "dialog-new-project-err-not-number";
    DIALOG_NEW_PROJECT_ERR_RANGE = "dialog-new-project-err-range";
    DIALOG_NEW_PROJECT_ERR_DRILL = "dialog-new-project-err-drill";
    DIALOG_NEW_PROJECT_PICK_LOCATION_TITLE = "dialog-new-project-pick-location-title";
    DIALOG_NEW_PROJECT_PICK_RULES_TITLE = "dialog-new-project-pick-rules-title";
    DIALOG_NEW_PROJECT_PICK_RULES_FILTER = "dialog-new-project-pick-rules-filter";
    DIALOG_NEW_PROJECT_ENTRY_ROW = "dialog-new-project-entry-row";
    DIALOG_NEW_PROJECT_FIELD_THICKNESS = "dialog-new-project-field-thickness";
    DIALOG_NEW_PROJECT_ERR_CREATE_FAILED = "dialog-new-project-err-create-failed";
    DIALOG_NEW_PROJECT_ERR_RULES_READ = "dialog-new-project-err-rules-read";
    DIALOG_NEW_PROJECT_ERR_RULES_SYNTAX = "dialog-new-project-err-rules-syntax";
    DIALOG_NEW_PROJECT_ERR_RULES_NO_PCB = "dialog-new-project-err-rules-no-pcb";
    DIALOG_NEW_PROJECT_ERR_RULES_MISSING = "dialog-new-project-err-rules-missing";
    DIALOG_NEW_PROJECT_ERR_RULES_VALUE = "dialog-new-project-err-rules-value";
    DIALOG_NEW_PROJECT_ERR_RULES_TOO_LARGE = "dialog-new-project-err-rules-too-large";
    DIALOG_NEW_PROJECT_CREATING = "dialog-new-project-creating";
    DIALOG_NEW_PROJECT_ERR_CREATE_PENDING = "dialog-new-project-err-create-pending";
    DIALOG_NEW_PROJECT_SETUP_UNREADABLE = "dialog-new-project-setup-unreadable";
    DIALOG_NEW_PROJECT_NAME_PLACEHOLDER = "dialog-new-project-name-placeholder";
    DIALOG_NEW_PROJECT_LOCATION_PLACEHOLDER = "dialog-new-project-location-placeholder";
    LANGUAGE_SYSTEM_DEFAULT = "language-system-default";
    MENU_FILE = "menu-file";
    OVERLAY_PLACEMENT_PAUSED_ESC_HINT = "overlay-placement-paused-esc-hint";
    PREFERENCES_AGENT_CONNECTIONS_LABEL = "preferences-agent-connections-label";
    PREFERENCES_AGENT_CONNECTIONS_HELP = "preferences-agent-connections-help";
}

/// Render a message: `t!(i18n, ids::MENU_FILE)` or, with arguments,
/// `t!(i18n, ID, "count" => 3_i64, "name" => "R1")`.
#[macro_export]
macro_rules! t {
    ($i18n:expr, $id:expr) => {
        $i18n.message($id, &[])
    };
    ($i18n:expr, $id:expr, $($key:literal => $value:expr),+ $(,)?) => {
        $i18n.message($id, &[$(($key, $crate::Arg::from($value))),+])
    };
}

#[cfg(test)]
mod consistency_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod tests;
