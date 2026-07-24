//! UI-surface state — theme, tool overlays, modal flags, dropdown and
//! command-palette state. Split from `app/state.rs` as pure code motion.

use crate::render_config::{GridStyle, LabelStyle, MultisheetStyle, PowerPortStyle};
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
    /// Loaded keyboard-shortcut profiles (bundled Altium / Classic
    /// built-ins plus any user profile in the OS config dir). Drives
    /// shortcut dispatch and the Help ▸ Keyboard Shortcuts reference.
    pub keymap_profiles: crate::keymap::ShortcutProfileSet,
    /// Active profile compiled to a fast key-sequence → command lookup.
    /// Rebuilt whenever the active profile changes.
    pub active_keymap: crate::keymap::CompiledKeymap,
    /// Pending multi-stroke chord buffer (e.g. Altium's `P W`). Lives in
    /// `UiState` rather than a process-global static so chords resolve in
    /// `update` under `&mut self` — sound across multiple windows. The
    /// resolver clears it on a match, a definite miss, or via the
    /// single-stroke restart retry.
    pub keymap_pending_sequence: Vec<crate::keymap::KeyStroke>,
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
    pub power_port_style: PowerPortStyle,
    pub preferences_draft_power_port_style: PowerPortStyle,
    pub label_style: LabelStyle,
    pub preferences_draft_label_style: LabelStyle,
    pub multisheet_style: MultisheetStyle,
    pub preferences_draft_multisheet_style: MultisheetStyle,
    pub grid_style: GridStyle,
    pub preferences_draft_grid_style: GridStyle,
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
    pub preferences_draft_symbol_grid_size_mm: f32,
    /// Symbol-editor grid display style (Dots / Crosses / Lines).
    pub preferences_draft_symbol_grid_style: crate::render_config::GridStyle,
    /// Symbol-editor pin-selection mode (pin body only / body + labels).
    pub preferences_draft_symbol_pin_selection: crate::render_config::PinSelectionMode,
    /// Keyboard Shortcuts pane — editable working copy of the shortcut
    /// profile set. Seeded from [`keymap_profiles`] each time the
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
    /// close request that intersects `dirty_paths`. Cleared on any
    /// of the three button choices.
    pub project_close_confirm: Option<crate::app::ProjectCloseConfirmState>,
    /// "Exit Signex — Unsaved Edits" confirmation modal. `Some` while
    /// the user is being asked to save / discard / cancel an app-exit
    /// request that intersects `dirty_paths`. Cleared on any of the
    /// three button choices.
    pub app_quit_confirm: Option<crate::app::AppQuitConfirmState>,
    /// v0.9 read-only Project Options metadata modal — `Some` while
    /// the user has the right-click → Project Options… popup open.
    pub project_options: Option<crate::app::ProjectOptionsState>,
    /// v0.11 Enable Version Control modal — opened from the
    /// project root context menu when the project dir has no
    /// `.git/` yet.
    pub enable_version_control: Option<crate::app::EnableVersionControlState>,
    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a
    /// footprint editor). `Some` while the dialog is visible;
    /// carries the in-flight Step X / Step Y / link buffers so
    /// keystrokes don't immediately mutate the active editor.
    pub grid_properties: Option<crate::app::GridPropertiesState>,
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
    /// Global cursor into the flattened ERC diagnostics list spanning all
    /// sheets in `erc_violations_by_path`. Used by next/prev navigation.
    pub erc_focus_global_index: Option<usize>,
    /// Cached authoritative project netlist (ADR-0002 D7). Derived state, held
    /// like any other Model state: recomputed in `update` off the shared sheet
    /// view and invalidated whenever a schematic edit touches connectivity
    /// (`finish_schematic_mutation`). `None` means "recompute before next use".
    pub project_netlist: Option<signex_net::ProjectNetlist>,
    /// Per-rule severity override — if empty, the rule's default is used.
    pub erc_severity_override:
        std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    /// Net-color overrides keyed by net-label text. Superseded by the
    /// per-wire `wire_color_overrides` map below which the Active-Bar
    /// net-colour flood populates; kept here so a future net-name
    /// palette (maybe the F5 dialog) can cross-reference it without
    /// another round-trip through state plumbing.
    #[allow(dead_code)]
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
    /// Altium-style lasso in flight. `Some(points)` means the user
    /// started a lasso — each canvas click appends a vertex; a
    /// double-click or a click on the first vertex closes the polygon
    /// and commits the selection. Escape or right-click cancels.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// App-level undo stack for net-color floods. Each entry is the
    /// full `wire_color_overrides` map captured before an action —
    /// popping one restores the previous state. This is separate from
    /// the engine's undo because net colours are render-only and
    /// shouldn't mix with document mutations.
    pub net_color_undo: Vec<std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>>,
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
    /// Command palette state — query / dropdown open flag / selected
    /// row. The chrome-strip search bar is the always-rendered input;
    /// `open` gates the dropdown overlay only.
    pub command_palette: super::super::command_palette::CommandPaletteState,
}

impl UiState {
    /// True when any Preferences draft differs from its saved live value.
    /// Single source of truth so every live-preview `PrefMsg::Draft*` handler
    /// stays consistent as new drafts are added (the old per-handler inline
    /// chains had drifted to different term sets). Covers ALL draft state —
    /// the 7 appearance drafts, the component-class table and the keymap
    /// working copy — so an appearance recompute can't report "clean" while
    /// a pending rebind or class edit would be lost on close.
    pub fn preferences_draft_differs(&self) -> bool {
        self.preferences_draft_theme != self.theme_id
            || self.preferences_draft_font != self.ui_font_name
            || self.preferences_draft_power_port_style != self.power_port_style
            || self.preferences_draft_label_style != self.label_style
            || self.preferences_draft_multisheet_style != self.multisheet_style
            || self.preferences_draft_grid_style != self.grid_style
            || self.preferences_draft_pcb_gpu_render != self.pcb_gpu_render
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
