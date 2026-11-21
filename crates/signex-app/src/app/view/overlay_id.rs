//! Overlay identity and paint order (#535).
//!
//! One ordered list, [`PAINT_ORDER`], that both consumers derive from:
//!
//! * the painter — `collect_overlays` walks it FORWARD, so later entries
//!   stack on top of earlier ones;
//! * the Esc ladder — `OpenOverlays::escape_message`
//!   (`app/bootstrap/subscription.rs`) walks it BACKWARD and takes the
//!   first open overlay, i.e. the topmost painted one.
//!
//! Before this module the two orders were maintained by hand in two
//! places, in opposite directions, with a doc comment asking the next
//! author to keep them in sync. Every ordering bug found across #514's
//! review rounds was that rule violated in one spot. Now there is one
//! list and no rule to remember: moving a variant here moves the paint
//! slot and the Esc rung together, because they are the same fact read
//! from opposite ends.
//!
//! # Openness is not visibility
//!
//! A blocking modal suppresses everything painted after it, and the
//! suppressed overlays' state stays set — Alt+F4 sets `app_quit_confirm`
//! with no modal check anywhere on that path, and keymap strokes are not
//! modal-gated either. So an overlay can be *open* and not *painted*.
//! [`visible`] is the one predicate that resolves that difference, and
//! both sides go through it; neither may walk [`PAINT_ORDER`] directly.

use super::*;

/// Every floating overlay the main window can paint — one variant per
/// builder in this module's siblings (`bars.rs`, `modals.rs`,
/// `overlays/mod.rs`) and in `view/mod.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OverlayId {
    // Pre-blocking block — painted before the blocking-modal cutoff, so
    // these are the only overlays that survive it. See `visible`.
    //
    // `ErrorNotice` is the app's one generic error card: an export
    // failure, a save that blocked a project close, a primitive that
    // would not open. It carried the hardcoded heading "Export Failed"
    // until #532, which is why a failed project close used to announce
    // itself as an export problem.
    ErrorNotice,
    NetlistIncompletePrompt,
    PrintPreview,
    BomPreview,
    NetColorCustom,

    // Editor-surface chrome.
    PlacementPaused,
    SchematicActiveBar,
    /// The Graphite zoom pill over the schematic canvas.
    CanvasZoomPill,
    FootprintActiveBar,
    FootprintMoveBy,
    FootprintAlign,
    SymbolEditorActiveBar,
    TextEdit,

    // The Active Bar dropdown, dock drag zones, floating panels.
    ActiveBarMenu,
    DockDragZone,
    ComponentsRowDragLabel,
    FloatingPanels,

    // Right-click menus and pickers: above the floating panels, so a menu
    // opened from one is not drawn under it (#2376).
    FootprintContextMenu,
    SymbolContextMenu,
    ContextMenu,
    TabContextMenu,
    ProjectTreeContextMenu,
    ComponentsRowContextMenu,
    /// Right-click menu of a Start screen project row (#2305).
    StartRowContextMenu,
    /// Right-click menu of a board layer tab (#2379).
    LayerTabContextMenu,
    GridPicker,
    PanelList,
    /// The zoom pill's preset menu (Graphite mockup #64).
    ZoomPresets,
    /// The status bar's Grid & Snap popover (mockup row 6).
    GridSnapPopover,
    /// The popover a board counter of the status bar opened (#2460).
    BoardStatusPopover,

    // Dialogs.
    Preferences,
    FindReplace,
    FindSimilar,
    PasteArray,
    /// The Create Union popover (mockup row 158).
    UnionPopover,
    /// Edit ▸ Paste Special (mockup row 155).
    SmartPaste,
    KeyboardShortcuts,
    About,
    PassiveCalculator,
    FirstRunTour,
    AutosaveRecovery,
    RenameDialog,
    RemoveDialog,
    ProjectCloseConfirm,
    CreateChildSheetConfirm,
    AppQuitConfirm,
    EnableVersionControl,
    GridProperties,
    SelectionFilterCustom,
    /// File > New Project (#2232): the setup dialog.
    NewProject,
    /// Tools ▸ Assign Footprints (#2248).
    AssignFootprints,
    /// Tools ▸ Annotation ▸ Number Schematic Sheets (#2247).
    SheetNumbering,
    /// Design ▸ Synchronize Sheet Entries and Ports (#1276).
    SheetSync,
    /// Tools ▸ Re-Annotate Board (#1312).
    BoardReannotate,
    /// Design ▸ Rooms ▸ Create Rooms from Sheets (#2457).
    CreateRooms,
    /// File ▸ Fabrication Outputs (#2388).
    FabOutputs,
    /// Design ▸ Differential Pair Classes… (#1282).
    DiffPairClasses,
    /// Board Reports and Board Print / PDF (#2404, #2402).
    BoardOutput,
    /// Tools ▸ Annotation ▸ Back Annotate Schematics (#1312).
    BackAnnotate,
    /// Project ▸ Compare Board with Schematic… (#2462).
    CompareBoard,
    /// Its confirmation listing the board deletions, above it.
    CompareBoardConfirm,
    AnnotateDialog,
    AnnotateResetConfirm,
    ReprefixConfirm,
    ErcDialog,
    /// Update PCB from schematic (#1312).
    UpdatePcb,
    /// New PCB / New Schematic setup (#2232).
    DocumentSetup,
    /// View ▸ Window ▸ Layouts ▸ Manage Layouts (mockup row 166).
    ManageLayouts,
    /// View ▸ Window ▸ Layouts ▸ Save Layout (mockup row 166).
    SaveLayout,

    // Library modals.
    LibraryPicker,
    EditRowModal,
    DeleteConfirm,
    PrimitivePicker,
    DocumentOptions,
    CreateOptions,
    StockCopy,
    CloseLibraryConfirm,
    LibraryRecovery,

    // Painted last, above everything else.
    CommandPalette,
    HoverTooltip,
    LibraryUpdates,
}

/// How many leading [`PAINT_ORDER`] entries paint BEFORE the
/// blocking-modal cutoff.
///
/// The visible set is always a prefix of [`PAINT_ORDER`] — the cutoff is
/// a single early return in `collect_overlays`, not a scattered set of
/// skips — which is why [`visible`] can hand back a borrowed slice
/// instead of building a list per frame.
const PRE_BLOCKING: usize = 5;

/// Paint order, bottom-most first. Later entries stack on top.
///
/// This array IS the ordering contract. Reordering it is a deliberate
/// behaviour change to both the paint stack and the Esc precedence, and
/// `paint_order_is_exhaustive_and_unique` keeps it honest about covering
/// every [`OverlayId`] exactly once.
pub(crate) const PAINT_ORDER: [OverlayId; 81] = [
    OverlayId::ErrorNotice,
    OverlayId::NetlistIncompletePrompt,
    OverlayId::PrintPreview,
    OverlayId::BomPreview,
    OverlayId::NetColorCustom,
    OverlayId::PlacementPaused,
    OverlayId::SchematicActiveBar,
    OverlayId::CanvasZoomPill,
    OverlayId::FootprintActiveBar,
    OverlayId::FootprintMoveBy,
    OverlayId::FootprintAlign,
    OverlayId::SymbolEditorActiveBar,
    OverlayId::TextEdit,
    OverlayId::ActiveBarMenu,
    OverlayId::DockDragZone,
    OverlayId::ComponentsRowDragLabel,
    OverlayId::FloatingPanels,
    // Every right-click menu and picker paints above the floating panels, so
    // one opened from a floating panel is on top and takes the clicks (#2376).
    OverlayId::FootprintContextMenu,
    OverlayId::SymbolContextMenu,
    OverlayId::ContextMenu,
    OverlayId::TabContextMenu,
    OverlayId::ProjectTreeContextMenu,
    OverlayId::ComponentsRowContextMenu,
    OverlayId::StartRowContextMenu,
    OverlayId::LayerTabContextMenu,
    OverlayId::GridPicker,
    OverlayId::PanelList,
    OverlayId::ZoomPresets,
    OverlayId::GridSnapPopover,
    OverlayId::BoardStatusPopover,
    OverlayId::Preferences,
    OverlayId::FindReplace,
    OverlayId::FindSimilar,
    OverlayId::PasteArray,
    OverlayId::UnionPopover,
    OverlayId::SmartPaste,
    OverlayId::KeyboardShortcuts,
    OverlayId::About,
    OverlayId::PassiveCalculator,
    OverlayId::FirstRunTour,
    OverlayId::AutosaveRecovery,
    OverlayId::RenameDialog,
    OverlayId::RemoveDialog,
    // Below the close and quit confirms: they must outrank it for Esc and the lock.
    OverlayId::NewProject,
    OverlayId::ProjectCloseConfirm,
    OverlayId::CreateChildSheetConfirm,
    OverlayId::AppQuitConfirm,
    OverlayId::EnableVersionControl,
    OverlayId::GridProperties,
    OverlayId::SelectionFilterCustom,
    OverlayId::AssignFootprints,
    OverlayId::SheetNumbering,
    OverlayId::SheetSync,
    OverlayId::BoardReannotate,
    OverlayId::CreateRooms,
    OverlayId::FabOutputs,
    OverlayId::DiffPairClasses,
    OverlayId::BoardOutput,
    OverlayId::BackAnnotate,
    OverlayId::CompareBoard,
    // Above its dialog: Esc closes the confirmation first.
    OverlayId::CompareBoardConfirm,
    OverlayId::AnnotateDialog,
    OverlayId::AnnotateResetConfirm,
    OverlayId::ReprefixConfirm,
    OverlayId::ErcDialog,
    OverlayId::UpdatePcb,
    // Above Update PCB: its Create PCB opens this dialog on top.
    OverlayId::DocumentSetup,
    OverlayId::ManageLayouts,
    OverlayId::SaveLayout,
    OverlayId::LibraryPicker,
    OverlayId::EditRowModal,
    OverlayId::DeleteConfirm,
    OverlayId::PrimitivePicker,
    OverlayId::DocumentOptions,
    OverlayId::CreateOptions,
    OverlayId::StockCopy,
    OverlayId::CloseLibraryConfirm,
    OverlayId::LibraryRecovery,
    OverlayId::CommandPalette,
    OverlayId::HoverTooltip,
    OverlayId::LibraryUpdates,
];

/// The overlays that can actually reach the screen right now.
///
/// A blocking modal owns the stack: `collect_overlays` returns right
/// after the pre-blocking block, so nothing later paints even when its
/// state is set. Both the painter and the Esc ladder must agree on that
/// or Esc dismisses a card the user cannot see — see the module docs.
///
/// Callers pass their own `has_blocking_modal` on purpose — the painter's
/// (`Signex::has_blocking_modal`, `bars.rs`) reads live state, the Esc
/// ladder's (`OpenOverlays::has_blocking_modal`) reads a snapshot — but
/// the two must agree term for term. They did not until #547: the ladder
/// filtered a print preview detached into its own OS window and the
/// painter did not, so with the preview detached the painter suppressed
/// the whole stack while nothing painted in its place, and Esc resolved
/// against overlays that were not on screen. The divergence predated this
/// module; #535 part 2 wrote it down here rather than fix it, because
/// unifying it inside a refactor whose whole claim is behaviour-neutrality
/// would have smuggled a real behaviour change through that claim.
/// Whether this overlay must stop a mouse wheel from reaching the canvas
/// underneath it (#562).
///
/// The bug: scrolling past the end of a modal's content kept going and
/// panned/zoomed the schematic behind it. Two facts compose into that —
/// `scrollable` stops capturing once it hits its limit (ordinary scroll
/// chaining), and nothing below it captured either. `Stack::update`
/// walks its layers top-down and returns on `shell.is_event_captured()`
/// (`iced_widget-0.14.2/src/stack.rs`), so a layer that captures does
/// shield what is under it — there simply was not one.
///
/// Wiring `on_scroll` on the two backdrops (`wrap_modal`,
/// `dismiss_layer`) was necessary but not sufficient: most overlays build
/// neither. Preferences, for one, renders its body directly. So the
/// guard is applied in `collect_overlays` instead, where every overlay
/// passes through exactly once no matter how its builder is written, except
/// the ids that guard their own hit areas (see the arm below).
///
/// Exhaustive on purpose: a new overlay has to say whether the wheel
/// stops at it.
pub(crate) fn swallows_scroll(id: OverlayId) -> bool {
    match id {
        // The hover tooltip follows the cursor and paints nothing the
        // user aims at. Capturing there would kill wheel-zoom over the
        // canvas whenever a tooltip happened to be showing.
        OverlayId::HoverTooltip | OverlayId::ComponentsRowDragLabel => false,
        // Its layer is window-sized (the pill is placed by `Translate`), so a
        // guard around the layer would swallow every wheel in the window:
        // no panel would scroll. The pill guards its own bounds instead.
        OverlayId::CanvasZoomPill => false,

        // Their layers are window-sized positioners (a `Fill` column, the zoom
        // pill's `Translate`): a guard there took the wheel over the whole
        // canvas. The builders guard the bar, pill and dropdown themselves
        // (`overlays::scroll_guard`).
        OverlayId::PlacementPaused
        | OverlayId::SchematicActiveBar
        | OverlayId::FootprintActiveBar
        | OverlayId::SymbolEditorActiveBar => false,

        // Everything else paints a surface over the canvas. Scrolling on
        // it — or past the end of its own scrollable — must not reach
        // through.
        OverlayId::ErrorNotice
        | OverlayId::NetlistIncompletePrompt
        | OverlayId::PrintPreview
        | OverlayId::BomPreview
        | OverlayId::NetColorCustom
        | OverlayId::FootprintContextMenu
        | OverlayId::FootprintMoveBy
        | OverlayId::FootprintAlign
        | OverlayId::SymbolContextMenu
        | OverlayId::TextEdit
        | OverlayId::ActiveBarMenu
        | OverlayId::ContextMenu
        | OverlayId::TabContextMenu
        | OverlayId::ProjectTreeContextMenu
        | OverlayId::ComponentsRowContextMenu
        | OverlayId::StartRowContextMenu
        | OverlayId::LayerTabContextMenu
        | OverlayId::GridPicker
        | OverlayId::PanelList
        | OverlayId::ZoomPresets
        | OverlayId::GridSnapPopover
        | OverlayId::BoardStatusPopover
        | OverlayId::DockDragZone
        | OverlayId::FloatingPanels
        | OverlayId::Preferences
        | OverlayId::FindReplace
        | OverlayId::FindSimilar
        | OverlayId::PasteArray
        | OverlayId::UnionPopover
        | OverlayId::SmartPaste
        | OverlayId::KeyboardShortcuts
        | OverlayId::About
        | OverlayId::PassiveCalculator
        | OverlayId::FirstRunTour
        | OverlayId::AutosaveRecovery
        | OverlayId::RenameDialog
        | OverlayId::RemoveDialog
        | OverlayId::ProjectCloseConfirm
        | OverlayId::CreateChildSheetConfirm
        | OverlayId::AppQuitConfirm
        | OverlayId::EnableVersionControl
        | OverlayId::GridProperties
        | OverlayId::SelectionFilterCustom
        | OverlayId::NewProject
        | OverlayId::AssignFootprints
        | OverlayId::SheetNumbering
        | OverlayId::SheetSync
        | OverlayId::BoardReannotate
        | OverlayId::CreateRooms
        | OverlayId::FabOutputs
        | OverlayId::DiffPairClasses
        | OverlayId::BoardOutput
        | OverlayId::BackAnnotate
        | OverlayId::CompareBoard
        | OverlayId::CompareBoardConfirm
        | OverlayId::AnnotateDialog
        | OverlayId::AnnotateResetConfirm
        | OverlayId::ReprefixConfirm
        | OverlayId::ErcDialog
        | OverlayId::UpdatePcb
        | OverlayId::DocumentSetup
        | OverlayId::ManageLayouts
        | OverlayId::SaveLayout
        | OverlayId::LibraryPicker
        | OverlayId::EditRowModal
        | OverlayId::DeleteConfirm
        | OverlayId::PrimitivePicker
        | OverlayId::DocumentOptions
        | OverlayId::CreateOptions
        | OverlayId::StockCopy
        | OverlayId::CloseLibraryConfirm
        | OverlayId::LibraryRecovery
        | OverlayId::CommandPalette
        | OverlayId::LibraryUpdates => true,
    }
}

/// Whether this overlay's layers are a right-click menu over its
/// `MenuBackdrop` (`Signex::anchored_layers`), which stops the wheel itself.
/// `collect_overlays` must not wrap them in `scroll_guard`: the guard claims
/// the pointer over the whole window-sized layer, which levitated the cursor
/// off the backdrop, so a click outside the menu never closed it (#2349).
pub(crate) fn has_menu_backdrop(id: OverlayId) -> bool {
    matches!(
        id,
        OverlayId::FootprintContextMenu
            | OverlayId::SymbolContextMenu
            | OverlayId::ContextMenu
            | OverlayId::TabContextMenu
            | OverlayId::ProjectTreeContextMenu
            | OverlayId::ComponentsRowContextMenu
            | OverlayId::StartRowContextMenu
            | OverlayId::LayerTabContextMenu
            | OverlayId::GridPicker
            | OverlayId::UnionPopover
            | OverlayId::ZoomPresets
    )
}

/// Whether this overlay, once painted, takes the cursor off the canvas, so
/// the hover card must not show. Surfaces only: not the paused-placement
/// banner, the text editor, the dock drop preview or floating panels.
pub(crate) fn covers_canvas_hover(id: OverlayId) -> bool {
    swallows_scroll(id)
        && !matches!(
            id,
            OverlayId::TextEdit | OverlayId::DockDragZone | OverlayId::FloatingPanels
        )
}

/// Tag the Active Bar and context-menu messages of overlay `layers` painted
/// in the undocked window `owner`, so `update` runs them against the sheet
/// that window shows (#1910). `None` (the main window) leaves them as is.
fn scoped_to_window<'a>(
    layers: impl IntoIterator<Item = Element<'a, Message>>,
    owner: Option<iced::window::Id>,
) -> Vec<Element<'a, Message>> {
    layers
        .into_iter()
        .map(|layer| match owner {
            None => layer,
            Some(window) => layer
                .map(move |message| scope_overlay_message(window, message))
                .boxed(),
        })
        .collect()
}

/// Tag an Active Bar or context-menu `message` with the undocked `window`
/// that sent it; anything else passes through (#1910).
pub(crate) fn scope_overlay_message(window: iced::window::Id, message: Message) -> Message {
    match message {
        Message::ActiveBar(inner) => Message::ActiveBar(
            crate::active_bar::ActiveBarMsg::ScopedToWindow(window, Box::new(inner)),
        ),
        Message::ContextMenu(inner) => {
            Message::ContextMenu(ContextMenuMsg::ScopedToWindow(window, Box::new(inner)))
        }
        other => other,
    }
}

/// Remap what an undocked window's canvas emits: canvas events carry the
/// window, a right-click opens the menu for that window's sheet, and an
/// Active Bar button pressed on the canvas (right-press, #1509) acts there
/// (#1910).
pub(crate) fn scope_canvas_message(window_id: iced::window::Id, message: Message) -> Message {
    match message {
        Message::CanvasEvent(event) => Message::CanvasEventInWindow { window_id, event },
        show @ Message::ContextMenu(ContextMenuMsg::Show(..)) => {
            scope_overlay_message(window_id, show)
        }
        bar @ Message::ActiveBar(_) => scope_overlay_message(window_id, bar),
        other => other,
    }
}

/// Remap what an undocked window's PCB canvas emits: canvas events and the
/// board's selection and drag messages carry the window, and a right-click
/// opens the menu for that window's board (#2240).
pub(crate) fn scope_pcb_canvas_message(window_id: iced::window::Id, message: Message) -> Message {
    match message {
        Message::PcbCanvas(msg) => Message::Pcb(crate::app::PcbMsg::CanvasInWindow {
            window: window_id,
            msg,
        }),
        other => scope_canvas_message(window_id, other),
    }
}

pub(crate) fn visible(has_blocking_modal: bool) -> &'static [OverlayId] {
    if has_blocking_modal {
        &PAINT_ORDER[..PRE_BLOCKING]
    } else {
        &PAINT_ORDER
    }
}

impl Signex {
    /// Append `id`'s layers to the stack under construction.
    ///
    /// Each builder keeps its own open/closed guard and returns nothing
    /// when it has nothing to paint; several push two layers (a
    /// click-outside dismiss backdrop, then the card). Both shapes
    /// `extend` the same way, so the arms stay one line each and the
    /// exhaustive match is what guarantees no overlay is silently
    /// dropped from the stack.
    pub(in crate::app::view) fn extend_overlay<'a>(
        &'a self,
        layers: &mut Vec<Element<'a, Message>>,
        id: OverlayId,
        window_id: iced::window::Id,
    ) {
        match id {
            OverlayId::ErrorNotice => layers.extend(self.error_notice_overlay()),
            OverlayId::NetlistIncompletePrompt => {
                layers.extend(self.netlist_incomplete_prompt_overlay())
            }
            OverlayId::PrintPreview => layers.extend(self.print_preview_overlay()),
            OverlayId::BomPreview => layers.extend(self.bom_preview_overlay()),
            OverlayId::NetColorCustom => layers.extend(self.net_color_custom_overlay()),
            OverlayId::PlacementPaused => layers.extend(self.placement_paused_overlay(window_id)),
            OverlayId::SchematicActiveBar => layers.extend(scoped_to_window(
                self.schematic_active_bar_overlay(window_id),
                self.overlay_owner(window_id),
            )),
            OverlayId::CanvasZoomPill => layers.extend(scoped_to_window(
                self.canvas_zoom_pill_overlay(window_id),
                self.overlay_owner(window_id),
            )),
            OverlayId::FootprintActiveBar => {
                layers.extend(self.footprint_active_bar_overlay(window_id))
            }
            OverlayId::FootprintContextMenu => {
                layers.extend(self.footprint_context_menu_overlay(window_id))
            }
            OverlayId::FootprintMoveBy => layers.extend(self.footprint_move_by_overlay(window_id)),
            OverlayId::FootprintAlign => layers.extend(self.footprint_align_overlay(window_id)),
            OverlayId::SymbolEditorActiveBar => {
                layers.extend(self.symbol_editor_active_bar_overlay(window_id))
            }
            OverlayId::SymbolContextMenu => {
                layers.extend(self.symbol_context_menu_overlay(window_id))
            }
            OverlayId::TextEdit => layers.extend(self.text_edit_overlay(window_id)),
            OverlayId::ActiveBarMenu => layers.extend(scoped_to_window(
                self.active_bar_menu_overlay(window_id),
                self.overlay_owner(window_id),
            )),
            OverlayId::ContextMenu => layers.extend(scoped_to_window(
                self.context_menu_overlay(window_id),
                self.overlay_owner(window_id),
            )),
            OverlayId::TabContextMenu => layers.extend(self.tab_context_menu_overlay(window_id)),
            OverlayId::ProjectTreeContextMenu => layers.extend(scoped_to_window(
                self.project_tree_context_menu_overlay(window_id),
                self.overlay_owner(window_id),
            )),
            OverlayId::ComponentsRowContextMenu => {
                layers.extend(self.components_row_context_menu_overlay(window_id))
            }
            OverlayId::StartRowContextMenu => {
                layers.extend(self.start_row_context_menu_overlay(window_id))
            }
            OverlayId::LayerTabContextMenu => layers.extend(self.layer_tab_menu_overlay(window_id)),
            OverlayId::GridPicker => layers.extend(self.grid_picker_overlay()),
            OverlayId::PanelList => layers.extend(self.panel_list_overlay()),
            OverlayId::ZoomPresets => layers.extend(self.zoom_presets_overlay(window_id)),
            OverlayId::GridSnapPopover => layers.extend(self.grid_snap_popover_overlay(window_id)),
            OverlayId::BoardStatusPopover => {
                if let Some(card) = self.board_popover_card(window_id) {
                    let dismissed = Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardPopover(
                        crate::status_bar::BoardPopoverEvent::Dismissed,
                    )));
                    layers.extend(Self::dismiss_over(dismissed, card));
                }
            }
            OverlayId::DockDragZone => layers.extend(self.dock_drag_zone_overlay()),
            OverlayId::ComponentsRowDragLabel => {
                layers.extend(self.components_row_drag_label_overlay(window_id))
            }
            OverlayId::FloatingPanels => layers.extend(self.floating_panels_overlay(window_id)),
            OverlayId::Preferences => layers.extend(self.preferences_overlay()),
            OverlayId::FindReplace => layers.extend(self.find_replace_overlay(window_id)),
            OverlayId::FindSimilar => layers.extend(self.find_similar_overlay()),
            OverlayId::PasteArray => layers.extend(self.paste_array_overlay()),
            OverlayId::UnionPopover => layers.extend(self.union_popover_overlay()),
            OverlayId::SmartPaste => layers.extend(self.smart_paste_overlay()),
            OverlayId::KeyboardShortcuts => layers.extend(self.keyboard_shortcuts_overlay()),
            OverlayId::About => layers.extend(self.about_overlay()),
            OverlayId::PassiveCalculator => layers.extend(self.passive_calculator_overlay()),
            OverlayId::FirstRunTour => layers.extend(self.first_run_tour_overlay()),
            OverlayId::AutosaveRecovery => layers.extend(self.autosave_recovery_overlay()),
            OverlayId::RenameDialog => layers.extend(self.rename_dialog_overlay()),
            OverlayId::RemoveDialog => layers.extend(self.remove_dialog_overlay()),
            OverlayId::ProjectCloseConfirm => layers.extend(self.project_close_confirm_overlay()),
            OverlayId::CreateChildSheetConfirm => {
                layers.extend(self.create_child_sheet_confirm_overlay())
            }
            OverlayId::AppQuitConfirm => layers.extend(self.app_quit_confirm_overlay()),
            OverlayId::EnableVersionControl => layers.extend(self.enable_version_control_overlay()),
            OverlayId::GridProperties => layers.extend(self.grid_properties_overlay()),
            OverlayId::SelectionFilterCustom => {
                layers.extend(self.selection_filter_custom_overlay())
            }
            OverlayId::NewProject => layers.extend(self.new_project_overlay()),
            OverlayId::AssignFootprints => layers.extend(self.assign_footprints_overlay()),
            OverlayId::SheetNumbering => layers.extend(self.sheet_numbering_overlay()),
            OverlayId::SheetSync => layers.extend(self.sheet_sync_overlay()),
            OverlayId::BoardReannotate => layers.extend(self.board_reannotate_overlay()),
            OverlayId::CreateRooms => layers.extend(self.create_rooms_overlay()),
            OverlayId::FabOutputs => layers.extend(self.fab_outputs_overlay()),
            OverlayId::DiffPairClasses => layers.extend(self.diff_pair_classes_overlay()),
            OverlayId::BoardOutput => layers.extend(self.board_output_overlay()),
            OverlayId::BackAnnotate => layers.extend(self.back_annotate_overlay()),
            OverlayId::CompareBoard => layers.extend(self.compare_board_overlay()),
            OverlayId::CompareBoardConfirm => {
                layers.extend(self.compare_board_confirm_overlay());
            }
            OverlayId::AnnotateDialog => layers.extend(self.annotate_dialog_overlay()),
            OverlayId::AnnotateResetConfirm => layers.extend(self.annotate_reset_confirm_overlay()),
            OverlayId::ReprefixConfirm => layers.extend(self.reprefix_confirm_overlay()),
            OverlayId::ErcDialog => layers.extend(self.erc_dialog_overlay()),
            OverlayId::UpdatePcb => layers.extend(self.update_pcb_overlay()),
            OverlayId::DocumentSetup => layers.extend(self.document_setup_overlay()),
            OverlayId::ManageLayouts => layers.extend(self.manage_layouts_overlay()),
            OverlayId::SaveLayout => layers.extend(self.save_layout_overlay()),
            OverlayId::LibraryPicker => layers.extend(self.library_picker_overlay()),
            OverlayId::EditRowModal => layers.extend(self.edit_row_modal_overlay()),
            OverlayId::DeleteConfirm => layers.extend(self.delete_confirm_overlay()),
            OverlayId::PrimitivePicker => layers.extend(self.primitive_picker_overlay()),
            OverlayId::DocumentOptions => layers.extend(self.document_options_overlay()),
            OverlayId::CreateOptions => layers.extend(self.create_options_overlay()),
            OverlayId::StockCopy => layers.extend(self.stock_copy_overlay()),
            OverlayId::CloseLibraryConfirm => layers.extend(self.close_library_confirm_overlay()),
            OverlayId::LibraryRecovery => layers.extend(self.library_recovery_overlay()),
            OverlayId::CommandPalette => layers.extend(self.command_palette_overlay(window_id)),
            OverlayId::HoverTooltip => layers.extend(self.view_hover_tooltip(window_id)),
            OverlayId::LibraryUpdates => layers.extend(self.library_updates_overlay()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn paint_order_is_exhaustive_and_unique() {
        // `PAINT_ORDER`'s declared length pins the count; a new variant
        // that never reaches the array is a compile error there, and a
        // variant listed twice (which would paint twice and give Esc two
        // rungs) is caught here.
        let unique: HashSet<OverlayId> = PAINT_ORDER.iter().copied().collect();
        assert_eq!(
            unique.len(),
            PAINT_ORDER.len(),
            "PAINT_ORDER lists an overlay more than once"
        );
    }

    /// The Grid & Snap popover (mockup row 6) paints above the floating
    /// panels and the dock drop zones, and below every dialog.
    #[test]
    fn the_grid_snap_popover_paints_above_the_floating_panels() {
        let slot = |id| PAINT_ORDER.iter().position(|o| *o == id).expect("listed");
        let popover = slot(OverlayId::GridSnapPopover);
        for under in [OverlayId::FloatingPanels, OverlayId::DockDragZone] {
            assert!(slot(under) < popover, "{under:?} paints over the popover");
        }
        assert!(popover < slot(OverlayId::Preferences));
    }

    #[test]
    fn a_blocking_modal_cuts_the_stack_down_to_the_pre_blocking_prefix() {
        assert_eq!(visible(true), &PAINT_ORDER[..PRE_BLOCKING]);
        assert_eq!(visible(false), &PAINT_ORDER[..]);
    }

    #[test]
    fn the_pre_blocking_prefix_is_exactly_the_five_that_share_the_early_slot() {
        // The four `has_blocking_modal` members plus `BomPreview`, which
        // paints in the same early block without being one of them —
        // "five, not four", the oddity the old hand-written Esc guard
        // spelled out in prose.
        assert_eq!(
            visible(true),
            [
                OverlayId::ErrorNotice,
                OverlayId::NetlistIncompletePrompt,
                OverlayId::PrintPreview,
                OverlayId::BomPreview,
                OverlayId::NetColorCustom,
            ]
        );
    }

    fn paint_slot(id: OverlayId) -> usize {
        PAINT_ORDER
            .iter()
            .position(|o| *o == id)
            .unwrap_or_else(|| panic!("{id:?} is not in PAINT_ORDER"))
    }

    /// #2376: every right-click menu and picker paints above the floating
    /// panels, so one opened from a floating panel is not drawn under it;
    /// the Esc ladder, walking the same list backwards, closes it first.
    #[test]
    fn every_menu_paints_above_the_floating_panels() {
        let floating = paint_slot(OverlayId::FloatingPanels);
        let menus = [
            OverlayId::FootprintContextMenu,
            OverlayId::SymbolContextMenu,
            OverlayId::ContextMenu,
            OverlayId::TabContextMenu,
            OverlayId::ProjectTreeContextMenu,
            OverlayId::ComponentsRowContextMenu,
            OverlayId::StartRowContextMenu,
            OverlayId::LayerTabContextMenu,
            OverlayId::GridPicker,
            OverlayId::PanelList,
            OverlayId::ZoomPresets,
            OverlayId::GridSnapPopover,
            OverlayId::BoardStatusPopover,
        ];
        for id in menus {
            assert!(
                paint_slot(id) > floating,
                "{id:?} paints under the floating panels"
            );
        }
        for id in PAINT_ORDER
            .iter()
            .copied()
            .filter(|id| has_menu_backdrop(*id))
        {
            assert!(
                paint_slot(id) > floating,
                "{id:?} paints under the floating panels"
            );
        }
    }

    /// #2376, through the whole main-window view: a right-click menu open over
    /// a floating panel is the topmost layer there, so its row takes the click.
    #[test]
    fn a_menu_over_a_floating_panel_takes_the_click() {
        use crate::layout_probe::{lay_out_texts, messages_on_click_at};
        const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

        let (mut app, main) = crate::canvas::overlay_camera_tests::graphite_app_showing(|_| {});
        app.ui_state.window_size = (WINDOW.width, WINDOW.height);
        let panel = crate::dock::FloatingPanel {
            kind: crate::panels::PanelKind::Properties,
            x: 500.0,
            y: 200.0,
            width: 320.0,
            height: 420.0,
            dragging: false,
        };
        let panel_rect = iced::Rectangle::new(
            iced::Point::new(panel.x, panel.y),
            iced::Size::new(panel.width, panel.height),
        );
        app.document_state.dock.floating.push(panel);
        app.interaction_state.last_mouse_pos = (panel_rect.x + 40.0, panel_rect.y + 60.0);
        let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(None)));
        assert!(app.interaction_state.project_tree_context_menu.is_some());

        let row = signex_i18n::t!(
            app.ui_state.i18n,
            signex_i18n::ids::DIALOG_NEW_PROJECT_ENTRY_ROW
        )
        .into_string();
        let texts = lay_out_texts(app.view(main), WINDOW);
        let at = texts
            .iter()
            .find(|t| t.text == row && panel_rect.contains(t.bounds.center()))
            .unwrap_or_else(|| panic!("no menu row over the floating panel: {texts:#?}"))
            .bounds
            .center();
        let messages = messages_on_click_at(app.view(main), WINDOW, None, at);
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Menu(crate::menu_bar::MenuMessage::NewProject))),
            "the click did not reach the menu row: {messages:?}"
        );
    }
}
