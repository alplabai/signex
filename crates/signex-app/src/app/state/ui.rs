//! UI-surface state — theme, tool overlays, modal flags, dropdown and
//! command-palette state. Split from `app/state.rs` as pure code motion.

use crate::render_config::GridStyle;
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

use super::{
    AnnotateOrder, KeymapRecorderState, ModalId, MoveSelectionState, NetColorCustomState,
    ReorderPicker, WindowKind,
};

pub struct UiState {
    pub theme_id: ThemeId,
    pub unit: Unit,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    /// The document `cursor_x/y` was last reported over. Center View At
    /// Cursor uses the point only in that document, never in another tab or
    /// window, and does nothing before the first hover (#525).
    pub cursor_document: Option<std::path::PathBuf>,
    pub zoom: f64,
    pub grid_size_mm: f32,
    pub visible_grid_mm: f32,
    pub snap_hotspots: bool,
    pub ui_font_name: String,
    /// User-editable component-class registry. Loaded from
    /// `prefs.json::component_classes` at boot; falls back to
    /// [`crate::fonts::default_component_classes`] when absent.
    /// Surfaced in Preferences ▸ Component Classes (add / rename /
    /// delete) and consumed by the New Component modal's class
    /// dropdown so users can extend the list without recompiling.
    pub component_classes: Vec<crate::fonts::ComponentClassEntry>,
    /// Draft mirror used by Preferences for cancel/discard
    /// semantics — mutated as the user edits, copied back into
    /// `component_classes` + persisted on Save.
    pub preferences_draft_component_classes: Vec<crate::fonts::ComponentClassEntry>,
    /// Preferences ▸ Rule Profiles (#1608).
    pub profile_manager: crate::app::state::rule_profile_ui::ProfileManager,
    /// The boot task is still reading the `User Default` profile into
    /// `erc_severity_override` / `pin_matrix_overrides`. Edits of the machine's
    /// rules wait for it: writing the maps back before they hold the file's
    /// rules would replace them with defaults.
    pub machine_rules_loading: bool,
    /// Why the machine's rules are read-only this session: the migration of
    /// the old preferences into `User Default` was refused (the preferences
    /// file could not record it, or `User Default` could not be read), so an
    /// edit now could be overwritten by that migration on a later launch.
    pub machine_rules_locked: Option<String>,
    /// Why the machine's rules are not being saved (the `User Default` file
    /// could not be read or written), shown in the ERC dialog. `None` when
    /// they are.
    pub machine_rules_problem: Option<String>,
    /// The profile new projects start from; `None` means "User Default if
    /// the machine has one, else Signex Default" (#1608). Persisted in
    /// `prefs.json`.
    pub default_rule_profile: Option<String>,
    /// The open "Save as profile" dialog of the ERC dialog (#1608).
    pub save_profile_dialog: Option<crate::app::state::rule_profile_ui::SaveProfileDialog>,
    /// Profile file writes waiting their turn (#1608).
    pub profile_writes: crate::app::state::rule_profile_ui::ProfileWriteQueue,
    /// Loaded keyboard-shortcut profiles (bundled Altium / Classic
    /// built-ins plus any user profile in the OS config dir). Drives
    /// shortcut dispatch and the Help ▸ Keyboard Shortcuts reference.
    pub keymap_profiles: crate::keymap::ShortcutProfileSet,
    /// Active profile compiled to a fast key-sequence → command lookup.
    /// Rebuilt whenever the active profile changes.
    ///
    /// Behind an `Arc` so each view call can hand the menu bar a
    /// refcount bump instead of deep-cloning every binding (#1380).
    pub active_keymap: std::sync::Arc<crate::keymap::CompiledKeymap>,
    /// Pending multi-stroke chord buffer (e.g. Altium's `P W`). Lives in
    /// `UiState` rather than a process-global static so chords resolve in
    /// `update` under `&mut self` — sound across multiple windows. The
    /// resolver clears it on a match, a definite miss, or via the
    /// single-stroke restart retry.
    pub keymap_pending_sequence: Vec<crate::keymap::KeyStroke>,
    /// Which window the pending chord began in (#559).
    ///
    /// The buffer above is app-wide, so without this a sequence could
    /// start in one window, finish in another, and resolve against the
    /// contexts of a third. `resolve_keymap_stroke` drops the prefix
    /// whenever the next stroke arrives from a different target.
    ///
    /// `pub(crate)` unlike its neighbours: `InputTarget` is crate-private,
    /// and a `pub` field carrying it would leak a private type out of the
    /// crate's interface. Nothing outside needs to read it.
    pub(crate) keymap_pending_target: Option<crate::app::dispatch::input::InputTarget>,
    /// Set while the pending chord is a complete shortcut waiting out
    /// `CHORD_TIMEOUT` for a longer one (#1378). Gates the timer and the
    /// pointer-cancel subscriptions, so they cost nothing when idle.
    pub(crate) keymap_chord_wait: Option<crate::app::dispatch::chord::ChordWait>,
    /// Source of `ChordWait::generation`.
    pub(crate) keymap_chord_generation: u64,
    pub canvas_font_name: String,
    pub canvas_font_size: f32,
    pub canvas_font_bold: bool,
    pub canvas_font_italic: bool,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub window_size: (f32, f32),
    /// OS-reported scale factor for the main window (1.0 at 100 %,
    /// 1.25 at 125 %, 2.0 at 200 %, etc.). Populated on
    /// `MainWindowOpened` and refreshed on every main-window resize,
    /// which also fires when Windows moves the window to a different
    /// monitor. Used to pick the 1×/2×/3× wordmark PNG so the brand
    /// lockup stays 1:1 with device pixels.
    pub main_window_scale: f32,
    pub panel_list_open: bool,
    pub preferences_open: bool,
    /// Help ▸ Keyboard Shortcuts modal — reference table over the
    /// active keyboard-shortcut profile. Toggled from the Help menu
    /// and from F1.
    pub keyboard_shortcuts_open: bool,
    /// Pre-grouped rows for the shortcuts modal, built by
    /// [`Self::refresh_shortcut_sections`] when the modal opens and again
    /// whenever `keymap_profiles` is replaced, so `view` does not rebuild
    /// the editor model every frame (#1380). `None` means "not built":
    /// the view then computes the sections inline, so a stale cache is
    /// impossible by construction.
    pub shortcut_sections: Option<Vec<crate::keyboard_shortcuts_modal::ShortcutSection>>,
    /// Help ▸ About Signex dialog.
    pub about_open: bool,
    /// File ▸ Open Recent — loaded from `prefs.json` at boot, updated on
    /// every successful project open.
    pub recent_projects: crate::recent_projects::RecentProjects,
    /// First-run tour overlay — a single dismissible card shown only
    /// before the user has dismissed it once. Initial value is read
    /// from prefs in `bootstrap`; transitions to `false` on the first
    /// dismiss gesture and the prefs file flips to "dismissed" so the
    /// card never reappears. Closes UX §4.3.
    pub first_run_tour_open: bool,
    pub find_replace: crate::find_replace::FindReplaceState,
    pub preferences_nav: crate::preferences::PrefNav,
    pub preferences_draft_theme: ThemeId,
    pub preferences_draft_font: String,
    /// Status / error line shown under Appearance ▸ Import/Export Theme.
    /// The export writes a file the user picked in a save dialog, so a
    /// failed write has to reach the user rather than leaving them
    /// believing the theme was saved (#533). Sibling of
    /// [`Self::preferences_keymap_status`]; empty renders nothing. Pure
    /// feedback — never part of the dirty predicate.
    pub preferences_theme_status: String,
    pub grid_style: GridStyle,
    pub preferences_draft_grid_style: GridStyle,
    /// Saved "Annotate undo" preference: one undo step per annotate run for
    /// the whole project, or one per sheet. Read when an annotate run starts.
    pub annotate_undo: crate::annotate_undo::AnnotateUndo,
    /// Draft mirror of [`Self::annotate_undo`] for the Preferences dialog.
    pub preferences_draft_annotate_undo: crate::annotate_undo::AnnotateUndo,
    /// Saved "Keep wires connected when moving" preference: a schematic move
    /// (drag, Move, Move By, align) drags the wires attached to the moved
    /// symbols' pins along with them. Default on.
    pub move_keeps_wires: bool,
    /// Draft mirror of [`Self::move_keeps_wires`] for the Preferences dialog.
    pub preferences_draft_move_keeps_wires: bool,
    /// Show the net name, class and pins after a short dwell over a wire, pin
    /// or label (#1270). On by default.
    pub net_hover_enabled: bool,
    /// Draft mirror of [`Self::net_hover_enabled`] for the Preferences dialog.
    pub preferences_draft_net_hover_enabled: bool,
    /// Saved "Select by drag direction" preference (#1257): a box drag to
    /// the right selects what it fully contains, to the left what it touches.
    /// Off falls back to the global `selection_mode`. Default on.
    pub drag_direction_select: bool,
    /// Draft mirror of [`Self::drag_direction_select`] for Preferences.
    pub preferences_draft_drag_direction_select: bool,
    /// Saved "Increment step" (#1253): how far a repeated label placement and
    /// Paste Array move the trailing number of a text. Never zero.
    pub increment_step: i32,
    /// Draft mirror of [`Self::increment_step`] for Preferences.
    pub preferences_draft_increment_step: i32,
    /// Saved "Designators on paste" (#1263).
    pub paste_designators: crate::paste_prefs::PasteDesignators,
    /// Draft mirror of [`Self::paste_designators`] for Preferences.
    pub preferences_draft_paste_designators: crate::paste_prefs::PasteDesignators,
    /// Saved "Window controls" placement (where min/max/close go).
    pub window_controls_placement: crate::chrome::controls::WindowControlsPlacement,
    /// Draft mirror of [`Self::window_controls_placement`] for Preferences.
    pub preferences_draft_window_controls_placement:
        crate::chrome::controls::WindowControlsPlacement,
    /// The placement the main window was created with. On macOS it decides
    /// native vs drawn buttons and cannot change until a restart.
    pub window_controls_placement_at_boot: crate::chrome::controls::WindowControlsPlacement,
    /// Button arrangement the Linux desktop asked for; `None` until the
    /// startup probe lands or when the desktop gave none.
    pub desktop_button_layout: Option<crate::chrome::desktop::DesktopButtonLayout>,
    /// Last known mode of the main window (fullscreen hides the macOS inset).
    pub main_window_mode: iced::window::Mode,
    /// The window that has keyboard focus, `None` while none of ours does.
    /// The system menu bar acts on it (docs/MENU_BAR.md, "Native macOS menu").
    pub focused_window: Option<iced::window::Id>,
    /// The main window has opened (`MainWindowOpened`): dialogs may open
    /// their own windows from here on.
    pub main_window_opened: bool,
    /// Text inputs were unfocused for the confirm dialog that now locks the
    /// main window (once per lock).
    pub lock_unfocused: bool,
    /// The size each content-sized dialog window was last given.
    pub modal_window_sizes: std::collections::HashMap<iced::window::Id, (f32, f32)>,
    /// The quit confirm was refreshed because the unsaved list changed after it
    /// opened (Discard All discarded nothing); the dialog says so.
    pub quit_confirm_refreshed: bool,
    /// How often an Edit Component Details window vanished unanswered, per
    /// browser; after two it stays in the window so typed text is not lost.
    pub edit_row_window_vanished: std::collections::HashMap<std::path::PathBuf, u8>,
    /// How often a recovery prompt's window vanished without being answered;
    /// after two it stays in the main window instead of reopening.
    pub recovery_window_vanished: u8,
    /// Which library browser opened each per-browser dialog window (edit row,
    /// delete confirm): the window reads and answers for THAT browser.
    pub library_modal_owners: std::collections::HashMap<iced::window::Id, std::path::PathBuf>,
    /// Logical size of the monitor holding the main window, when known.
    pub main_window_monitor_size: Option<iced::Size>,
    /// Opt-in autosave of recovery copies (#104). Off by default.
    pub autosave_enabled: bool,
    /// Draft mirror of [`Self::autosave_enabled`] for the Preferences dialog.
    pub preferences_draft_autosave_enabled: bool,
    /// Saved PCB GPU-render toggle (experimental). The *effective* live value
    /// used by rendering is `PcbCanvas::gpu_render` (updated immediately on
    /// draft change for live preview, like `render_config` globals for
    /// grid/theme); this field is the persisted value used only for the
    /// dirty comparison + on-disk persistence. See [`Self::preferences_draft_differs`].
    pub pcb_gpu_render: bool,
    /// Draft mirror of [`Self::pcb_gpu_render`] for Preferences cancel/discard.
    pub preferences_draft_pcb_gpu_render: bool,
    /// Default symbol-editor grid size (mm) — used when a library is
    /// first opened. Changed from Preferences ▸ Appearance ▸ Symbol Editor.
    ///
    /// #629 — these three had no committed field at all until this one and
    /// its two siblings were added: the `preferences_draft_*` mirror WAS
    /// the saved value, seeded from disk at boot and written back on every
    /// picker move. That left `revert_preferences_drafts` nothing to
    /// restore from, so Cancel silently kept the change while the four
    /// settings above it reverted.
    pub symbol_grid_size_mm: f32,
    /// Draft mirror of [`Self::symbol_grid_size_mm`].
    pub preferences_draft_symbol_grid_size_mm: f32,
    /// Symbol-editor grid display style (Dots / Crosses / Lines).
    pub symbol_grid_style: crate::render_config::GridStyle,
    /// Draft mirror of [`Self::symbol_grid_style`]. The only one of the
    /// three with a live preview: the draft is pushed into
    /// `PanelContext::symbol_grid_style` on change, so Cancel has to push
    /// the committed value back there as well as restoring this field.
    pub preferences_draft_symbol_grid_style: crate::render_config::GridStyle,
    /// Symbol-editor pin-selection mode (pin body only / body + labels).
    pub symbol_pin_selection: crate::render_config::PinSelectionMode,
    /// Wheel-zoom speed in percent per notch (5..=50, default 10),
    /// shared by the schematic, PCB, symbol and footprint canvases.
    pub zoom_step_percent: u8,
    /// Interface scale (0.8..=2.0) laid on top of the OS display scale
    /// through the daemon's `scale_factor` hook (#1228).
    pub ui_scale: f32,
    /// Draft mirror of [`Self::ui_scale`], as a percent in 10 % steps.
    /// Applied on Save, like the other Appearance drafts.
    pub preferences_draft_ui_scale_percent: u16,
    /// Draft mirror of [`Self::zoom_step_percent`]. Also the *effective*
    /// value the canvases zoom with (see [`Self::effective_zoom_step`]).
    pub preferences_draft_zoom_step_percent: u8,
    /// Draft mirror of [`Self::symbol_pin_selection`].
    pub preferences_draft_symbol_pin_selection: crate::render_config::PinSelectionMode,
    /// Keyboard Shortcuts pane — editable working copy of the shortcut
    /// profile set. Seeded from [`Self::keymap_profiles`] each time the
    /// Preferences window opens (and on Discard), mutated through the
    /// [`crate::keymap::KeymapEditorModel`] API as the user edits, and
    /// committed back to `keymap_profiles` + recompiled into
    /// `active_keymap` on Save so live dispatch picks up the change.
    pub preferences_keymap_editor: crate::keymap::KeymapEditorModel,
    /// Status / error line shown under the Keyboard Shortcuts pane —
    /// carries parse failures, save failures and profile actions so no
    /// error is swallowed silently. Empty renders the default conflict
    /// summary instead.
    pub preferences_keymap_status: String,
    /// Boot-time keyboard-shortcut load failure, carried for the life of
    /// the session (#595). `Some` means a shortcuts file EXISTS on disk
    /// and could not be honoured — unreadable, unparseable, or naming an
    /// `active_profile` that no longer resolves — so the running set is
    /// the bundled built-in fallback rather than the user's own profiles.
    ///
    /// That is emphatically NOT the same as the user having no custom
    /// profiles: [`crate::keymap::load_profile_set_at`] returns `Ok` for a
    /// missing file, so a fresh install leaves this `None`. Treating the
    /// two alike is what let a Preferences Apply serialise the
    /// custom-profile-free fallback over the user's real file.
    ///
    /// Unlike [`Self::preferences_keymap_status`] this is NOT cleared when
    /// the Preferences dialog opens — it is a persistent banner, not
    /// transient feedback. Only a successful save clears it, because only
    /// then is the file on disk one this process wrote.
    pub keymap_load_error: Option<String>,
    /// Path of `keyboard_shortcuts.toml.bak` when that file is on disk
    /// (#603).
    ///
    /// Distinct from [`Self::keymap_load_error`] on purpose: the backup
    /// outlives the failure that produced it. Once a successful save
    /// clears the load error the banner goes away, and before this field
    /// existed that was the last time the app ever mentioned the backup
    /// — the user's profiles sat in a file nothing could see or act on.
    /// `Some` here means the Keyboard pane offers Restore and Discard,
    /// error or no error.
    ///
    /// Refreshed on every Preferences open and after either action, the
    /// same way `prefs_load_error` is re-probed (#602). It is a cached
    /// answer to "does that file exist", not a claim about its contents.
    pub keymap_backup: Option<String>,
    /// The last known health of `prefs.json` (#602) — NOT a boot-time
    /// snapshot, unlike [`Self::keymap_load_error`] right above. It is
    /// set at boot and refreshed on every Preferences open and at the end
    /// of every Preferences Save, so the boot value only ever reaches a
    /// user through the boot `tracing::error!`:
    /// `seed_preferences_drafts_from_live` re-probes and overwrites it
    /// before the dialog first renders. The boot assignment still has to
    /// be right, because the field must not lie in the window before that
    /// first open.
    ///
    /// `Some` means the file EXISTS and could not be loaded — unreadable,
    /// unparseable, or with a non-object root — so every preference
    /// reader is silently handing back its default and
    /// [`crate::fonts::check_prefs_file`]'s sibling `update_prefs_json`
    /// is refusing every write to keep the bytes repairable. The UI looks
    /// factory-reset and nothing the user changes sticks, which is
    /// exactly the pair that has to be said out loud rather than left in
    /// the Messages panel.
    ///
    /// As with [`Self::keymap_load_error`], absence is NOT a failure:
    /// a missing (or all-whitespace) file is a fresh install, which the
    /// writer starts from happily, so this stays `None` there.
    ///
    /// Re-probing is what [`Self::keymap_load_error`] cannot do: that
    /// flag can only be cleared by a successful save, because only then
    /// is the file on disk one this process wrote. Here the check is a
    /// single file read with no state behind it, so a user who repairs
    /// `prefs.json` by hand sees the banner go on the next open, and one
    /// who breaks it mid-session sees it appear on the next Save.
    pub prefs_load_error: Option<String>,
    /// Result line for the Preferences prefs-file recovery action (#602).
    /// Transient per-action feedback — never part of the dirty
    /// predicate — and cleared on every dialog open, exactly like
    /// [`Self::preferences_theme_status`]. Empty renders nothing.
    pub preferences_prefs_status: String,
    /// Live search query for the Keyboard Shortcuts pane. Filters the
    /// grouped shortcut table case-insensitively by label / command id /
    /// trigger. Reset to empty each time Preferences opens.
    pub preferences_keymap_search: String,
    /// Chord recorder overlay. `Some` while the user is capturing a new
    /// keystroke for a binding; the keyboard subscription routes raw
    /// strokes to the recorder instead of the live keymap resolver
    /// while it is open.
    pub preferences_keymap_recorder: Option<KeymapRecorderState>,
    pub preferences_dirty: bool,
    /// Sticky companion to [`Self::preferences_dirty`] for imperative edits
    /// the draft comparator cannot observe — currently only Import Theme,
    /// which swaps the live [`Self::custom_theme`] in place (there is no
    /// draft copy to diff against). Once set, recomputing the dirty flag
    /// from [`Self::preferences_has_unsaved_changes`] keeps it `true` until
    /// Save or a draft reseed (open / revert) clears it, so a later
    /// appearance-draft change can't clobber the pending import back to
    /// "clean" and let the dialog close without a prompt.
    pub preferences_dirty_sticky: bool,
    pub custom_theme: Option<signex_types::theme::CustomThemeFile>,
    /// Rename-sheet modal state. Opened from the Projects-panel tree
    /// context menu; `None` when the modal is closed.
    pub rename_dialog: Option<crate::app::RenameDialogState>,
    /// Remove-from-project modal state (Delete / Exclude / Cancel).
    pub remove_dialog: Option<crate::app::RemoveDialogState>,
    /// "Close Project — Unsaved Edits" confirmation modal. `Some`
    /// while the user is being asked to save / discard / cancel a
    /// close request that touches a dirty document. Cleared on any
    /// of the three button choices.
    pub project_close_confirm: Option<crate::app::ProjectCloseConfirmState>,
    /// "Create the missing sheet?" confirm; `Some` while it is up.
    pub create_child_sheet_confirm: Option<crate::app::contracts::CreateChildSheetConfirmState>,
    /// A Create Sheet request from the Projects tree waiting for its parent
    /// sheet to open: `(parent sheet, missing child file)` (#1618).
    pub pending_create_sheet: Option<(std::path::PathBuf, std::path::PathBuf)>,
    /// "Exit Signex — Unsaved Edits" confirmation modal. `Some` while
    /// the user is being asked to save / discard / cancel an app-exit
    /// request that touches a dirty document. Cleared on any of the
    /// three button choices.
    pub app_quit_confirm: Option<crate::app::AppQuitConfirmState>,
    /// v0.11 Enable Version Control modal — opened from the
    /// project root context menu when the project dir has no
    /// `.git/` yet.
    pub enable_version_control: Option<crate::app::EnableVersionControlState>,
    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a
    /// footprint editor). `Some` while the dialog is visible;
    /// carries the in-flight Step X / Step Y / link buffers so
    /// keystrokes don't immediately mutate the active editor.
    pub grid_properties: Option<crate::app::GridPropertiesState>,
    /// The Find Similar Objects dialog, while open (#1257).
    pub find_similar: Option<crate::find_similar::FindSimilarState>,
    /// Token of the newest Find Similar project search. Lives outside the
    /// dialog state so a reopened dialog never reuses an old token.
    pub find_similar_generation: u64,
    /// The open Paste Array dialog (#1264).
    pub paste_array: Option<crate::paste_array::PasteArrayState>,
    /// How many Paste Array dialogs were opened: tags each one so a numbers
    /// read answers only the dialog that asked for it.
    pub paste_array_opens: u64,
    /// v0.18.14.1 — Custom Selection Filter modal draft. `Some`
    /// while the dialog is visible; the eight-bool draft mirror
    /// commits on Apply, discards on Cancel.
    pub selection_filter_custom: Option<crate::app::SelectionFilterCustomState>,
    /// ERC results for the currently-visible sheet. Driven by the
    /// per-sheet cache below — switching tabs repoints this at the
    /// cached violations for that sheet, so markers and the Messages
    /// panel always match what's on the canvas.
    pub erc_violations: Vec<signex_erc::Violation>,
    /// Per-sheet ERC violation cache, keyed by the sheet's on-disk
    /// file path. Run ERC populates this for every sheet in the
    /// project; tab switches point `erc_violations` at the matching
    /// entry without rerunning the analysis.
    pub erc_violations_by_path:
        std::collections::HashMap<std::path::PathBuf, Vec<signex_erc::Violation>>,
    /// Findings each waived (rule id, net) pair hid in the newest run
    /// (#1581); 0 hidden means the waiver matched nothing (e.g. renamed net).
    pub erc_waived_hidden: std::collections::BTreeMap<(String, String), usize>,
    /// Findings a per-finding waiver hid in the newest run (#1285).
    pub erc_waived_findings: Vec<crate::app::handlers::erc::WaivedFinding>,
    /// (rule id, objects) of waivers that lapsed in the newest run.
    pub erc_lapsed_waivers: std::collections::BTreeSet<(String, Vec<String>)>,
    /// The waiver being written in the ERC panel, if any (#1285).
    pub erc_waive_draft: Option<crate::panels::ErcWaiveDraft>,
    /// Global cursor into the flattened ERC diagnostics list spanning all
    /// sheets in `erc_violations_by_path`. Used by next/prev navigation.
    pub erc_focus_global_index: Option<usize>,
    /// Token of the newest ERC run; a finished run applies only if it still
    /// matches (#111).
    pub erc_run_generation: u64,
    /// An ERC run is in flight; the ERC panel shows "Running…".
    pub erc_running: bool,
    /// The rule set the newest ERC run used, for the report header (#1085).
    pub erc_run_rule_set: Option<signex_output::ErcReportRuleSet>,
    /// An "Update from profile" waiting for the user's confirmation, with the
    /// diff it would apply (#1085). Nothing is changed until confirmed.
    pub pending_profile_update: Option<crate::app::state::PendingProfileUpdate>,
    /// The machine's rule set, as the `User Default` profile holds it (#1608):
    /// per-rule severity overrides, used by a project that stores no rules of
    /// its own. If empty, each rule's default is used. Loaded from the profile
    /// at launch and written back to it on every edit.
    pub erc_severity_override:
        std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    /// Net-color overrides keyed by net-label text. Drives the Active-Bar
    /// net-colour menu state and the net-color modal; the canvas paints
    /// from the per-wire `wire_color_overrides` map below.
    pub net_colors: std::collections::HashMap<String, signex_types::theme::Color>,
    /// AutoFocus mode — when true, non-selected items dim on the canvas.
    pub auto_focus: bool,
    /// Annotate dialog open flag. When true, the Annotate-Schematics modal
    /// covers the canvas with its preview + confirm-apply UI.
    pub annotate_dialog_open: bool,
    /// Annotate dialog: order-of-processing choice. Controls the iteration
    /// order used to assign sequential numbers.
    pub annotate_order: AnnotateOrder,
    /// ERC dialog open flag — opens the full severity-matrix + pin-matrix UI.
    pub erc_dialog_open: bool,
    /// Reset-and-renumber confirmation modal. When true, the Design →
    /// Reset menu item shows a confirm before discarding every number.
    pub annotate_reset_confirm: bool,
    /// Per-modal offset in window pixels from the centered position.
    /// Updated when the user drags the title bar. Persists until the app
    /// closes so reopening a dialog lands where it was last placed.
    pub modal_offsets: std::collections::HashMap<ModalId, (f32, f32)>,
    /// Active modal drag: which modal is being dragged + the last mouse
    /// position so the delta can be computed from the next DragMove event.
    pub modal_dragging: Option<(ModalId, f32, f32)>,
    /// Active tab drag: which document tab is being dragged + the last
    /// mouse position. Used by auto-detach — when the cursor crosses the
    /// main window edge the tab undocks into its own OS window.
    pub tab_dragging: Option<(usize, f32, f32)>,
    /// Move-Selection dialog state (Altium's numeric ΔX / ΔY move).
    pub move_selection: MoveSelectionState,
    /// F5 Net Color palette state — open flag and transient edit buffer.
    pub net_color_palette_open: bool,
    /// Parameter Manager dialog state.
    pub parameter_manager_open: bool,
    /// Active "pick a reference item" mode for z-order operations
    /// (BringToFrontOf / SendToBackOf). When Some, the next canvas click
    /// resolves the reference uuid and submits the Reorder command.
    pub reorder_picker: Option<ReorderPicker>,
    /// Armed click-click Move / Move Selection (#271). The canvas mirrors it
    /// in `CanvasSlot::move_pick`; the picking press consumes it.
    pub move_pick: Option<crate::canvas::MovePick>,
    /// Pin-connection matrix overrides — sparse map keyed by (row, col)
    /// pin-type index. Any entry present replaces the default severity
    /// for that pair; missing entries fall back to the hard-coded
    /// baseline in `pin_matrix_view`. Persisted alongside the ERC
    /// severity map.
    pub pin_matrix_overrides: std::collections::HashMap<(u8, u8), signex_erc::Severity>,
    /// Symbols whose designator the user locked against reannotation.
    /// Exposed as per-row checkboxes in the Annotate dialog; the engine
    /// skips these uuids in `annotate_with_seed_and_locks`.
    pub annotate_locked: std::collections::HashSet<uuid::Uuid>,
    /// Cached Annotate-dialog preview; loaded off-thread and re-derived in
    /// `update`, never in `view` (#111).
    pub annotate_preview: crate::app::handlers::erc::AnnotatePreviewCache,
    /// The Annotate or Reset Duplicate Designators run in flight, if any (#859).
    pub annotate_runs: crate::app::handlers::erc::AnnotateRuns,
    /// Token of the newest Re-prefix to standard library lookup (#1315); a
    /// result carrying an older one is dropped.
    pub reprefix_generation: u64,
    /// The Re-prefix to standard confirm: the planned changes, awaiting a
    /// Confirm. `None` when no confirm is open.
    pub reprefix_confirm: Option<crate::app::handlers::erc::ReprefixConfirm>,
    /// A confirm was dropped while its detached window was open: the next
    /// `finish_update` closes that window.
    pub reprefix_window_close_pending: bool,
    /// Altium-style rubber-band selection mode. Drives how the box
    /// drag classifies hits (Inside / Outside / TouchingLine).
    pub selection_mode: crate::schematic_runtime::hit_test::SelectionMode,
    /// Net-color override armed from the Active Bar palette. When Some,
    /// the cursor turns into a paint-bucket over the canvas and the
    /// next click on a wire floods that color across every connected
    /// wire. Cleared after the click applies, or by Escape. Colors are
    /// render-time only — they do NOT write back to the .standard_sch.
    pub pending_net_color: Option<signex_types::theme::Color>,
    /// Per-wire color overrides keyed by wire uuid. Populated by the
    /// net-color click; consulted when drawing wires. Not serialised.
    pub wire_color_overrides: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    /// Net-class style of every wire and junction, resolved from the class
    /// table and the sheets' netlists after each update (`finish_update`).
    /// The canvas reads it instead of the project. Not serialised.
    pub net_class_styles: crate::app::state::net_class_styles::NetClassStyleCache,
    /// Altium-style lasso in flight. `Some(points)` means the user
    /// started a lasso — each canvas click appends a vertex; a
    /// double-click or a click on the first vertex closes the polygon
    /// and commits the selection. Escape or right-click cancels.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// App-level undo stack for net-color floods. Each step holds the
    /// overrides of the sheet's own wires captured before an action plus the
    /// engine's `undo_position` at that moment, so Ctrl+Z can interleave
    /// colour steps and engine edits in true chronological order. Separate
    /// from the engine's history because net colours are render-only and
    /// shouldn't mix with document mutations.
    pub net_color_undo: std::collections::HashMap<std::path::PathBuf, Vec<NetColorStep>>,
    /// Keyed by document path: each document has its own timeline.
    /// Redo mirror of [`Self::net_color_undo`]; cleared by any new colour
    /// action or engine edit.
    /// Per sheet, bumped by every net-colour change. Colours are not part of
    /// the engine revision, so a save records this too and may mark the sheet
    /// clean only if it is unchanged when the write lands.
    pub net_color_revision: std::collections::HashMap<std::path::PathBuf, u64>,
    pub net_color_redo: std::collections::HashMap<std::path::PathBuf, Vec<NetColorStep>>,
    /// Custom net-color picker state. When `show = true`, a floating
    /// iced_aw ColorPicker appears anchored to the Active Bar button;
    /// `draft` is the user's pending pick — committed on OK.
    pub net_color_custom: NetColorCustomState,
    /// Id of the primary app window — set once `iced::window::open` for
    /// the main window resolves. Every `view(id)` call checks this to
    /// decide whether it's rendering the main shell or a secondary
    /// (detached modal / undocked tab) window.
    pub main_window_id: Option<iced::window::Id>,
    /// Every non-main window Signex owns, keyed by its iced id. Lets
    /// `view(id)` dispatch between the main shell, detached modals, and
    /// (later) undocked tabs. `SecondaryWindowClosed` removes entries so
    /// the detached content reattaches to the main window.
    pub windows: std::collections::HashMap<iced::window::Id, WindowKind>,
    /// Calculator state, owned here so the modal view stays a pure
    /// function of the model (ADR-0001 A1).
    pub passive_calculator: signex_widgets::passive_calculator::CalculatorControl,
    /// Whether the Tools > Passive Network Calculator modal is open.
    pub passive_calculator_open: bool,
    /// Command palette state — query / dropdown open flag / selected
    /// row. The chrome-strip search bar is the always-rendered input;
    /// `open` gates the dropdown overlay only.
    pub command_palette: super::super::command_palette::CommandPaletteState,
}

impl UiState {
    /// The one answer to "is the main window fullscreen" the strip and the
    /// search-bar geometry share.
    pub fn main_window_is_fullscreen(&self) -> bool {
        crate::chrome::controls::is_effectively_fullscreen(
            self.main_window_mode,
            self.window_size,
            self.main_window_monitor_size,
        )
    }

    /// Replace the loaded profile set and rebuild the shortcuts-modal
    /// cache from it, so the cache can never outlive the profiles it was
    /// built from. (Callers recompile `active_keymap` themselves.)
    pub fn set_keymap_profiles(&mut self, profiles: crate::keymap::ShortcutProfileSet) {
        self.keymap_profiles = profiles;
        self.refresh_shortcut_sections();
    }

    /// Rebuild the shortcuts-modal cache: built while the modal is open,
    /// dropped otherwise (the next open rebuilds it).
    pub fn refresh_shortcut_sections(&mut self) {
        self.shortcut_sections = self
            .keyboard_shortcuts_open
            .then(|| crate::keyboard_shortcuts_modal::build_sections(&self.keymap_profiles));
    }

    /// Per-wheel-notch zoom factor every canvas uses. Reads the DRAFT, like
    /// `grid_style`: it carries the live preview while Preferences is open
    /// and equals the saved value otherwise (Discard re-seeds the draft).
    pub fn effective_zoom_step(&self) -> f32 {
        crate::canvas::Camera::zoom_step_factor(self.preferences_draft_zoom_step_percent)
    }

    /// True when any Preferences draft differs from its saved live value.
    /// Single source of truth so every live-preview `PrefMsg::Draft*` handler
    /// stays consistent as new drafts are added (the old per-handler inline
    /// chains had drifted to different term sets). Covers ALL draft state —
    /// the 10 appearance drafts, the component-class table and the keymap
    /// working copy — so an appearance recompute can't report "clean" while
    /// a pending rebind or class edit would be lost on close.
    ///
    /// The three Symbol Editor terms joined in #629. Until then the claim
    /// above was false: they wrote themselves to `prefs.json` on change and
    /// were deliberately left out, so the predicate was consistent with
    /// their behaviour but not with the footer the user was looking at.
    pub fn preferences_draft_differs(&self) -> bool {
        self.preferences_draft_theme != self.theme_id
            || self.preferences_draft_font != self.ui_font_name
            || self.preferences_draft_grid_style != self.grid_style
            || self.preferences_draft_pcb_gpu_render != self.pcb_gpu_render
            || self.preferences_draft_annotate_undo != self.annotate_undo
            || self.preferences_draft_move_keeps_wires != self.move_keeps_wires
            || self.preferences_draft_net_hover_enabled != self.net_hover_enabled
            || self.preferences_draft_drag_direction_select != self.drag_direction_select
            || self.preferences_draft_increment_step != self.increment_step
            || self.preferences_draft_paste_designators != self.paste_designators
            || self.preferences_draft_window_controls_placement != self.window_controls_placement
            || self.preferences_draft_autosave_enabled != self.autosave_enabled
            || self.preferences_draft_symbol_grid_style != self.symbol_grid_style
            // Exact `!=` on the f32 is deliberate: the picker assigns from
            // `canvas::grid::GRID_SIZES_MM`, so draft and committed are the
            // same constant unless the user actually moved it. A tolerance
            // here would let a real change read as clean.
            || self.preferences_draft_symbol_grid_size_mm != self.symbol_grid_size_mm
            || self.preferences_draft_symbol_pin_selection != self.symbol_pin_selection
            || self.preferences_draft_zoom_step_percent != self.zoom_step_percent
            || self.preferences_draft_ui_scale_percent
                != crate::ui_scale::scale_to_percent(self.ui_scale)
            || self.preferences_draft_component_classes != self.component_classes
            || self
                .preferences_keymap_editor
                .differs_from(&self.keymap_profiles)
    }

    /// True when closing Preferences without Save would lose work — drives
    /// the Save/Discard footer + every dirty-close guard. The draft
    /// comparator plus the sticky flag for imperative edits it can't see
    /// (see [`Self::preferences_dirty_sticky`]).
    pub fn preferences_has_unsaved_changes(&self) -> bool {
        self.preferences_dirty_sticky || self.preferences_draft_differs()
    }
}

/// One net-colour undo/redo step: the owning sheet's overrides to restore, stamped
/// with the engine edit-timeline position it was recorded at.
#[derive(Debug, Clone)]
pub struct NetColorStep {
    pub colors: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    pub at: usize,
}
