use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::CanvasEvent;
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::tab_bar::TabMessage;
use crate::toolbar::ToolMessage;

use super::selection_request::SelectionRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    LeftPanel,
    RightPanel,
    BottomPanel,
    /// The divider between the Components panel list and its details.
    ComponentsSplit,
}

#[derive(Debug, Clone)]
pub enum Message {
    Menu(MenuMessage),
    PassiveCalculator(signex_widgets::passive_calculator::CalculatorMessage),
    Tool(ToolMessage),
    /// Tab-bar message carrying the id of the window whose tab bar emitted
    /// it. Lets the handler distinguish main-window tab reorder/select
    /// (which mutates `document_state.active_tab`) from an undocked
    /// window's tab bar, which only has one visible tab and must not
    /// clobber the main window's active index.
    Tab {
        window_id: iced::window::Id,
        msg: TabMessage,
    },
    Dock(DockMessage),
    /// Debounced dock-layout persistence loop (#692), routed to
    /// `dispatch_dock_layout_message`.
    DockLayout(crate::dock::DockLayoutMsg),
    /// Debounced search-box persistence loop (#859), routed to
    /// `dispatch_search_prefs_message`.
    SearchPrefs(crate::persist::SearchPrefsMsg),
    /// Opt-in autosave loop (#104), routed to `dispatch_autosave_message`.
    Autosave(crate::autosave::AutosaveMsg),
    /// UI / chrome message family (ADR-0001 D3). Theme, unit + grid
    /// toggles, the grid picker, layout drag, window resize, and the
    /// status-bar request enum. Routed to `dispatch_ui_message`.
    Ui(UiMsg),
    CanvasEvent(CanvasEvent),
    /// Canvas event stamped with the window that produced it. The
    /// dispatch layer swaps the window's `CanvasSlot` into the
    /// main canvas slot for the duration of the handler so the
    /// hundreds of `active_canvas_mut()` call sites read and write the
    /// right canvas transparently. Keyboard-generated canvas events
    /// (FitAll shortcut, etc.) continue to use the unwrapped
    /// `Message::CanvasEvent` variant and always target the main
    /// window.
    CanvasEventInWindow {
        window_id: iced::window::Id,
        event: CanvasEvent,
    },
    /// Grid Properties dialog — namespaced (ADR-0001 D3).
    GridProperties(GridPropertiesMsg),
    /// Custom Selection Filter modal message family — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_selection_filter_message`.
    SelectionFilter(SelectionFilterMsg),
    /// File / save message family (ADR-0001 D3). Namespaced under
    /// `Message::File` and routed to `dispatch_file_message`.
    File(FileMsg),
    /// Edit-command message family (ADR-0001 D3). Namespaced under
    /// `Message::Edit` and routed to `dispatch_edit_message`.
    Edit(EditMsg),
    /// The local agent channel (#1924): an agent's request on a project's
    /// documents, or the listener reporting. Routed to
    /// `dispatch_agent_message`.
    Agent(crate::app::agent::AgentMsg),
    Selection(SelectionRequest),
    /// Keyboard-input family (ADR-0001 D3): Esc, unclaimed keys, and the
    /// footprint-editor mode shortcut. Routed to `dispatch_input_message`.
    Input(InputMsg),
    /// Overlay / modal-chrome message family (ADR-0001 D3). Panel-list
    /// toggle, panel open, find/replace open, keyboard-shortcuts and
    /// first-run-tour dismiss, modal drag, canvas focus-at, auto-focus
    /// toggle. Routed to `dispatch_overlay_message`.
    Overlay(OverlayMsg),
    ActiveBar(crate::active_bar::ActiveBarMsg),
    /// In-canvas text-edit message family (ADR-0001 D3). Namespaced
    /// under `Message::TextEdit` and routed to
    /// `dispatch_text_edit_message`.
    TextEdit(TextEditMsg),
    /// Context-menu subsystem message family — namespaced (ADR-0001
    /// D3). Canvas / project-tree / tab right-click menus plus the
    /// submenu hover state machine. Routed to
    /// `dispatch_context_menu_message`.
    ContextMenu(ContextMenuMsg),
    /// Project lifecycle message family (ADR-0001 D3). Namespaced
    /// under `Message::Project` and routed to `dispatch_project_message`.
    Project(ProjectMsg),
    /// Rename modal message family — namespaced (ADR-0001 D3). Routed
    /// to `dispatch_rename_message`.
    Rename(RenameMsg),
    /// Remove-from-project modal message family — namespaced (ADR-0001
    /// D3). Routed to `dispatch_remove_message`.
    Remove(RemoveMsg),
    /// Enable Version Control modal message family — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_enable_version_control_message`.
    EnableVersionControl(EnableVersionControlMsg),
    /// Preferences modal message family — namespaced (ADR-0001 D3).
    /// Routed to `dispatch_preferences_message`.
    Preferences(PreferencesMsg),
    FindReplaceMsg(crate::find_replace::FindReplaceMsg),
    /// Window lifecycle / docking / native-chrome message
    /// family (ADR-0001 D3). Routed to `dispatch_window_message`.
    Window(WindowMsg),
    /// ERC dialog message family — namespaced (ADR-0001 D3). Routed to
    /// `dispatch_erc_message`.
    Erc(ErcMsg),
    /// Annotate dialog message family — namespaced (ADR-0001 D3). Routed
    /// to `dispatch_annotate_message`.
    Annotate(AnnotateMsg),
    /// Move Selection dialog family (ADR-0001 D3). Altium numeric
    /// ΔX / ΔY move. Routed to `dispatch_move_selection_message`.
    MoveSelection(MoveSelectionMsg),
    /// Per-net colour overrides + F5 palette open/close — namespaced
    /// (ADR-0001 D3). Routed to `dispatch_net_color_message`.
    NetColor(NetColorMsg),
    /// Parameter Manager dialog family (ADR-0001 D3). Bulk parameter
    /// editor. Routed to `dispatch_parameter_manager_message`.
    ParameterManager(ParameterManagerMsg),
    /// PCB canvas pointer events: hover, click, box select (#2191).
    /// Routed to `dispatch_pcb_canvas_message`.
    PcbCanvas(crate::pcb_canvas::PcbCanvasMsg),
    /// PCB board-view events, e.g. showing or hiding a net's ratsnest
    /// (ADR-0001 D3). Routed to `dispatch_pcb_message`.
    Pcb(PcbMsg),
    /// Cross Probe and Cross Select Mode between the schematic and the
    /// board (#1312). Routed to `dispatch_cross_probe_message`.
    CrossProbe(CrossProbeMsg),
    /// Update PCB from schematic and New PCB (#1312). Routed to
    /// `dispatch_update_pcb_message`.
    UpdatePcb(crate::app::UpdatePcbMsg),
    /// Export subsystem message family — namespaced (ADR-0001 D3).
    /// PDF / netlist / BOM export lifecycle plus the export-error
    /// modal dismiss. Routed to `dispatch_export_message`.
    Export(ExportMsg),
    /// BOM-preview modal — namespaced family (ADR-0001 D3). Routed to
    /// `dispatch_bom_preview_message`.
    BomPreview(BomPreviewMsg),
    /// Print-preview modal — namespaced family (ADR-0001 D3). Routed to
    /// `dispatch_print_preview_message`.
    PrintPreview(PrintPreviewMsg),
    /// Library subsystem message — folded under one variant so
    /// the dispatcher can route to `library_dispatch::handle` in one
    /// shot. See `crate::library::LibraryMessage` for the inner
    /// shape.
    Library(crate::library::LibraryMessage),
    /// Command-palette message family (ADR-0001 D3). Namespaced under
    /// `Message::CommandPalette` and routed to
    /// `dispatch_command_palette_message`.
    CommandPalette(CommandPaletteMsg),
}

impl Message {
    /// The do-nothing message: widgets and async tasks that must produce
    /// a `Message` but have nothing to say.
    pub fn noop() -> Self {
        Message::Ui(UiMsg::Noop)
    }
}

mod cross_probe;
mod dialogs;
mod messages;
mod net_nav;
mod pcb;
mod state;

pub use cross_probe::CrossProbeMsg;
pub use dialogs::*;
pub use messages::*;
pub use net_nav::NetNavMsg;
pub use pcb::{BoardSketchMsg, DrcRunResult, PcbMsg, PcbViewMsg};
pub use state::*;

#[cfg(test)]
mod tests {
    /// Last path segments that wrap a sub-enum although they carry none of the
    /// usual suffixes. Empty today; add a name here rather than loosening the
    /// suffix rule.
    const SUB_ENUM_TYPES: &[&str] = &[];
    const SUB_ENUM_SUFFIXES: &[&str] = &["Msg", "Message", "Event", "Request"];
    /// Ratchet: flat leaves on the root `Message` may only go down.
    const MAX_FLAT_LEAVES: usize = 2;
    /// Sanity cap on all variants, so a wall of wrapped enums is still noticed.
    const MAX_VARIANTS: usize = 60;

    /// Is this variant line (one level of indent, first letter upper-case) a
    /// flat leaf? Only a single-field tuple variant whose type names a message
    /// sub-enum is namespaced; unit, struct and plain-value variants are leaves.
    // shortcut: goes by name only (`*Msg`/`*Message`/`*Event`/`*Request` is trusted to be a
    // sub-enum per house naming) and a variant wrapped across lines counts as a leaf (errs strict).
    fn is_flat_leaf(line: &str) -> bool {
        let rest = line.trim_start();
        let Some(open) = rest.find('(') else {
            return true;
        };
        if rest[..open].contains(['{', ',', ' ']) {
            return true;
        }
        let Some(inner) = rest[open + 1..].trim_end().strip_suffix("),") else {
            return true;
        };
        if inner.contains([',', '(', '<', '&']) {
            return true;
        }
        let ty = inner.rsplit("::").next().unwrap_or(inner).trim();
        !(SUB_ENUM_TYPES.contains(&ty) || SUB_ENUM_SUFFIXES.iter().any(|s| ty.ends_with(s)))
    }

    fn root_variant_lines() -> Vec<&'static str> {
        let src = include_str!("mod.rs");
        let start = src.find("pub enum Message {").expect("Message enum");
        let body = &src[start..];
        let end = body.find("\n}\n").expect("enum end");
        body[..end]
            .lines()
            .filter(|l| {
                l.starts_with("    ") && l[4..].starts_with(|c: char| c.is_ascii_uppercase())
            })
            .collect()
    }

    /// ADR-0001 D3: the root `Message` wraps one sub-enum per surface and
    /// carries no flat leaf tail. A new flat leaf must join a family instead.
    #[test]
    fn root_message_stays_namespaced() {
        let variants = root_variant_lines();
        let leaves: Vec<_> = variants.iter().filter(|l| is_flat_leaf(l)).collect();
        assert_eq!(
            leaves.len(),
            MAX_FLAT_LEAVES,
            "root Message flat leaves are {leaves:?}: add none, and lower MAX_FLAT_LEAVES when a leaf is removed"
        );
        assert!(
            variants.len() <= MAX_VARIANTS,
            "root Message has {} variants (max {MAX_VARIANTS})",
            variants.len()
        );
    }

    #[test]
    fn the_flat_leaf_classifier_tells_leaves_from_sub_enum_wrappers() {
        assert!(is_flat_leaf("    Noop,"));
        assert!(is_flat_leaf("    Tab {"));
        assert!(is_flat_leaf("    CanvasEventInWindow {"));
        assert!(is_flat_leaf("    Count(usize),"));
        assert!(is_flat_leaf("    Pair(UiMsg, u8),"));
        assert!(is_flat_leaf("    Pair(u8, UiMsg),"));
        assert!(is_flat_leaf("    Ref(&'static UiMsg),"));
        assert!(is_flat_leaf("    Gen(Foo<Bar>::UiMsg),"));
        assert!(is_flat_leaf("    Fun(fn(u8) -> UiMsg),"));
        assert!(!is_flat_leaf("    Menu(MenuMessage),"));
        assert!(!is_flat_leaf("    Ui(UiMsg),"));
        assert!(!is_flat_leaf("    CanvasEvent(CanvasEvent),"));
        assert!(!is_flat_leaf("    Selection(SelectionRequest),"));
        assert!(!is_flat_leaf("    Agent(crate::app::agent::AgentMsg),"));
    }
}
