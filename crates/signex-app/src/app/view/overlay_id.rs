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
    FootprintActiveBar,
    FootprintContextMenu,
    FootprintMoveBy,
    FootprintAlign,
    SymbolEditorActiveBar,
    SymbolContextMenu,
    TextEdit,

    // Right-click menus, pickers, dock drag zones, floating panels.
    ActiveBarMenu,
    ContextMenu,
    TabContextMenu,
    ProjectTreeContextMenu,
    GridPicker,
    PanelList,
    DockDragZone,
    FloatingPanels,

    // Dialogs.
    Preferences,
    FindReplace,
    KeyboardShortcuts,
    PassiveCalculator,
    FirstRunTour,
    RenameDialog,
    RemoveDialog,
    ProjectCloseConfirm,
    AppQuitConfirm,
    ProjectOptions,
    EnableVersionControl,
    GridProperties,
    SelectionFilterCustom,
    AnnotateDialog,
    AnnotateResetConfirm,
    ErcDialog,

    // Library modals.
    LibraryPicker,
    NewComponent,
    EditRowModal,
    DeleteConfirm,
    PrimitivePicker,
    DocumentOptions,
    CreateOptions,
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
pub(crate) const PAINT_ORDER: [OverlayId; 50] = [
    OverlayId::ErrorNotice,
    OverlayId::NetlistIncompletePrompt,
    OverlayId::PrintPreview,
    OverlayId::BomPreview,
    OverlayId::NetColorCustom,
    OverlayId::PlacementPaused,
    OverlayId::SchematicActiveBar,
    OverlayId::FootprintActiveBar,
    OverlayId::FootprintContextMenu,
    OverlayId::FootprintMoveBy,
    OverlayId::FootprintAlign,
    OverlayId::SymbolEditorActiveBar,
    OverlayId::SymbolContextMenu,
    OverlayId::TextEdit,
    OverlayId::ActiveBarMenu,
    OverlayId::ContextMenu,
    OverlayId::TabContextMenu,
    OverlayId::ProjectTreeContextMenu,
    OverlayId::GridPicker,
    OverlayId::PanelList,
    OverlayId::DockDragZone,
    OverlayId::FloatingPanels,
    OverlayId::Preferences,
    OverlayId::FindReplace,
    OverlayId::KeyboardShortcuts,
    OverlayId::PassiveCalculator,
    OverlayId::FirstRunTour,
    OverlayId::RenameDialog,
    OverlayId::RemoveDialog,
    OverlayId::ProjectCloseConfirm,
    OverlayId::AppQuitConfirm,
    OverlayId::ProjectOptions,
    OverlayId::EnableVersionControl,
    OverlayId::GridProperties,
    OverlayId::SelectionFilterCustom,
    OverlayId::AnnotateDialog,
    OverlayId::AnnotateResetConfirm,
    OverlayId::ErcDialog,
    OverlayId::LibraryPicker,
    OverlayId::NewComponent,
    OverlayId::EditRowModal,
    OverlayId::DeleteConfirm,
    OverlayId::PrimitivePicker,
    OverlayId::DocumentOptions,
    OverlayId::CreateOptions,
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
    ) {
        match id {
            OverlayId::ErrorNotice => layers.extend(self.error_notice_overlay()),
            OverlayId::NetlistIncompletePrompt => {
                layers.extend(self.netlist_incomplete_prompt_overlay())
            }
            OverlayId::PrintPreview => layers.extend(self.print_preview_overlay()),
            OverlayId::BomPreview => layers.extend(self.bom_preview_overlay()),
            OverlayId::NetColorCustom => layers.extend(self.net_color_custom_overlay()),
            OverlayId::PlacementPaused => layers.extend(self.placement_paused_overlay()),
            OverlayId::SchematicActiveBar => layers.extend(self.schematic_active_bar_overlay()),
            OverlayId::FootprintActiveBar => layers.extend(self.footprint_active_bar_overlay()),
            OverlayId::FootprintContextMenu => layers.extend(self.footprint_context_menu_overlay()),
            OverlayId::FootprintMoveBy => layers.extend(self.footprint_move_by_overlay()),
            OverlayId::FootprintAlign => layers.extend(self.footprint_align_overlay()),
            OverlayId::SymbolEditorActiveBar => {
                layers.extend(self.symbol_editor_active_bar_overlay())
            }
            OverlayId::SymbolContextMenu => layers.extend(self.symbol_context_menu_overlay()),
            OverlayId::TextEdit => layers.extend(self.text_edit_overlay()),
            OverlayId::ActiveBarMenu => layers.extend(self.active_bar_menu_overlay()),
            OverlayId::ContextMenu => layers.extend(self.context_menu_overlay()),
            OverlayId::TabContextMenu => layers.extend(self.tab_context_menu_overlay()),
            OverlayId::ProjectTreeContextMenu => {
                layers.extend(self.project_tree_context_menu_overlay())
            }
            OverlayId::GridPicker => layers.extend(self.grid_picker_overlay()),
            OverlayId::PanelList => layers.extend(self.panel_list_overlay()),
            OverlayId::DockDragZone => layers.extend(self.dock_drag_zone_overlay()),
            OverlayId::FloatingPanels => layers.extend(self.floating_panels_overlay()),
            OverlayId::Preferences => layers.extend(self.preferences_overlay()),
            OverlayId::FindReplace => layers.extend(self.find_replace_overlay()),
            OverlayId::KeyboardShortcuts => layers.extend(self.keyboard_shortcuts_overlay()),
            OverlayId::PassiveCalculator => layers.extend(self.passive_calculator_overlay()),
            OverlayId::FirstRunTour => layers.extend(self.first_run_tour_overlay()),
            OverlayId::RenameDialog => layers.extend(self.rename_dialog_overlay()),
            OverlayId::RemoveDialog => layers.extend(self.remove_dialog_overlay()),
            OverlayId::ProjectCloseConfirm => layers.extend(self.project_close_confirm_overlay()),
            OverlayId::AppQuitConfirm => layers.extend(self.app_quit_confirm_overlay()),
            OverlayId::ProjectOptions => layers.extend(self.project_options_overlay()),
            OverlayId::EnableVersionControl => layers.extend(self.enable_version_control_overlay()),
            OverlayId::GridProperties => layers.extend(self.grid_properties_overlay()),
            OverlayId::SelectionFilterCustom => {
                layers.extend(self.selection_filter_custom_overlay())
            }
            OverlayId::AnnotateDialog => layers.extend(self.annotate_dialog_overlay()),
            OverlayId::AnnotateResetConfirm => layers.extend(self.annotate_reset_confirm_overlay()),
            OverlayId::ErcDialog => layers.extend(self.erc_dialog_overlay()),
            OverlayId::LibraryPicker => layers.extend(self.library_picker_overlay()),
            OverlayId::NewComponent => layers.extend(self.new_component_overlay()),
            OverlayId::EditRowModal => layers.extend(self.edit_row_modal_overlay()),
            OverlayId::DeleteConfirm => layers.extend(self.delete_confirm_overlay()),
            OverlayId::PrimitivePicker => layers.extend(self.primitive_picker_overlay()),
            OverlayId::DocumentOptions => layers.extend(self.document_options_overlay()),
            OverlayId::CreateOptions => layers.extend(self.create_options_overlay()),
            OverlayId::CloseLibraryConfirm => layers.extend(self.close_library_confirm_overlay()),
            OverlayId::LibraryRecovery => layers.extend(self.library_recovery_overlay()),
            OverlayId::CommandPalette => layers.extend(self.command_palette_overlay()),
            OverlayId::HoverTooltip => layers.extend(self.view_hover_tooltip()),
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
}
