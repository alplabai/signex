//! Preferences dialog: the page list on the left, the open page on the
//! right, the Apply / OK footer under both (mockup row 143).
//!
//! Opened via Tools > Preferences (or keyboard shortcut). Every page draws
//! on the `setting_row` grid.

use crate::render_config::{GridStyle, PinSelectionMode};
use iced::widget::{Space, Widget, column, container, row, scrollable};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeId;
use signex_widgets::{button, raw, text};

use crate::styles::{Spacing, TypeScale};
/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

mod annotation;
mod appearance;
mod component_classes;
mod distributors;
mod editing;
mod extension_page;
#[cfg(test)]
pub(crate) use extension_page::{
    row_texts as extension_row_texts, value_kind as extension_value_kind,
};
mod fast_grids;
mod graphite_fields;
#[cfg(test)]
pub(crate) use graphite_fields::setting_labels;
mod keycaps;
mod keymap;
mod language;
mod pages_state;
mod pages_view;
pub(crate) use pages_view::controls::{GLYPH_EXPAND, GLYPH_SHRINK};
mod pcb_defaults;
mod pcb_display;
mod profiles;
mod schematic_display;
mod setting_row;
mod shell;
mod system_general;
mod theme_cards;
mod widgets;

pub use crate::app::core::rule_profile_page::{ProfileEditorTab, ProfileNameAction};
use annotation::content_annotation;
use appearance::content_appearance;
use component_classes::content_component_classes;
pub(crate) use component_classes::saved_classes;
pub use component_classes::{ClassesViewMsg, ClassesViewState};
use distributors::content_library_distributors;
use editing::content_editing;
use keymap::content_keyboard_shortcuts;
pub use keymap::{KeymapViewMsg, KeymapViewState};
use language::content_language;
pub use pages_state::PreferencesPagesState;
pub use pages_view::{PagesViewMsg, PagesViewState};
pub use pcb_defaults::PcbDefaultsMsg;
use pcb_defaults::content_pcb_defaults;
use profiles::content_profiles;
pub use profiles::{ProfileAction, ProfilePageMsg, ProfilesMsg};
pub use schematic_display::BodyFillMsg;
use schematic_display::{DisplayPage, content_schematic_display};

// Shared style / text / button helpers live in `widgets`; re-export them
// at their original module-local visibility so the shell here and every
// section submodule keep resolving them through `use super::*`.
pub(in crate::preferences) use widgets::{
    danger_button_style, text_muted, text_primary, text_warning,
};

// ─── Navigation Items ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefNav {
    /// Theme, fonts, grids, rendering, scale, navigation and window.
    Appearance,
    /// The interface language (mockup row 144a).
    Language,
    /// Settings that cover every editor: autosave of recovery copies and
    /// agent connections (mockup row 147).
    General,
    /// Schematic annotation settings (the undo scope of an annotate run).
    Annotation,
    /// Schematic editing behaviour (rubber-banding wires on move).
    Editing,
    /// How the schematic is drawn (symbol body fill).
    SchematicDisplay,
    /// What a new board dimension starts from (#2868).
    PcbDefaults,
    /// How the board is drawn (how much other copper dims while a net is
    /// highlighted, #2514).
    PcbDisplay,
    /// Keyboard-shortcut profile editor — pick / create / delete /
    /// import / export a profile and rebind individual commands via the
    /// chord recorder. Backed by `UiState::preferences_keymap_editor`.
    Keyboard,
    /// Rule profiles: the library of named ERC rule sets, the default for
    /// new projects, and the editor for each profile (#1608).
    RuleProfiles,
    /// v0.9 Library settings — Distributor APIs, lifecycle defaults.
    LibraryDistributors,
    /// The component classes every new library is seeded with (the class
    /// registry itself is per library, in each `.snxlib` manifest). Backed
    /// by `prefs.json::component_classes`; defaults seed from
    /// `fonts::default_component_classes` on a fresh install.
    ComponentClasses,
    /// The page of an installed extension that added preferences (#2495):
    /// the extension's install index. It lists each preference with its
    /// kind and its value; it holds no draft. Not in [`Self::ALL`]: the nav
    /// lists these after the built-in pages, only when one is installed.
    Extension(usize),
}

impl PrefNav {
    /// The pages in nav order (mockup row 143): System, Schematic, PCB
    /// (#2868), Validation, Library.
    pub const ALL: &'static [PrefNav] = &[
        PrefNav::Appearance,
        PrefNav::Language,
        PrefNav::Keyboard,
        PrefNav::General,
        PrefNav::Annotation,
        PrefNav::Editing,
        PrefNav::SchematicDisplay,
        PrefNav::PcbDefaults,
        PrefNav::PcbDisplay,
        PrefNav::RuleProfiles,
        PrefNav::LibraryDistributors,
        PrefNav::ComponentClasses,
    ];

    /// The English page name, which nav search also matches.
    pub fn label(self) -> &'static str {
        match self {
            PrefNav::Appearance => "Appearance",
            PrefNav::Language => "Language",
            PrefNav::Annotation => "Annotation",
            PrefNav::Editing => "Editing",
            PrefNav::SchematicDisplay => "Display",
            PrefNav::PcbDefaults => "PCB Defaults",
            PrefNav::PcbDisplay => "PCB Display",
            PrefNav::Keyboard => "Keyboard Shortcuts",
            PrefNav::General => "General",
            PrefNav::RuleProfiles => "Rule Profiles",
            PrefNav::LibraryDistributors => "Distributor APIs",
            PrefNav::ComponentClasses => "Component Classes",
            PrefNav::Extension(_) => "Extension",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            PrefNav::Appearance => "System",
            PrefNav::Language => "System",
            PrefNav::Annotation => "Schematic",
            PrefNav::Editing => "Schematic",
            PrefNav::SchematicDisplay => "Schematic",
            PrefNav::PcbDefaults => "PCB",
            PrefNav::PcbDisplay => "PCB",
            PrefNav::Keyboard => "System",
            PrefNav::General => "System",
            PrefNav::RuleProfiles => "Validation",
            PrefNav::LibraryDistributors => "Library",
            PrefNav::ComponentClasses => "Library",
            PrefNav::Extension(_) => "Extensions",
        }
    }
}

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PrefMsg {
    /// Inert message: handled by doing nothing.
    Noop,
    /// Navigate to a category.
    Nav(PrefNav),
    /// The Graphite nav search text changed; only filters the nav rows.
    NavSearchChanged(String),
    /// Close without saving (only if not dirty; app ignores if dirty).
    Close,
    /// Discard all unsaved changes and close.
    DiscardAndClose,
    /// Commit the current draft and keep the dialog open.
    Save,
    /// Commit the current draft, then close (the Graphite footer's OK).
    SaveAndClose,
    /// Every `prefs.json` write a Save started has finished (#1351). Save
    /// returns them as background tasks, so the file-health probe that
    /// reports a refused write has to wait for this.
    SaveWritesFinished,
    /// Update the draft theme (not applied until Save).
    DraftTheme(ThemeId),
    /// Update the draft UI font (not applied until Save).
    DraftFont(String),
    /// Update draft visible-grid render style (applies as live preview).
    DraftGridStyle(GridStyle),
    /// Toggle the experimental PCB GPU render. Applies as live preview,
    /// persisted on Save.
    DraftPcbGpuRender(bool),
    /// Anti-aliasing on or off; applies at the next start.
    DraftAntialiasing(bool),
    /// Choose the undo scope of an annotate run. Persisted on Save.
    DraftAnnotateUndo(crate::annotate_undo::AnnotateUndo),
    /// Toggle "Keep wires connected when moving". Persisted on Save.
    DraftMoveKeepsWires(bool),
    /// Toggle "Select by drag direction". Persisted on Save.
    DraftDragDirectionSelect(bool),
    /// Toggle "Show net info on hover". Persisted on Save.
    DraftNetHoverEnabled(bool),
    /// Toggle "Pan at the canvas edge" (#2872). Persisted on Save.
    DraftEdgePanEnabled(bool),
    /// Choose the edge pan speed. Persisted on Save.
    DraftEdgePanSpeed(crate::app::edge_pan::EdgePanSpeed),
    /// The "Fill symbol bodies" rows changed. Persisted on Save.
    SymbolBodyFill(BodyFillMsg),
    /// Toggle "Cross out not-fitted parts" (#1294). Persisted on Save.
    DraftCrossOutNotFitted(bool),
    /// A PCB Defaults row changed (#2868). Persisted on Save.
    PcbDefaults(PcbDefaultsMsg),
    /// Toggle "Two-finger scroll pans". Persisted on Save.
    DraftTrackpadPan(bool),
    /// Choose "Mouse wheel": zoom or scroll the view. Persisted on Save.
    DraftWheelMode(crate::canvas::WheelMode),
    /// Choose "Layer switcher": the strip or a dropdown. Persisted on Save.
    DraftLayerSwitcher(crate::layer_strip_prefs::LayerSwitcher),
    /// Choose "Selected option": Neutral or Accent. Persisted on Save.
    DraftSelectedSegment(signex_types::theme::SelectedSegmentLook),
    /// Toggle "Remember window size and position". Persisted on Save.
    DraftRememberWindowGeometry(bool),
    /// Move the "Increment step" (-100..=100, never 0). Persisted on Save.
    DraftIncrementStep(i32),
    /// Choose what a pasted part is called. Persisted on Save.
    DraftPasteDesignators(crate::paste_prefs::PasteDesignators),
    /// Choose where the window's minimize / maximize / close buttons go.
    /// Persisted on Save.
    DraftWindowControlsPlacement(crate::chrome::controls::WindowControlsPlacement),
    /// Toggle opt-in autosave of recovery copies (#104). Persisted on Save.
    DraftAutosaveEnabled(bool),
    /// Toggle "Allow agent connections" (#1924). Persisted on Save; turning it
    /// off closes the listener and drops its connections.
    DraftAgentConnectionsEnabled(bool),
    /// Update the default symbol-editor grid size. Persisted on Save.
    DraftSymbolGridSize(f32),
    /// Text typed into the Fast grid 1 (`0`) or Fast grid 2 (`1`) field,
    /// in mm. Checked and persisted on Save (#1672).
    DraftFastGrid(usize, String),
    /// Update the symbol-editor grid style (applies immediately, persisted).
    DraftSymbolGridStyle(GridStyle),
    /// Update the wheel-zoom speed (percent per notch, 5..=50) shared by
    /// every canvas. Persisted on Save.
    DraftZoomStepPercent(u8),
    /// Update how much copper off a highlighted board net dims (percent,
    /// 0..=100). Persisted on Save (#2514).
    DraftPcbDimOthersPercent(u8),
    /// Update the interface scale (percent, 80..=200 in 10 % steps).
    /// Applied to every window on Save.
    DraftUiScalePercent(u16),
    /// Update the symbol-editor pin-selection mode (persisted on change).
    DraftSymbolPinSelection(PinSelectionMode),
    /// Choose the interface language (mockup row 144a). A draft: Save switches every
    /// window to it, with no restart; Discard drops it.
    DraftLanguage(signex_i18n::LanguagePref),
    /// Rule Profiles pane (#1608). Its edits are written to the profile
    /// files at once, so they are not drafts and never mark the dialog dirty.
    Profiles(ProfilesMsg),
    /// Move an unloadable `prefs.json` aside so writes can resume from a
    /// fresh file, keeping the original on disk beside it (#602). Offered
    /// only from the banner, and the handler re-checks the file before
    /// acting: a healthy file is never moved.
    ResetPrefsFile,
    /// The broken-file banner's Open folder: show the folder that holds
    /// `prefs.json` in the file manager (mockup row 143).
    PrefsFolderOpenRequested,
    /// The Appearance and Editing pages' view state.
    PagesView(PagesViewMsg),
    /// The Keyboard shortcuts page's view state.
    KeymapView(KeymapViewMsg),
    /// The Component Classes page's view state.
    ClassesView(ClassesViewMsg),
    /// Library → Distributor APIs pane forwarded a settings message.
    /// Folded into `PrefMsg` so the Preferences modal can mount the
    /// live panel as a first-class pane without breaking the
    /// existing `Message::Preferences(PreferencesMsg::Inner(_))` plumbing. The handler
    /// re-dispatches via `Message::Library(LibraryMessage::Settings)`.
    LibrarySettings(crate::library::messages::SettingsMsg),
    /// Component-class editor messages — all edits land on the
    /// `preferences_draft_component_classes` mirror so Cancel /
    /// Discard reverts cleanly. Save copies the draft into the live
    /// `component_classes` list and persists via
    /// `fonts::write_component_classes_pref`.
    ComponentClassEditKey {
        index: usize,
        key: String,
    },
    ComponentClassEditLabel {
        index: usize,
        label: String,
    },
    ComponentClassAdd,
    ComponentClassRemove {
        index: usize,
    },
    /// Reset the draft list to the seed defaults (`DEFAULT_COMPONENT_CLASSES`).
    ComponentClassResetDefaults,

    // ── Keyboard Shortcuts pane ──
    // All edits land on `UiState::preferences_keymap_editor` (a working
    // copy) so Cancel / Discard revert cleanly; Save commits the set
    // and recompiles the live keymap.
    /// Live search query for the shortcut table (case-insensitive filter
    /// on label / command id / trigger). Pure view state — the handler
    /// just stores it and never marks the draft dirty.
    KeymapSearchChanged(String),
    /// Switch the active profile shown in the editor.
    KeymapProfileSelected(String),
    /// Fork the active profile into a new editable custom profile.
    KeymapCreateCustomProfile,
    /// Delete the active custom profile draft (built-ins are protected).
    KeymapDeleteActiveProfile,
    /// Recover the profiles held in `keyboard_shortcuts.toml.bak` (#603).
    /// Reads the backup through the normal loader — it is not a
    /// file-level copy back, because the backup is by definition the file
    /// that failed to load.
    KeymapRestoreFromBackup,
    /// Delete `keyboard_shortcuts.toml.bak` (#603). Only ever from this
    /// explicit action; no save path removes it.
    KeymapDiscardBackup,
    /// Open a file picker to import a custom profile (`.toml`).
    KeymapImportProfile,
    /// Loaded profile source from the import pick dialog.
    KeymapProfileLoaded(String),
    /// The picked profile file could not be read; the reason.
    KeymapProfileUnreadable(String),
    /// Save the active profile to a `.toml` file.
    KeymapExportProfile,
    /// Outcome of the keymap-export write (#533). `Ok` carries the path
    /// actually written, `Err` the IO error rendered for the user;
    /// cancelling the save dialog emits nothing. Reports through
    /// `preferences_keymap_status`, where the serialization failure at
    /// the same call site already reported.
    KeymapExportFinished(Result<std::path::PathBuf, String>),
    /// Open the chord recorder for a specific binding, or for a command
    /// with no key yet (`unassigned`), whose OK adds a binding.
    KeymapRecorderOpen {
        slot: crate::keymap::BindingSlot,
        command: crate::keymap::AppCommandId,
        label: String,
        context: crate::keymap::ShortcutContext,
        trigger: String,
        unassigned: bool,
    },
    /// Dismiss the recorder without applying.
    KeymapRecorderCancel,
    /// Arm the recorder to capture keystrokes.
    KeymapRecorderStart,
    /// Stop capturing (keeps what was recorded).
    KeymapRecorderStop,
    /// Clear the captured strokes and keep recording.
    KeymapRecorderClear,
    /// Live modifier state while recording (drives the transient hint).
    KeymapRecorderModifiersChanged(crate::keymap::Modifiers),
    /// A captured keystroke, routed from the keyboard subscription.
    KeymapRecorderKeyPressed(crate::keymap::KeyStroke),
    /// Apply the recorded chord to the binding under edit. With nothing
    /// recorded (after Clear), the binding's key is taken away.
    KeymapRecorderApply,
}

// ─── Render inputs ────────────────────────────────────────────

/// Everything the Preferences dialog reads to render one frame.
///
/// The dialog is a four-hop chain — `view` / `view_body` -> `build_dialog`
/// -> `build_content` — and every hop needs very nearly the same inputs.
/// Passed positionally that was 23 parameters per hop, and several share a
/// type: `draft_theme` and `saved_theme` are both `ThemeId`;
/// `draft_grid_style` and `draft_symbol_grid_style` are both `GridStyle`.
/// A transposed pair would have compiled silently and shipped the wrong
/// control state. Naming the fields makes that a compile error instead.
///
/// Every field is `Copy`, so the whole struct is passed by value and each
/// hop costs a move rather than a fresh borrow.
#[derive(Clone, Copy)]
pub struct PrefsView<'a> {
    pub nav: PrefNav,
    pub draft_theme: ThemeId,
    pub saved_theme: ThemeId,
    pub draft_font: &'a str,
    pub draft_grid_style: GridStyle,
    pub draft_pcb_gpu_render: bool,
    pub draft_antialiasing: bool,
    pub draft_annotate_undo: crate::annotate_undo::AnnotateUndo,
    pub draft_move_keeps_wires: bool,
    pub draft_net_hover_enabled: bool,
    pub draft_edge_pan: crate::app::edge_pan::EdgePanPrefs,
    pub draft_symbol_body_fill: crate::symbol_fill_prefs::SymbolBodyFill,
    /// The fill colour field's preset palette and custom picker are open.
    pub body_fill_palette_open: bool,
    pub body_fill_picker_open: bool,
    pub draft_cross_out_not_fitted: bool,
    /// The PCB Defaults draft (#2868).
    pub draft_pcb_defaults: &'a crate::pcb_defaults_prefs::PcbDefaultsDraft,
    /// How much other copper dims while a board net is highlighted (#2514).
    pub draft_pcb_dim_others_percent: u8,
    pub draft_trackpad_pan: bool,
    pub draft_wheel_mode: crate::canvas::WheelMode,
    pub draft_layer_switcher: crate::layer_strip_prefs::LayerSwitcher,
    pub draft_selected_segment: signex_types::theme::SelectedSegmentLook,
    pub draft_remember_window_geometry: bool,
    pub draft_drag_direction_select: bool,
    pub draft_increment_step: i32,
    pub draft_paste_designators: crate::paste_prefs::PasteDesignators,
    pub draft_window_controls_placement: crate::chrome::controls::WindowControlsPlacement,
    /// Whether the drafted placement only applies after a restart (macOS).
    pub window_controls_restart_note: bool,
    pub draft_autosave_enabled: bool,
    pub draft_agent_connections_enabled: bool,
    /// The language the labels read in.
    pub i18n: &'a signex_i18n::I18n,
    /// The drafted interface language (mockup row 144a).
    pub draft_language: &'a signex_i18n::LanguagePref,
    /// The drafted language's catalog, built in `update`, for the Language
    /// page's preview.
    pub language_preview: &'a signex_i18n::I18n,
    /// Name of the language "Follow the system" resolves to, read in
    /// `update` when the dialog opens.
    pub system_language_name: &'a str,
    /// The pages with unsaved changes, for the nav's dots.
    pub dirty_pages: &'a [PrefNav],
    /// View state of the Appearance and Editing pages.
    pub pages_view: &'a PagesViewState,
    /// View state of the Keyboard shortcuts page.
    pub keymap_view: &'a KeymapViewState,
    /// View state of the Component Classes page.
    pub classes_view: &'a ClassesViewState,
    pub draft_symbol_grid_size_mm: f32,
    /// Text of the Fast grid 1 and Fast grid 2 fields (#1672).
    pub draft_fast_grids: &'a [String; 2],
    /// Why the last Save was refused over a fast grid; empty renders nothing.
    pub fast_grid_status: &'a str,
    pub draft_symbol_grid_style: GridStyle,
    pub draft_symbol_pin_selection: PinSelectionMode,
    pub draft_zoom_step_percent: u8,
    pub draft_ui_scale_percent: u16,
    pub dirty: bool,
    pub profiles: &'a crate::app::rule_profile_ui::ProfileManager,
    /// A profile edit is still on its way to its file.
    pub profiles_saving: bool,
    /// Why edits of `User Default` are not reaching its file, if they are not.
    pub machine_rules_issue: Option<crate::app::rule_profile_ui::MachineRulesIssue<'a>>,
    /// The profile new projects start from; `None` is automatic.
    pub default_profile: Option<&'a str>,
    pub distributor_settings: &'a crate::library::state::DistributorSettings,
    pub panel_tokens: &'a signex_types::theme::ThemeTokens,
    pub draft_component_classes: &'a [crate::fonts::ComponentClassEntry],
    pub keymap_editor: &'a crate::keymap::KeymapEditorModel,
    pub keymap_status: &'a str,
    /// Boot-time shortcut-file load failure (#595). `Some` renders a
    /// persistent banner under the pane header naming the file and the
    /// error; `None` renders nothing. Distinct from `keymap_status`,
    /// which is transient per-action feedback cleared on every open.
    pub keymap_load_error: Option<&'a str>,
    /// Path of `keyboard_shortcuts.toml.bak` when it exists (#603).
    /// `Some` renders the recover row with its Restore and Discard
    /// actions. Independent of `keymap_load_error` — the backup outlives
    /// the failure that produced it.
    pub keymap_backup: Option<&'a str>,
    /// `prefs.json` health failure (#602). `Some` renders a dialog-wide
    /// banner naming the file and the error; `None` renders nothing.
    ///
    /// Dialog-wide and not pane-local, unlike [`Self::keymap_load_error`]:
    /// `keyboard_shortcuts.toml` backs one pane, but `prefs.json` backs
    /// every pane in this dialog — theme, dock, ERC severities, filter
    /// presets, component classes — so a banner tucked inside one pane is
    /// invisible to a user sitting in another.
    pub prefs_load_error: Option<&'a str>,
    /// Result of the prefs-file recovery action. Transient per-action
    /// feedback shown in the same banner; empty renders nothing.
    pub prefs_status: &'a str,
    pub keymap_search: &'a str,
    /// Graphite nav search text (filters the nav rows only).
    pub nav_search: &'a str,
    pub keymap_recorder: Option<&'a crate::app::KeymapRecorderState>,
    /// The installed extensions, for their pages (#2495).
    pub extensions: &'a crate::extension::ExtensionHost,
    /// Their commands as the Keyboard shortcuts page names them.
    pub extension_commands: &'a crate::keymap::ExtensionCommandNames,
}

// ─── Dialog entry points ──────────────────────────────────────

/// Build the full-screen backdrop + centred dialog.
pub fn view<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let panel_tokens = v.panel_tokens;
    let dialog = build_dialog(v, true);
    let (card_w, card_h) = (shell::GRAPHITE_DLG_W, shell::GRAPHITE_DLG_H);

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                // In-app overlay: keep a centred, capped card (the detached
                // window path uses `view_body` directly and fills the window).
                container(dialog)
                    .width(Length::Fill.max(card_w))
                    .height(Length::Fill.max(card_h)),
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::app::view::dialogs::scrim_style(panel_tokens))
    .boxed()
}

// ─── Dialog shell ─────────────────────────────────────────────

/// Render the Preferences dialog body without the centred backdrop or
/// the in-dialog header — used by `view_detached_modal` so the dialog
/// fills its own OS window, whose native title bar supplies the title and
/// the close button. In-window callers go through `view()` which wraps the
/// full dialog (header included) in a tinted dismiss layer.
pub(crate) fn view_body<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    build_dialog(v, false)
}

fn build_dialog<'a>(v: PrefsView<'a>, with_header: bool) -> Element<'a, PrefMsg> {
    shell::graphite_dialog(v, with_header)
}

// ─── Right content ────────────────────────────────────────────

/// The open page in a scroller over the page background. Each page draws
/// its own heading, intro and sections on the `setting_row` grid.
fn build_content<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let PrefsView {
        nav,
        i18n,
        profiles,
        profiles_saving,
        machine_rules_issue,
        default_profile,
        panel_tokens,
        ..
    } = v;

    let inner = match nav {
        PrefNav::Appearance => content_appearance(v),
        PrefNav::Language => content_language(v),
        PrefNav::Keyboard => content_keyboard_shortcuts(v),
        PrefNav::General => system_general::content_general(v),
        PrefNav::Annotation => content_annotation(v),
        PrefNav::Editing => content_editing(v),
        PrefNav::SchematicDisplay => content_schematic_display(DisplayPage {
            draft: v.draft_symbol_body_fill,
            // The tint of the theme the dialog is showing.
            theme_tint: signex_types::theme::canvas_colors(v.draft_theme).body_tint,
            palette_open: v.body_fill_palette_open,
            picker_open: v.body_fill_picker_open,
            cross_out_not_fitted: v.draft_cross_out_not_fitted,
            tokens: panel_tokens,
            i18n,
        }),
        PrefNav::PcbDefaults => content_pcb_defaults(v.draft_pcb_defaults, panel_tokens, i18n),
        PrefNav::PcbDisplay => {
            pcb_display::content_pcb_display(v.draft_pcb_dim_others_percent, panel_tokens, i18n)
        }
        PrefNav::RuleProfiles => content_profiles(
            profiles,
            profiles_saving,
            machine_rules_issue,
            default_profile,
            panel_tokens,
            i18n,
        ),
        // The library subsystem owns the Distributor APIs form; the page
        // re-emits its `LibraryMessage::Settings(_)` as
        // `PrefMsg::LibrarySettings(_)`, and the handler re-dispatches it
        // through `Message::Library` so the settings stay on
        // `LibraryState.settings`.
        PrefNav::LibraryDistributors => content_library_distributors(v),
        PrefNav::ComponentClasses => content_component_classes(v),
        PrefNav::Extension(index) => {
            extension_page::content_extension(v.extensions, index, panel_tokens, i18n)
        }
    };

    container(
        scrollable(inner)
            .width(Length::Fill)
            .style(graphite_fields::scroll_style(panel_tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(0)
    .style(move |theme: &Theme| container::Style {
        background: Some(Background::Color(theme.palette().background.base.color)),
        ..container::Style::default()
    })
    .boxed()
}

/// The broken-file banner (#602, mockup row 143): full width under the header, in the
/// error tone. It names the file and the loader's reason, says what is
/// showing defaults, and offers Start a fresh preferences file and Open
/// folder. After the recovery action it carries that action's result.
///
/// `None` — a dialog without the banner — is the healthy case: nothing to
/// report and nothing to offer.
fn build_prefs_file_banner<'a>(
    load_error: Option<&'a str>,
    status: &'a str,
    i18n: &signex_i18n::I18n,
    t: &signex_types::theme::ThemeTokens,
) -> Option<Element<'a, PrefMsg>> {
    use crate::styles::{PillTone, Radii, chrome_px, pill_colors, ti, tokens};
    use setting_row::{StatusGlyph, semibold, small_button_style, status_glyph};
    use signex_i18n::{ids, t};
    if load_error.is_none() && status.is_empty() {
        return None;
    }
    let r = tokens::resolve(t);
    let ink = ti(t.text);
    let (glyph, tone) = if load_error.is_some() {
        (StatusGlyph::Warn, PillTone::Err)
    } else {
        (StatusGlyph::Info, PillTone::Info)
    };
    let (fill, tone_ink) = pill_colors(t, tone);

    let mut lines = column![].spacing(SP.xs).width(Length::Fill);
    if let Some(error) = load_error {
        // Name the real file, resolved through the same accessor the
        // probe and the recovery use, so the path the user is told about
        // is the path the app actually reads and writes.
        let path = crate::fonts::prefs_file_path().display().to_string();
        lines = lines.push(
            text(t!(i18n, ids::PREFERENCES_FILE_BROKEN))
                .size(TS.body)
                .font(semibold())
                .color(ink)
                .boxed(),
        );
        // The path alone in mono (mockup row 143, #3040). A path wider than
        // the banner breaks inside it rather than running past its edge.
        lines = lines.push(
            text(signex_i18n::UserText::new(path))
                .size(chrome_px(11.5 * 11.0 / 12.0))
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(ink)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .boxed(),
        );
        // $error is the loader's English reason (#2061), as plain text.
        lines = lines.push(
            text(t!(i18n, ids::PREFERENCES_FILE_REASON, "error" => error))
                .size(TS.caption)
                .color(ink)
                .width(Length::Fill)
                .boxed(),
        );
        // Scoped to what `prefs.json` actually backs: Keyboard Shortcuts
        // (`keyboard_shortcuts.toml`) and Distributor APIs
        // (`distributors.toml`) still save.
        lines = lines.push(
            text(t!(i18n, ids::PREFERENCES_FILE_DEFAULTS))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    if !status.is_empty() {
        // raw: the recovery result is set by its handler (#2061: handler-worded state text).
        lines = lines.push(text(raw(status)).size(TS.caption).color(ink).boxed());
    }
    // Offered only while the file is actually broken: once it loads, the
    // banner is a result line and there is nothing left to act on.
    if load_error.is_some() {
        let small = |label: signex_i18n::Localized, msg: PrefMsg| {
            button(
                container(text(label).size(TS.caption).color(ink))
                    .height(Length::Fill)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .padding([0.0, SP.m])
            .height(Length::Fixed(chrome_px(22.0 * 11.0 / 12.0)))
            .on_press(msg)
            .style(small_button_style(t))
            .boxed()
        };
        lines = lines.push(
            container(
                row![
                    small(
                        t!(i18n, ids::PREFERENCES_FILE_START_FRESH),
                        PrefMsg::ResetPrefsFile
                    ),
                    small(
                        t!(i18n, ids::PREFERENCES_FILE_OPEN_FOLDER),
                        PrefMsg::PrefsFolderOpenRequested
                    ),
                ]
                .spacing(SP.s + SP.xs),
            )
            .padding(iced::Padding {
                top: SP.s + SP.xs,
                ..iced::Padding::ZERO
            })
            .boxed(),
        );
    }

    let strip = container(
        row![
            container(status_glyph(glyph, tone_ink)).padding(iced::Padding {
                top: 1.0,
                ..iced::Padding::ZERO
            }),
            lines,
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Start),
    )
    .padding([SP.m, chrome_px(10.0 * 11.0 / 12.0)])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    Some(
        container(strip)
            .padding(iced::Padding {
                top: SP.m,
                right: SP.l,
                bottom: 0.0,
                left: SP.l,
            })
            .width(Length::Fill)
            .boxed(),
    )
}

#[cfg(test)]
mod nav_tests;
