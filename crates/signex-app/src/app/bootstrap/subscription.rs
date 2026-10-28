//! `Signex::subscription` — keyboard / window / tick event wiring.
//! Split from `app/bootstrap.rs` as pure code motion.

use super::super::*;

use super::window_subscriptions;
use crate::app::view::overlay_id::OverlayId;
use iced::Subscription;

/// Which overlays are open, as the Esc ladder sees them.
///
/// A named struct rather than the positional tuple this used to be: with
/// twelve same-typed `bool`s, swapping two of them silently routed Esc to
/// the wrong modal, with nothing to catch it.
///
/// **Built in `update`, not in the subscription** (#535). Until then this
/// was baked into `Subscription::with`, which forced `Hash` and — by
/// choice, not by the bound — `Copy`. That made per-modal payloads
/// unrepresentable: the per-browser delete-confirm rung could not exist,
/// because its Cancel message needs an owned `PathBuf`. Resolving against
/// live state instead drops both, and removes a one-update staleness
/// window as well, since the subscription snapshot was rebuilt only
/// *after* `update` returned — so an Esc in the same frame as a
/// click-Close saw the pre-click world.
///
/// Only [`KeyContext`] stays in the hashed subscription payload now.
#[derive(Clone, Default)]
struct OpenOverlays {
    find_replace_open: bool,
    kbd_shortcuts_open: bool,
    about_open: bool,
    first_run_tour_open: bool,
    /// The recovery-copy prompt (#1164).
    autosave_recovery_open: bool,
    prefs_open: bool,
    annotate_open: bool,
    erc_open: bool,
    /// The ERC dialog's "Save as profile" form is showing (#1608). Esc
    /// closes it (or its overwrite question) before the dialog itself.
    erc_save_profile_open: bool,
    rename_open: bool,
    remove_open: bool,
    enable_vc_open: bool,
    library_create_options_open: bool,
    passive_calculator_open: bool,
    // #514 — the rest of the Esc ladder. Each closes exactly one gap that
    // used to fall through to the `Tool::Select` reset behind the modal.
    annotate_reset_confirm_open: bool,
    reprefix_confirm_open: bool,
    app_quit_confirm_open: bool,
    project_close_confirm_open: bool,
    create_child_sheet_confirm_open: bool,
    grid_properties_open: bool,
    selection_filter_custom_open: bool,
    library_picker_open: bool,
    library_document_options_open: bool,
    library_updates_open: bool,
    library_primitive_picker_open: bool,
    close_library_confirm_open: bool,
    /// `None` while no library-recovery dialog is open. `RecoveryDialog`
    /// (`crate::library::recovery`) itself carries a `PathBuf` (and, for
    /// the broken-binding case, a `String`/`Uuid`), so it can't be
    /// embedded directly in this plain-data struct — this discriminant
    /// is the cheap stand-in that lets Esc still pick the right one of
    /// the three dialogs' own Cancel-choice messages.
    recovery_kind: Option<RecoveryKind>,
    error_notice_open: bool,
    /// The notice lives in its own window: it no longer blocks the stack here.
    error_notice_detached: bool,
    netlist_incomplete_prompt_open: bool,
    netlist_prompt_detached: bool,
    print_preview_open: bool,
    bom_preview_open: bool,
    net_color_custom_open: bool,
    /// The `.snxlib` whose Library Browser has a delete-row confirm open,
    /// if any.
    ///
    /// A payload, not a `bool`, because
    /// `LibraryMessage::BrowserDeleteRowCancel` is addressed to a specific
    /// library — the modal is per-browser-tab. This is the field that
    /// could not exist while the struct travelled through
    /// `Subscription::with` as a `Copy` type, which is why this modal was
    /// the one documented hole in the ladder (#514, #526).
    delete_confirm: Option<std::path::PathBuf>,
    /// The `.snxlib` whose Library Browser has the Edit Component
    /// Details modal open, if any. Per-library, like `delete_confirm`.
    edit_row_modal: Option<std::path::PathBuf>,
}

// `KeyContext` used to live here — the slice of modal state the
// subscription closure branched on before producing any message. #557
// phase 1 deleted it along with the closure: those branches now run in
// `app/dispatch/input.rs` against live state, so the keyboard
// subscription carries no state at all and its identity is constant for
// the process lifetime.

/// Which of the three library-recovery flows is open — see
/// `OpenOverlays::recovery_kind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum RecoveryKind {
    LibraryMissing,
    GitMissing,
    BrokenBinding,
}

impl OpenOverlays {
    /// The Esc ladder: which modal a bare Esc dismisses. `None` means no
    /// modal claimed the key, so the caller falls through to
    /// `Message::EscapePressed` (the tool reset).
    ///
    /// **Ordering is not decided here.** It is read off
    /// [`PAINT_ORDER`](crate::app::view::overlay_id::PAINT_ORDER) walked
    /// BACKWARD — whatever paints on top wins Esc — so a reorder happens
    /// in exactly one place and reaches the painter and this ladder at
    /// once. Until #535 part 2 this was 25 hand-written `if`s that a doc
    /// comment asked you to keep in the exact reverse of a list in
    /// another file; every ordering bug found across #514's review rounds
    /// was that request not honoured in one spot, and patching the pairs
    /// one at a time kept reintroducing new violations elsewhere.
    ///
    /// [`visible`](crate::app::view::overlay_id::visible) supplies the
    /// blocking-modal cutoff, so the "resolve only within the visible
    /// five" guard is no longer written out here either — it falls out of
    /// walking a shorter slice.
    ///
    /// What this still does NOT quantify over is every modal in the app —
    /// only over *fields of `OpenOverlays`*. That gap is real (see
    /// `every_modal_claims_escape` at the foot of this file) and is
    /// exactly what let #511 (the passive calculator) and #514 (~14 more
    /// modals) ship with no rung at all. A field that IS here and has no
    /// rung is a live bug: Esc falls through and silently resets the
    /// active canvas tool *behind* the open modal.
    /// `every_ladder_field_claims_escape` fails if a new field arrives
    /// without one.
    fn escape_message(&self) -> Option<Message> {
        crate::app::view::overlay_id::visible(self.has_blocking_modal())
            .iter()
            .rev()
            .find_map(|id| self.rung(*id))
    }

    /// Whether a blocking modal owns the overlay stack — i.e. whether
    /// everything painted after the pre-blocking block is suppressed.
    ///
    /// Mirrors `Signex::has_blocking_modal`
    /// (`app/view/overlays/bars.rs`). The two agreed on three of their
    /// four terms until #547: `print_preview_open` was already false here
    /// once Print Preview had been detached into its own OS window, while
    /// the painter's predicate ignored detachment and kept suppressing —
    /// so the main window painted ZERO overlays behind a detached
    /// preview while Esc still resolved against the ones it wasn't
    /// painting. The painter now carries the same filter and the terms
    /// match one for one.
    fn has_blocking_modal(&self) -> bool {
        (self.error_notice_open && !self.error_notice_detached)
            || (self.netlist_incomplete_prompt_open && !self.netlist_prompt_detached)
            || self.print_preview_open
            || self.net_color_custom_open
    }

    /// Whether a confirm or warning dialog is open, the class that locks
    /// the system menu bar's model rows. Tool windows (ERC, BOM, Preferences,
    /// Annotate, Print Preview, calculator, find/replace, shortcuts) do not.
    fn has_locking_confirm(&self) -> bool {
        self.error_notice_open
            || self.netlist_incomplete_prompt_open
            || self.app_quit_confirm_open
            || self.project_close_confirm_open
            || self.create_child_sheet_confirm_open
            || self.rename_open
            || self.remove_open
            || self.annotate_reset_confirm_open
            || self.close_library_confirm_open
            || self.delete_confirm.is_some()
            || self.recovery_kind.is_some()
            || self.autosave_recovery_open
    }

    /// The Cancel/Close message `id` claims for Esc, or `None` when that
    /// overlay is closed — or has no rung by design.
    ///
    /// Arms are listed in `PAINT_ORDER` sequence so a reviewer can read
    /// the array and this match side by side. The match is exhaustive,
    /// which is what makes "a new overlay silently has no Esc" a compile
    /// error instead of a bug report.
    fn rung(&self, id: OverlayId) -> Option<Message> {
        match id {
            // ── Pre-blocking block ──────────────────────────────────
            // When a blocking modal is up these five are the ONLY ids
            // `visible` hands back, which is what stops Esc cancelling an
            // invisible dialog behind an export-error card: Alt+F4 sets
            // `app_quit_confirm` with no modal check on that path, and
            // keymap strokes are not modal-gated either, so those flags
            // really can be set while only the blocking card paints.
            //
            // `BomPreview` sits in this prefix without being one of the
            // four blocking members — the "five, not four" oddity the old
            // guard spelled out in prose.
            OverlayId::ErrorNotice => self
                .error_notice_open
                .then(|| Message::Overlay(OverlayMsg::DismissErrorNotice)),
            // Cancel path only — the prompt's other action is "Export
            // anyway (incomplete)", which Esc must never trigger.
            OverlayId::NetlistIncompletePrompt => self
                .netlist_incomplete_prompt_open
                .then(|| Message::Export(ExportMsg::NetlistCancelIncomplete)),
            OverlayId::PrintPreview => self
                .print_preview_open
                .then(|| Message::PrintPreview(PrintPreviewMsg::Close)),
            OverlayId::BomPreview => self
                .bom_preview_open
                .then(|| Message::BomPreview(BomPreviewMsg::Close)),
            OverlayId::NetColorCustom => self
                .net_color_custom_open
                .then(|| Message::NetColor(NetColorMsg::CustomShow(false))),

            // ── Editor chrome, menus, pickers, panels: no rung ───────
            // None of these is a modal; Esc over a context menu resets
            // the tool, as it always has.
            //
            // `FootprintAlign` looks like an omission and is not. #370
            // gives the footprint Align… dialog an Esc of its own,
            // resolved in `dispatch/mod.rs` AFTER this ladder returns
            // `None`. Folding it in here would rank it at paint slot 11,
            // i.e. above `BomPreview` at slot 4, changing what Esc does
            // when both are open. Arguably a fix — align does paint on
            // top — but a behaviour change, and part 2 is a derivation.
            OverlayId::PlacementPaused
            | OverlayId::SchematicActiveBar
            | OverlayId::FootprintActiveBar
            | OverlayId::FootprintContextMenu
            | OverlayId::FootprintMoveBy
            | OverlayId::FootprintAlign
            | OverlayId::SymbolEditorActiveBar
            | OverlayId::SymbolContextMenu
            | OverlayId::TextEdit
            | OverlayId::ActiveBarMenu
            | OverlayId::ContextMenu
            | OverlayId::TabContextMenu
            | OverlayId::ProjectTreeContextMenu
            | OverlayId::GridPicker
            | OverlayId::PanelList
            | OverlayId::DockDragZone
            | OverlayId::FloatingPanels => None,

            // ── Dialogs ─────────────────────────────────────────────
            OverlayId::Preferences => self
                .prefs_open
                .then(|| Message::Preferences(PreferencesMsg::Close)),
            OverlayId::FindReplace => self
                .find_replace_open
                .then(|| Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::Close)),
            OverlayId::KeyboardShortcuts => self
                .kbd_shortcuts_open
                .then(|| Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)),
            OverlayId::About => self
                .about_open
                .then(|| Message::Overlay(OverlayMsg::CloseAbout)),
            OverlayId::PassiveCalculator => self
                .passive_calculator_open
                .then(|| Message::Overlay(OverlayMsg::ClosePassiveCalculator)),
            OverlayId::FirstRunTour => self
                .first_run_tour_open
                .then(|| Message::Overlay(OverlayMsg::DismissFirstRunTour)),
            // "Decide later": Esc must neither load nor delete the copy.
            OverlayId::AutosaveRecovery => self.autosave_recovery_open.then(|| {
                Message::Autosave(crate::autosave::AutosaveMsg::Recovery(
                    crate::autosave::RecoveryChoice::Later,
                ))
            }),
            OverlayId::RenameDialog => self.rename_open.then(|| Message::Rename(RenameMsg::Close)),
            OverlayId::RemoveDialog => self.remove_open.then(|| Message::Remove(RemoveMsg::Close)),
            OverlayId::ProjectCloseConfirm => self
                .project_close_confirm_open
                .then(|| Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel))),
            OverlayId::CreateChildSheetConfirm => self
                .create_child_sheet_confirm_open
                .then(|| Message::Project(ProjectMsg::CreateChildSheetAnswered { create: false })),
            OverlayId::AppQuitConfirm => self
                .app_quit_confirm_open
                .then(|| Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::Cancel))),
            OverlayId::EnableVersionControl => self
                .enable_vc_open
                .then(|| Message::EnableVersionControl(EnableVersionControlMsg::Close)),
            OverlayId::GridProperties => self
                .grid_properties_open
                .then(|| Message::GridProperties(GridPropertiesMsg::Close)),
            OverlayId::SelectionFilterCustom => self
                .selection_filter_custom_open
                .then(|| Message::SelectionFilter(SelectionFilterMsg::CloseCustom)),
            OverlayId::AnnotateDialog => self
                .annotate_open
                .then(|| Message::Annotate(AnnotateMsg::CloseDialog)),
            // Child confirm of the annotate dialog, with its own Cancel
            // (`CloseResetConfirm`, not `CloseDialog`). It outranks its
            // parent — closing the parent first would orphan the confirm —
            // and it outranks it because it paints later, which
            // `PAINT_ORDER` now states instead of a comment asking you to
            // take it on trust.
            OverlayId::AnnotateResetConfirm => self
                .annotate_reset_confirm_open
                .then(|| Message::Annotate(AnnotateMsg::CloseResetConfirm)),
            // Child of the annotate dialog like the reset confirm; Esc cancels,
            // it must never confirm a rewrite of the whole project.
            OverlayId::ReprefixConfirm => self
                .reprefix_confirm_open
                .then(|| Message::Annotate(AnnotateMsg::ReprefixCancelled)),
            // The Save-as form is a child panel of the dialog: Esc closes it
            // (or steps back from its overwrite question) and never writes.
            OverlayId::ErcDialog => self.erc_open.then(|| {
                if self.erc_save_profile_open {
                    Message::Erc(ErcMsg::ProfileSaveCancelled)
                } else {
                    Message::Erc(ErcMsg::CloseDialog)
                }
            }),

            // ── Library modals ──────────────────────────────────────
            OverlayId::LibraryPicker => self
                .library_picker_open
                .then(|| Message::Library(crate::library::messages::LibraryMessage::ClosePicker)),
            // Addressed to ONE library, like `DeleteConfirm`.
            OverlayId::EditRowModal => self.edit_row_modal.clone().map(|library_path| {
                Message::Library(crate::library::messages::LibraryMessage::BrowserEdit {
                    library_path,
                    msg: crate::library::messages::BrowserEditMsg::Cancel,
                })
            }),
            // The one rung carrying owned data, and the reason the ladder
            // moved out of the subscription in part 1: this Cancel is
            // addressed to ONE library, and a `PathBuf` could not ride
            // inside the old `Copy` payload.
            OverlayId::DeleteConfirm => self.delete_confirm.clone().map(|library_path| {
                Message::Library(
                    crate::library::messages::LibraryMessage::BrowserDeleteRowCancel {
                        library_path,
                    },
                )
            }),
            OverlayId::PrimitivePicker => self.library_primitive_picker_open.then(|| {
                Message::Library(crate::library::messages::LibraryMessage::PrimitivePicker(
                    crate::library::messages::PrimitivePickerMsg::Cancel,
                ))
            }),
            OverlayId::DocumentOptions => self.library_document_options_open.then(|| {
                Message::Library(crate::library::messages::LibraryMessage::DocumentOptionsCancel)
            }),
            OverlayId::CreateOptions => self.library_create_options_open.then(|| {
                Message::Library(
                    crate::library::messages::LibraryMessage::LibraryCreateOptionsCancel,
                )
            }),
            OverlayId::CloseLibraryConfirm => self.close_library_confirm_open.then(|| {
                Message::Library(
                    crate::library::messages::LibraryMessage::CloseLibraryConfirm(
                        crate::library::messages::CloseLibraryChoice::Cancel,
                    ),
                )
            }),
            OverlayId::LibraryRecovery => self.recovery_kind.map(|kind| match kind {
                RecoveryKind::LibraryMissing => Message::Library(
                    crate::library::messages::LibraryMessage::RecoveryLibraryMissing(
                        crate::library::recovery::LibraryMissingChoice::Cancel,
                    ),
                ),
                RecoveryKind::GitMissing => Message::Library(
                    crate::library::messages::LibraryMessage::RecoveryGitMissing(
                        crate::library::recovery::GitMissingChoice::Cancel,
                    ),
                ),
                RecoveryKind::BrokenBinding => Message::Library(
                    crate::library::messages::LibraryMessage::RecoveryBrokenBinding(
                        crate::library::recovery::BrokenBindingChoice::Cancel,
                    ),
                ),
            }),

            // ── Painted last ────────────────────────────────────────
            // The command palette and the chord recorder swallow keyboard
            // input wholesale before the ladder is reached, so their Esc
            // lives in `app/dispatch/input.rs` — `claim_command_palette`
            // and `claim_keymap_recorder`, which resolve against live
            // `ui_state` and never reach this table (#557). The hover
            // tooltip is not dismissible at all.
            OverlayId::CommandPalette | OverlayId::HoverTooltip => None,
            OverlayId::LibraryUpdates => self.library_updates_open.then(|| {
                Message::Library(crate::library::messages::LibraryMessage::LibraryUpdatesCancel)
            }),
        }
    }
}

/// Which overlay slot a detachable modal occupies in the main window
/// (#547).
///
/// The pairing exists so an Esc typed in a detached window can be
/// answered from the SAME rung table the in-window card resolves
/// against, instead of a second list of Close messages that would drift
/// out of step with it. Exhaustive over `ModalId`, so a new detachable
/// modal is a compile error here rather than a silent fall-through to
/// the main window's tool reset.
///
/// `None` is not an omission: `MoveSelection`, `NetColorPalette` and
/// `ParameterManager` have no `OverlayId` and no `OpenOverlays` field —
/// they are part of the `every_modal_claims_escape` gap, and Esc does
/// nothing over their in-window cards either.
pub(crate) fn modal_overlay_id(modal: crate::app::state::ModalId) -> Option<OverlayId> {
    use crate::app::state::ModalId;
    match modal {
        ModalId::AnnotateDialog => Some(OverlayId::AnnotateDialog),
        ModalId::AnnotateResetConfirm => Some(OverlayId::AnnotateResetConfirm),
        ModalId::ReprefixConfirm => Some(OverlayId::ReprefixConfirm),
        ModalId::ErcDialog => Some(OverlayId::ErcDialog),
        ModalId::Preferences => Some(OverlayId::Preferences),
        ModalId::FindReplace => Some(OverlayId::FindReplace),
        ModalId::RenameDialog => Some(OverlayId::RenameDialog),
        ModalId::RemoveDialog => Some(OverlayId::RemoveDialog),
        ModalId::PrintPreview => Some(OverlayId::PrintPreview),
        ModalId::BomPreview => Some(OverlayId::BomPreview),
        ModalId::EnableVersionControl => Some(OverlayId::EnableVersionControl),
        ModalId::GridProperties => Some(OverlayId::GridProperties),
        ModalId::SelectionFilterCustom => Some(OverlayId::SelectionFilterCustom),
        ModalId::About => Some(OverlayId::About),
        ModalId::KeyboardShortcuts => Some(OverlayId::KeyboardShortcuts),
        ModalId::PassiveCalculator => Some(OverlayId::PassiveCalculator),
        ModalId::FirstRunTour => Some(OverlayId::FirstRunTour),
        ModalId::CreateChildSheet => Some(OverlayId::CreateChildSheetConfirm),
        ModalId::ErrorNotice => Some(OverlayId::ErrorNotice),
        ModalId::NetlistIncompletePrompt => Some(OverlayId::NetlistIncompletePrompt),
        ModalId::AutosaveRecovery => Some(OverlayId::AutosaveRecovery),
        ModalId::ProjectCloseConfirm => Some(OverlayId::ProjectCloseConfirm),
        ModalId::AppQuitConfirm => Some(OverlayId::AppQuitConfirm),
        ModalId::LibraryPicker => Some(OverlayId::LibraryPicker),
        ModalId::LibraryEditRow => Some(OverlayId::EditRowModal),
        ModalId::LibraryPrimitivePicker => Some(OverlayId::PrimitivePicker),
        ModalId::LibraryDocumentOptions => Some(OverlayId::DocumentOptions),
        ModalId::LibraryCreateOptions => Some(OverlayId::CreateOptions),
        ModalId::LibraryUpdates => Some(OverlayId::LibraryUpdates),
        ModalId::LibraryCloseConfirm => Some(OverlayId::CloseLibraryConfirm),
        ModalId::LibraryDeleteConfirm => Some(OverlayId::DeleteConfirm),
        ModalId::LibraryRecovery => Some(OverlayId::LibraryRecovery),
        ModalId::NetColorCustom => Some(OverlayId::NetColorCustom),
        ModalId::MoveSelection | ModalId::NetColorPalette | ModalId::ParameterManager => None,
    }
}

impl Signex {
    /// Resolve the Esc ladder against live state, or `None` when no
    /// overlay claims the key.
    ///
    /// The single entry point for the `Message::EscapePressed` handler
    /// (#535). `OpenOverlays` stays private to this module: the ladder is
    /// one concern in one file, and callers only ever want the answer.
    pub(crate) fn escape_overlay_message(&self) -> Option<Message> {
        self.open_overlays(None).escape_message()
    }

    /// What Esc does inside `modal`'s own detached window: exactly what
    /// Esc over that modal's in-window card would do, and nothing else
    /// (#547).
    ///
    /// The ladder is deliberately NOT walked here. It ranks overlays by
    /// where they paint **in the main window**, and none of that stack
    /// covers a separate OS window — running it would let a dialog in
    /// another window outrank the one the user is actually typing into.
    /// So the answer is a single rung, looked up by id.
    ///
    /// `modal`'s own detachment is passed to `open_overlays` so its flag
    /// reads open: the flag is normally forced false *because* the
    /// in-window card is not painted, which is the right answer for an
    /// Esc in the main window and the wrong one here.
    ///
    /// `None` means that modal has no Esc at all — either no `OverlayId`
    /// (Move Selection, the net-colour palette, the parameter manager)
    /// or an `OverlayId` whose rung is `None` by design. Its in-window
    /// card ignores Esc too, so this stays consistent with it rather than
    /// inventing a dismissal only the detached form has.
    pub(crate) fn detached_modal_escape_message(
        &self,
        modal: crate::app::state::ModalId,
    ) -> Option<Message> {
        let id = modal_overlay_id(modal)?;
        self.open_overlays(Some(modal)).rung(id)
    }

    /// The one decision "a confirm or warning dialog owns the user's
    /// attention": the system menu bar greys its model rows and refuses them
    /// while it holds. Read from the same snapshot the Esc ladder uses, so
    /// both see the same open dialogs (docs/MENU_BAR.md, "Native macOS menu").
    ///
    /// The annotate-reset confirm is the one locking dialog the snapshot
    /// hides while detached; naming it as the Esc source keeps it counted.
    pub(crate) fn has_locking_confirm_modal(&self) -> bool {
        self.open_overlays(Some(crate::app::state::ModalId::AnnotateResetConfirm))
            .has_locking_confirm()
    }

    /// Snapshot the open overlays for Esc resolution, from live state.
    ///
    /// Called from the `Message::EscapePressed` handler in
    /// `app/dispatch/mod.rs`, not from the subscription (#535). Building it
    /// here rather than inside `Subscription::with` is what lets a rung
    /// carry owned data (see `OpenOverlays::delete_confirm`) and what
    /// removes the one-update staleness the subscription snapshot had.
    ///
    /// `esc_from_detached` names the modal whose own detached window the
    /// Esc came from, if any — see `detached_modal_escape_message`. Every
    /// other caller passes `None`.
    fn open_overlays(&self, esc_from_detached: Option<crate::app::state::ModalId>) -> OpenOverlays {
        // True while `modal` is showing in its own detached OS window
        // rather than as an in-window overlay. Several builders skip
        // painting the in-window card in that case (`view/overlays/bars.rs`,
        // `view/overlays/modals.rs`) — the Esc ladder must skip the
        // corresponding rung for the same reason, or Esc would dismiss a
        // dialog the user can't even see, while the real (detached) window
        // stays open untouched.
        //
        // Unless the Esc came from that very window, which is the one
        // case where the user IS looking at the card (#547).
        let modal_detached = |modal: crate::app::state::ModalId| -> bool {
            esc_from_detached != Some(modal)
                && self.ui_state.windows.values().any(|kind| {
                    matches!(kind, crate::app::state::WindowKind::DetachedModal(m) if *m == modal)
                })
        };

        OpenOverlays {
            find_replace_open: self.ui_state.find_replace.open
                && !modal_detached(crate::app::state::ModalId::FindReplace),
            kbd_shortcuts_open: self.ui_state.keyboard_shortcuts_open
                && !modal_detached(crate::app::state::ModalId::KeyboardShortcuts),
            about_open: self.ui_state.about_open
                && !modal_detached(crate::app::state::ModalId::About),
            first_run_tour_open: self.ui_state.first_run_tour_open
                && !modal_detached(crate::app::state::ModalId::FirstRunTour),
            autosave_recovery_open: self.document_state.autosave.current_offer().is_some(),
            prefs_open: self.ui_state.preferences_open
                && !modal_detached(crate::app::state::ModalId::Preferences),
            annotate_open: self.ui_state.annotate_dialog_open
                && !modal_detached(crate::app::state::ModalId::AnnotateDialog),
            erc_open: self.ui_state.erc_dialog_open
                && !modal_detached(crate::app::state::ModalId::ErcDialog),
            erc_save_profile_open: self.ui_state.save_profile_dialog.is_some(),
            rename_open: self.ui_state.rename_dialog.is_some(),
            remove_open: self.ui_state.remove_dialog.is_some(),
            enable_vc_open: self.ui_state.enable_version_control.is_some()
                && !modal_detached(crate::app::state::ModalId::EnableVersionControl),
            library_create_options_open: self.library.create_options.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryCreateOptions),
            passive_calculator_open: self.ui_state.passive_calculator_open
                && !modal_detached(crate::app::state::ModalId::PassiveCalculator),
            annotate_reset_confirm_open: self.ui_state.annotate_reset_confirm
                && !modal_detached(crate::app::state::ModalId::AnnotateResetConfirm),
            reprefix_confirm_open: self.ui_state.reprefix_confirm.is_some()
                && !modal_detached(crate::app::state::ModalId::ReprefixConfirm),
            app_quit_confirm_open: self.ui_state.app_quit_confirm.is_some(),
            project_close_confirm_open: self.ui_state.project_close_confirm.is_some(),
            create_child_sheet_confirm_open: self.ui_state.create_child_sheet_confirm.is_some(),
            grid_properties_open: self.ui_state.grid_properties.is_some()
                && !modal_detached(crate::app::state::ModalId::GridProperties),
            selection_filter_custom_open: self.ui_state.selection_filter_custom.is_some()
                && !modal_detached(crate::app::state::ModalId::SelectionFilterCustom),
            library_picker_open: self.library.picker.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryPicker),
            library_document_options_open: self.library.document_options.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryDocumentOptions),
            library_updates_open: self.library.library_updates.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryUpdates),
            library_primitive_picker_open: self.library.primitive_picker.is_some()
                && !modal_detached(crate::app::state::ModalId::LibraryPrimitivePicker),
            close_library_confirm_open: self.library.close_library_confirm.is_some(),
            recovery_kind: self
                .library
                .recovery
                .as_ref()
                .map(|recovery| match recovery {
                    crate::library::recovery::RecoveryDialog::LibraryMissing { .. } => {
                        RecoveryKind::LibraryMissing
                    }
                    crate::library::recovery::RecoveryDialog::GitMissing { .. } => {
                        RecoveryKind::GitMissing
                    }
                    crate::library::recovery::RecoveryDialog::BrokenPrimitiveBinding { .. } => {
                        RecoveryKind::BrokenBinding
                    }
                }),
            error_notice_open: self.document_state.error_notice.is_some(),
            error_notice_detached: modal_detached(crate::app::state::ModalId::ErrorNotice),
            netlist_incomplete_prompt_open: self.document_state.netlist_incomplete_prompt.is_some(),
            netlist_prompt_detached: modal_detached(
                crate::app::state::ModalId::NetlistIncompletePrompt,
            ),
            print_preview_open: self.document_state.preview.is_some()
                && !modal_detached(crate::app::state::ModalId::PrintPreview),
            bom_preview_open: self.document_state.bom_preview.is_some()
                && !modal_detached(crate::app::state::ModalId::BomPreview),
            net_color_custom_open: self.ui_state.net_color_custom.show
                && !modal_detached(crate::app::state::ModalId::NetColorCustom),
            // MUST mirror `delete_confirm_overlay`'s pick
            // (`view/overlays/modals.rs:291-316`): it iterates
            // `library_browsers` and returns on the FIRST entry with a
            // confirm open, so Esc has to cancel that same one or it
            // dismisses a card the user is not looking at.
            //
            // Two browser tabs can hold a confirm at once, and which of
            // them paints is then decided by `HashMap` iteration order.
            // That ambiguity predates this change and is not fixed here —
            // mirroring the builder keeps Esc and paint agreeing on
            // whichever one wins.
            delete_confirm: self
                .library
                .library_browsers
                .iter()
                .find(|(_, browser)| browser.delete_confirm.is_some())
                .map(|(path, _)| path.clone()),
            // MUST mirror `edit_row_modal_overlay`'s pick: first browser
            // with an open edit modal.
            edit_row_modal: self
                .library
                .library_browsers
                .iter()
                .find(|(_, browser)| browser.edit_modal.is_some())
                .map(|(path, _)| path.clone()),
        }
    }

    /// True while any drag that a global mouse-move / button-release must
    /// drive is in flight. The subscription listens for `DragEnd` only
    /// while this holds, so every drag that a release has to end has to be
    /// counted here — a release outside the widget that started it is
    /// otherwise never delivered.
    pub fn is_pointer_drag_active(&self) -> bool {
        self.interaction_state.dragging.is_some()
            || self.document_state.dock.tab_drag.is_some()
            || self.ui_state.modal_dragging.is_some()
            || self.ui_state.tab_dragging.is_some()
            || self
                .document_state
                .dock
                .floating
                .iter()
                .any(|fp| fp.dragging)
            // A BOM column resize, docked or detached: its handle is 4 px
            // wide, so the release usually lands elsewhere and only the
            // global `DragEnd` can end it.
            || self
                .document_state
                .bom_preview
                .as_ref()
                .is_some_and(|preview| preview.column_resize.is_some())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // A pure transport, nothing more (#557 phase 1). It filters and
        // forwards; which app-level consumer owns the key is decided in
        // `app/dispatch/input.rs` against live state.
        //
        // `keyboard::listen()` cannot be the source: it drops the window
        // id, because its filter matches `subscription::Event::Interaction
        // { .. }` and the `..` swallows `window`
        // (`iced_futures-0.14.0/src/keyboard.rs`). `event::listen_with`
        // hands the id to its filter (`iced_futures-0.14.0/src/event.rs:26`);
        // the predicate here is otherwise the one `keyboard::listen()`
        // applies — keyboard events, `Status::Ignored`.
        //
        // No `.with(...)` and no `.map(...)`: the message is built in the
        // filter itself. That matters twice over. `listen_with` takes a
        // plain `fn`, and this closure captures nothing, so it coerces.
        // And `Subscription::with` folds its value into the subscription's
        // IDENTITY hash — the old `KeyContext` meant every command-palette
        // / recorder / shortcuts open-close tore down and respawned the
        // keyboard stream. This subscription's identity is now constant
        // for the process lifetime.
        let kbd = iced::event::listen_with(|event, status, window| match (event, status) {
            (iced::Event::Keyboard(event), iced::event::Status::Ignored) => {
                Some(Message::Input(InputMsg::KeyInput { window, event }))
            }
            _ => None,
        });

        // Mouse events for drag-to-resize/floating-drag.
        // Dragging subscribes to cursor moves directly; the idle branch
        // below still forwards CursorMoved (to keep `last_mouse_pos`
        // fresh) but drops everything else it does not need.
        let drag_active = self.is_pointer_drag_active();
        let modal_drag_active = self.ui_state.modal_dragging.is_some();
        let mouse_sub = if modal_drag_active {
            // Modal drag takes priority — release ends the modal drag
            // specifically (not the generic DragEnd).
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Message::Overlay(OverlayMsg::ModalDragEnd),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::noop(),
            })
        } else if drag_active {
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Message::Ui(UiMsg::DragEnd),
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Message::ContextMenu(ContextMenuMsg::Close),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::noop(),
            })
        } else {
            // Always track the cursor so `last_mouse_pos` is fresh when the
            // user starts a modal drag — otherwise the first delta is huge
            // and the dialog jumps. DragMove is a no-op when no drag is
            // active (it just updates last_mouse_pos).
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Message::ContextMenu(ContextMenuMsg::Close),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::noop(),
            })
        };
        // Window-close events from winit: routed so Phase 2/3 can drop
        // detached-modal / undocked-tab entries from ui_state.windows.
        let window_close = iced::window::close_events()
            .map(|id| Message::Window(WindowMsg::SecondaryWindowClosed(id)));
        // OS close requests (native close button, Alt+F4, taskbar close).
        // These only reach the app because every window is opened with
        // `exit_on_close_request: false`; otherwise winit closes the window
        // itself. The main window goes through the unsaved-changes guard,
        // any other window closes.
        let window_close_request = iced::window::close_requests()
            .map(|id| Message::Window(WindowMsg::WindowCloseRequested(id)));
        // Window-resize subscription. `iced::event::listen()`'s
        // Window::Resized event doesn't fire on the very first frame —
        // subscribing to `window::resize_events()` directly gets the
        // initial physical size so dropdowns position correctly without
        // a manual resize.
        // Fire a WindowResizedFor for every OS resize event, carrying
        // the window id so the dispatcher can ignore resizes of
        // detached modal / undocked-tab windows. A plain WindowResized
        // without the id would clobber `ui_state.window_size` with
        // e.g. the 420x240 size of the Move dialog, which then shifts
        // the Active-Bar dropdowns on the main window.
        let window_resize = iced::window::resize_events().map(|(id, size)| {
            Message::Window(WindowMsg::WindowResizedFor(id, size.width, size.height))
        });

        // Hover-open timer for the right-click context-menu submenus.
        // Active while ANY menu that owns submenus is open — canvas
        // right-click, project-tree right-click, or document-tab
        // right-click. The dispatcher checks `pending_submenu`'s
        // elapsed time on each tick.
        let any_menu_open = self.interaction_state.context_menu.is_some()
            || self.interaction_state.project_tree_context_menu.is_some()
            || self.interaction_state.tab_context_menu.is_some();
        let hover_tick = if any_menu_open {
            iced::time::every(std::time::Duration::from_millis(50))
                .map(|_| Message::ContextMenu(ContextMenuMsg::SubmenuTickHover))
        } else {
            Subscription::none()
        };

        // Hover-tooltip wake tick. The tooltip overlay only shows
        // after the cursor has dwelled on a placed symbol for 250 ms
        // — without a periodic re-render, the view layer would never
        // notice the threshold crossing once the cursor stopped
        // moving (no mouse events → no redraw). This fires until the
        // user moves off the symbol; once the tooltip is up, normal
        // CursorMoved events keep it tracking the cursor.
        let symbol_hover_active = self.interaction_state.hover_symbol_uuid.is_some()
            && self
                .interaction_state
                .hover_started_at
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(900));
        let hover_tooltip_tick = if symbol_hover_active {
            iced::time::every(std::time::Duration::from_millis(80)).map(|_| Message::noop())
        } else {
            Subscription::none()
        };

        // External `.snxlib` edits (#531): a slow timer whose handler stats
        // the mounted files off-thread; `view` only reads the caches.
        let library_poll = crate::library::external_poll::subscription(
            &self.library,
            self.document_state.project_fs.is_watching(),
        )
        .map(Message::Library);

        // Opt-in autosave timer (#104); empty while the preference is off.
        let autosave_tick = crate::autosave::subscription(
            self.ui_state.autosave_enabled,
            self.document_state.autosave.has_owed_moves(),
        )
        .map(Message::Autosave);

        // #1378 - a pending chord is abandoned by a pointer press or a focus
        // change, and a waiting one fires on timeout. Both subscriptions
        // exist only while there is something to watch, so idle costs
        // nothing; `.with(generation)` gives each wait its own timer.
        let chord_cancel = if self.ui_state.keymap_pending_sequence.is_empty() {
            Subscription::none()
        } else {
            iced::event::listen_with(|event, _status, _window| {
                crate::app::dispatch::chord::chord_cancelling_event(&event)
                    .then(|| Message::Ui(UiMsg::KeymapChordCancelled))
            })
        };
        let chord_timeout = match self.ui_state.keymap_chord_wait {
            Some(wait) => iced::time::every(crate::app::dispatch::chord::CHORD_TIMEOUT)
                .with(wait.generation)
                .map(|(generation, _)| Message::Ui(UiMsg::KeymapChordTimedOut { generation })),
            None => Subscription::none(),
        };

        Subscription::batch([
            window_subscriptions::window_focus(),
            window_subscriptions::native_menu_activations(),
            chord_cancel,
            chord_timeout,
            kbd,
            mouse_sub,
            window_close,
            window_close_request,
            window_resize,
            hover_tick,
            hover_tooltip_tick,
            library_poll,
            autosave_tick,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set one flag on an otherwise-closed set and resolve the Esc ladder.
    fn only(set: impl FnOnce(&mut OpenOverlays)) -> Option<Message> {
        let mut overlays = OpenOverlays::default();
        set(&mut overlays);
        overlays.escape_message()
    }

    #[test]
    fn nothing_open_falls_through_to_the_tool_reset() {
        assert!(
            OpenOverlays::default().escape_message().is_none(),
            "with no modal open Esc must reach `Message::EscapePressed`"
        );
    }

    /// The hand-written ladder closed its blocking-modal guard with a
    /// `debug_assert!` that at least one of the four members really was
    /// set once the guard had been entered. Deriving the walk from
    /// `PAINT_ORDER` deletes the guard, so the invariant it protected
    /// gets a test instead: in the blocking regime Esc must always be
    /// claimed by one of the visible five and never fall through to the
    /// tool reset.
    #[test]
    fn a_blocking_modal_always_claims_escape_itself() {
        // Each member alone, then with a suppressed dialog set behind
        // it. The second case is the one that matters: Alt+F4 and keymap
        // strokes are not modal-gated, so a rung below the cutoff really
        // can be open while only the blocking card paints.
        let claims_escape = |name: &str, mut overlays: OpenOverlays| {
            for behind in [false, true] {
                overlays.grid_properties_open = behind;
                assert!(
                    overlays.escape_message().is_some(),
                    "{name} owns the stack but let Esc fall through \
                     (grid_properties open behind it: {behind})"
                );
            }
        };
        claims_escape(
            "error_notice",
            OpenOverlays {
                error_notice_open: true,
                ..OpenOverlays::default()
            },
        );
        claims_escape(
            "netlist_incomplete_prompt",
            OpenOverlays {
                netlist_incomplete_prompt_open: true,
                ..OpenOverlays::default()
            },
        );
        claims_escape(
            "print_preview",
            OpenOverlays {
                print_preview_open: true,
                ..OpenOverlays::default()
            },
        );
        claims_escape(
            "net_color_custom",
            OpenOverlays {
                net_color_custom_open: true,
                ..OpenOverlays::default()
            },
        );
    }

    // ── #547 / #554: which window the Esc came from ─────────────────
    //
    // The two tests below are the ladder's half of that question — the
    // rung table and the detachment filters. The behavioural half, which
    // drives `Signex::update` per window kind, lives beside the code it
    // exercises in `app/dispatch/escape.rs`.

    #[test]
    fn every_detachment_filtered_modal_can_still_answer_its_own_escape() {
        // `open_overlays` forces these six rungs closed while the modal is
        // detached, precisely because the in-window card stops painting.
        // That is only safe if the detached window can answer the key
        // itself — otherwise Esc has nowhere left to go and the modal
        // becomes undismissable by keyboard. Adding a seventh filter
        // without a `modal_overlay_id` entry fails here.
        use crate::app::state::ModalId;
        for modal in [
            ModalId::Preferences,
            ModalId::AnnotateDialog,
            ModalId::ErcDialog,
            ModalId::AnnotateResetConfirm,
            ModalId::ReprefixConfirm,
            ModalId::PrintPreview,
            ModalId::BomPreview,
        ] {
            assert!(
                modal_overlay_id(modal).is_some(),
                "{modal:?} is detachment-filtered in `open_overlays` but has no \
                 overlay slot, so Esc in its own window would do nothing"
            );
        }
    }

    #[test]
    fn a_detached_modal_answers_escape_with_the_same_message_its_card_would() {
        // The whole point of routing through `modal_overlay_id` + `rung`
        // rather than a second table of Close messages: the detached
        // window and the in-window card cannot drift apart.
        let (mut app, _boot) = Signex::new();
        app.ui_state.erc_dialog_open = true;

        let card = only(|o| o.erc_open = true);
        let detached = {
            app.ui_state.windows.insert(
                iced::window::Id::unique(),
                crate::app::state::WindowKind::DetachedModal(crate::app::state::ModalId::ErcDialog),
            );
            app.detached_modal_escape_message(crate::app::state::ModalId::ErcDialog)
        };

        assert!(
            matches!(card, Some(Message::Erc(ErcMsg::CloseDialog))),
            "in-window ERC card's Esc changed — update this test's expectation"
        );
        assert!(
            matches!(detached, Some(Message::Erc(ErcMsg::CloseDialog))),
            "detached ERC window's Esc must send what the in-window card sends"
        );
    }

    /// #511 — the passive calculator shipped in #490 with no rung here, so
    /// Esc left the modal open AND fell through to the tool reset, silently
    /// reverting the active canvas tool behind it.
    #[test]
    fn passive_calculator_claims_escape() {
        assert!(matches!(
            only(|o| o.passive_calculator_open = true),
            Some(Message::Overlay(OverlayMsg::ClosePassiveCalculator))
        ));
    }

    /// Every *field of `OpenOverlays`* claims Esc with its own Cancel
    /// message — NOT "every modal in the app". That gap is real and is
    /// exactly what let #511 (the passive calculator) and #514 (~14 more
    /// modals) ship with no rung: a modal only gets covered here once
    /// someone remembers to add it as a field and a rung, and nothing
    /// forces that. Documented modals that are deliberately NOT fields —
    /// so NOT covered by this test — are: the command palette and the
    /// keymap chord recorder (`palette_and_keymap_recorder_are_not_on_the_ladder`,
    /// both swallow input earlier). The per-library rungs (`delete_confirm`,
    /// `edit_row_modal`) carry an owned `library_path` and are covered by
    /// `every_ladder_field_claims_escape` and the ranking tests instead.
    #[test]
    fn every_modal_claims_escape() {
        assert!(matches!(
            only(|o| o.find_replace_open = true),
            Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close
            ))
        ));
        assert!(matches!(
            only(|o| o.kbd_shortcuts_open = true),
            Some(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts))
        ));
        assert!(matches!(
            only(|o| o.first_run_tour_open = true),
            Some(Message::Overlay(OverlayMsg::DismissFirstRunTour))
        ));
        assert!(matches!(
            only(|o| o.autosave_recovery_open = true),
            Some(Message::Autosave(crate::autosave::AutosaveMsg::Recovery(
                crate::autosave::RecoveryChoice::Later
            )))
        ));
        assert!(matches!(
            only(|o| o.erc_open = true),
            Some(Message::Erc(ErcMsg::CloseDialog))
        ));
        assert!(matches!(
            only(|o| o.annotate_open = true),
            Some(Message::Annotate(AnnotateMsg::CloseDialog))
        ));
        assert!(matches!(
            only(|o| o.prefs_open = true),
            Some(Message::Preferences(PreferencesMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.rename_open = true),
            Some(Message::Rename(RenameMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.remove_open = true),
            Some(Message::Remove(RemoveMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.enable_vc_open = true),
            Some(Message::EnableVersionControl(
                EnableVersionControlMsg::Close
            ))
        ));
        assert!(matches!(
            only(|o| o.library_create_options_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::LibraryCreateOptionsCancel
            ))
        ));
        // #514 additions below.
        assert!(matches!(
            only(|o| o.annotate_reset_confirm_open = true),
            Some(Message::Annotate(AnnotateMsg::CloseResetConfirm))
        ));
        assert!(matches!(
            only(|o| o.reprefix_confirm_open = true),
            Some(Message::Annotate(AnnotateMsg::ReprefixCancelled))
        ));
        assert!(matches!(
            only(|o| o.app_quit_confirm_open = true),
            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
        assert!(matches!(
            only(|o| o.project_close_confirm_open = true),
            Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
        assert!(matches!(
            only(|o| o.grid_properties_open = true),
            Some(Message::GridProperties(GridPropertiesMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.selection_filter_custom_open = true),
            Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom))
        ));
        assert!(matches!(
            only(|o| o.library_document_options_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::DocumentOptionsCancel
            ))
        ));
        assert!(matches!(
            only(|o| o.library_primitive_picker_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::PrimitivePicker(
                    crate::library::messages::PrimitivePickerMsg::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.library_updates_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::LibraryUpdatesCancel
            ))
        ));
        assert!(matches!(
            only(|o| o.close_library_confirm_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::CloseLibraryConfirm(
                    crate::library::messages::CloseLibraryChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.library_picker_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::ClosePicker
            ))
        ));
        assert!(matches!(
            only(|o| o.recovery_kind = Some(RecoveryKind::LibraryMissing)),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::RecoveryLibraryMissing(
                    crate::library::recovery::LibraryMissingChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.recovery_kind = Some(RecoveryKind::GitMissing)),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::RecoveryGitMissing(
                    crate::library::recovery::GitMissingChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.recovery_kind = Some(RecoveryKind::BrokenBinding)),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::RecoveryBrokenBinding(
                    crate::library::recovery::BrokenBindingChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.error_notice_open = true),
            Some(Message::Overlay(OverlayMsg::DismissErrorNotice))
        ));
        assert!(matches!(
            only(|o| o.netlist_incomplete_prompt_open = true),
            Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
        ));
        assert!(matches!(
            only(|o| o.print_preview_open = true),
            Some(Message::PrintPreview(PrintPreviewMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.bom_preview_open = true),
            Some(Message::BomPreview(BomPreviewMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.net_color_custom_open = true),
            Some(Message::NetColor(NetColorMsg::CustomShow(false)))
        ));
    }

    /// Order is load-bearing — the deepest (topmost-painted) modal wins.
    /// `passive_calculator_overlay` paints at `view/mod.rs:789`,
    /// `find_replace_overlay` at `:787` (earlier, i.e. underneath) and
    /// `preferences_overlay` at `:786` (earliest of the three) — so with
    /// all three flags set, the passive calculator must win. This is a
    /// KNOWN reversal from trunk (where `find_replace_open` used to win):
    /// that was a latent ordering bug — find_replace outranked a modal
    /// that visually painted on top of it — fixed as a side effect of
    /// deriving the whole ladder from paint order instead of hand-ordering
    /// it (see the doc comment on `escape_message`).
    /// #535 — the rung that could not exist before the ladder moved to
    /// `update`, pinned in both directions.
    ///
    /// Membership is already forced by `every_ladder_field_claims_escape`;
    /// this is about *position*. `delete_confirm_overlay` paints between
    /// `edit_row_modal_overlay` and `primitive_picker_overlay`, so the rung
    /// must lose to the primitive picker and beat the library picker. Get
    /// this backwards and Esc dismisses a card underneath the one the user
    /// is looking at.
    #[test]
    fn delete_confirm_ranks_below_the_primitive_picker_and_above_the_library_picker() {
        let overlays = OpenOverlays {
            delete_confirm: Some(std::path::PathBuf::from("parts.snxlib")),
            library_primitive_picker_open: true,
            ..OpenOverlays::default()
        };
        assert!(
            matches!(
                overlays.escape_message(),
                Some(Message::Library(
                    crate::library::messages::LibraryMessage::PrimitivePicker(_)
                ))
            ),
            "the primitive picker paints above the delete confirm, so it must win"
        );

        let overlays = OpenOverlays {
            delete_confirm: Some(std::path::PathBuf::from("parts.snxlib")),
            library_picker_open: true,
            ..OpenOverlays::default()
        };
        match overlays.escape_message() {
            Some(Message::Library(
                crate::library::messages::LibraryMessage::BrowserDeleteRowCancel { library_path },
            )) => assert_eq!(
                library_path,
                std::path::PathBuf::from("parts.snxlib"),
                "the Cancel must be addressed to the library whose confirm is open — \
                 carrying that path is the entire reason this rung needs live state"
            ),
            other => panic!(
                "delete confirm paints above the library picker, so it must win; got {other:?}"
            ),
        }
    }

    #[test]
    fn the_deepest_modal_wins() {
        let overlays = OpenOverlays {
            find_replace_open: true,
            prefs_open: true,
            passive_calculator_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Overlay(OverlayMsg::ClosePassiveCalculator))
        ));
    }

    /// #514 — the reset confirm is a child of the annotate dialog with
    /// its own distinct Cancel message; getting the ordering backwards
    /// would resolve Esc to `CloseDialog` and close the parent out from
    /// under the confirm.
    #[test]
    fn annotate_reset_confirm_outranks_annotate_dialog() {
        let overlays = OpenOverlays {
            annotate_open: true,
            annotate_reset_confirm_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Annotate(AnnotateMsg::CloseResetConfirm))
        ));
    }

    /// The app-quit gate can be triggered while a shallower dialog (e.g.
    /// Preferences) is still open; the gate the user just triggered must
    /// win, not the dialog underneath it.
    #[test]
    fn quit_gate_outranks_a_dialog_layered_under_it() {
        let overlays = OpenOverlays {
            app_quit_confirm_open: true,
            prefs_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
    }

    /// #514 round 4 — refutes a round-3 claim. `app_quit_confirm_open`
    /// and a `has_blocking_modal` member CAN both be `true` at once: Alt+F4
    /// / native window close sets `app_quit_confirm` unconditionally
    /// (`handle_app_quit_requested`, no `has_blocking_modal` check on that
    /// path anywhere) — so an export failure followed by Alt+F4 leaves
    /// both set while only the blocking modal's card paints. The guard at
    /// the top of `escape_message` is what keeps Esc resolving to the
    /// visible card in that reachable state, not the invisible quit gate;
    /// this test exercises all four members of the group against it.
    #[test]
    fn blocking_modal_outranks_a_quit_gate_set_behind_it() {
        assert!(matches!(
            OpenOverlays {
                error_notice_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::Overlay(OverlayMsg::DismissErrorNotice))
        ));
        assert!(matches!(
            OpenOverlays {
                netlist_incomplete_prompt_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
        ));
        assert!(matches!(
            OpenOverlays {
                print_preview_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::PrintPreview(PrintPreviewMsg::Close))
        ));
        assert!(matches!(
            OpenOverlays {
                net_color_custom_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::NetColor(NetColorMsg::CustomShow(false)))
        ));
    }

    /// The concrete reachable repro from round 4's review: export fails
    /// (`error_notice_open` set, only the error card paints, per
    /// `has_blocking_modal`'s early return), then the user hits Alt+F4 /
    /// the OS close button with dirty documents, which sets
    /// `app_quit_confirm_open` with no regard for what's currently
    /// painted. Esc must dismiss the card the user can actually see, not
    /// silently cancel a quit gate they never saw — leaving them stuck
    /// staring at an export-error card that Esc appears to do nothing to.
    #[test]
    fn alt_f4_behind_an_error_notice_card_does_not_steal_escape() {
        let overlays = OpenOverlays {
            error_notice_open: true,
            app_quit_confirm_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Overlay(OverlayMsg::DismissErrorNotice))
        ));
    }

    /// Pin the `has_blocking_modal` group's own internal order, derived
    /// from `collect_overlays:750-754` reversed: net-colour > bom-preview >
    /// print-preview > netlist-incomplete-prompt > export-error.
    /// `bom_preview_open` sits inside this chain even though it isn't a
    /// `has_blocking_modal` member — it paints in the same early block, one
    /// slot below `net_color_custom_open` and one above `print_preview_open`.
    #[test]
    fn blocking_modal_group_internal_order_matches_derived_position() {
        assert!(matches!(
            OpenOverlays {
                net_color_custom_open: true,
                bom_preview_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::NetColor(NetColorMsg::CustomShow(false)))
        ));
        assert!(matches!(
            OpenOverlays {
                bom_preview_open: true,
                print_preview_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::BomPreview(BomPreviewMsg::Close))
        ));
        assert!(matches!(
            OpenOverlays {
                print_preview_open: true,
                netlist_incomplete_prompt_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::PrintPreview(PrintPreviewMsg::Close))
        ));
        assert!(matches!(
            OpenOverlays {
                netlist_incomplete_prompt_open: true,
                error_notice_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
        ));
    }

    /// Round-2 regression #1: `enable_vc_open` was hand-placed above
    /// `erc_open`, but `erc_dialog_open` paints at `view/mod.rs:792`
    /// (`detachable_dialogs_overlay`, pushed last inside it), strictly
    /// later than `enable_vc_open`'s home in `simple_dialogs_overlay`
    /// (`:791`) — so ERC must outrank Enable Version Control.
    #[test]
    fn erc_dialog_outranks_enable_version_control() {
        let overlays = OpenOverlays {
            erc_open: true,
            enable_vc_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Erc(ErcMsg::CloseDialog))
        ));
    }

    /// Round-2 regression #2: `enable_vc_open` was also hand-placed above
    /// `grid_properties_open`, but inside `simple_dialogs_overlay`
    /// (`modals.rs:118-141`) `grid_properties` is pushed AFTER `enable_vc`
    /// (`:133` then `:136`) — later push = paints on top — so Grid
    /// Properties must outrank Enable Version Control, not the reverse.
    #[test]
    fn grid_properties_outranks_enable_version_control() {
        let overlays = OpenOverlays {
            grid_properties_open: true,
            enable_vc_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::GridProperties(GridPropertiesMsg::Close))
        ));
    }

    /// `bom_preview_open` paints in `collect_overlays`' earliest block
    /// (`:753`) with no exclusivity guarantee of its own, so anything
    /// pushed later — Find & Replace (`:787`) included — paints over it
    /// and must win Esc too.
    #[test]
    fn bom_preview_loses_to_a_rung_that_paints_above_it() {
        let overlays = OpenOverlays {
            bom_preview_open: true,
            find_replace_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close
            ))
        ));
    }

    /// The Document Options / Primitive Picker / Library Updates /
    /// Close-Library-confirm modals can all be reached from a Library
    /// Browser tab while the picker is still the shallower host surface
    /// behind them — the picker must not steal Esc from any of them.
    #[test]
    fn library_document_options_outranks_library_picker() {
        let overlays = OpenOverlays {
            library_picker_open: true,
            library_document_options_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::DocumentOptionsCancel
            ))
        ));
    }

    /// The gates below, layered over a shallower dialog pushed after them
    /// in `simple_dialogs_overlay`, must still lose to it — Grid
    /// Properties (Ctrl+G) opened, then an unsaved-edits app-quit request
    /// (chrome ✕ / File ▸ Exit / Alt+F4) layers behind it; Esc must close
    /// the card the user can actually see.
    #[test]
    fn a_dialog_pushed_after_the_quit_gate_outranks_it() {
        let overlays = OpenOverlays {
            app_quit_confirm_open: true,
            grid_properties_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::GridProperties(GridPropertiesMsg::Close))
        ));
    }

    /// #514-followup — `bom_preview_open` is NOT a `has_blocking_modal`
    /// member (only `error_notice` / `netlist_incomplete_prompt` /
    /// `preview` / `net_color_custom.show` are), so it must not steal Esc
    /// from a quit/close gate the way the four true blocking modals do.
    #[test]
    fn quit_gate_outranks_bom_preview_because_it_is_not_actually_blocking() {
        let overlays = OpenOverlays {
            app_quit_confirm_open: true,
            bom_preview_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
    }

    /// #514 — the actual reported symptom, generalized: Esc must never
    /// reach `Message::EscapePressed` (the tool reset) while a modal
    /// represented on the ladder is open.
    ///
    /// What this test mechanically guarantees, precisely (no more, no
    /// less):
    ///
    /// 1. **Every field on `OpenOverlays`, of any type, is accounted for.**
    ///    `open_overlays_field_names!`'s identifier-list argument is
    ///    destructured against `OpenOverlays::default()` with no `..` rest
    ///    pattern, so add, remove, or rename ANY field — bool or not — and
    ///    this test fails to COMPILE ("pattern does not mention field
    ///    `…`") until the list is updated. `recovery_kind` is not
    ///    special-cased out of this list — it goes through the exact same
    ///    macro as every bool field, which is what closed the previous
    ///    hole (a hand-written non-bool tail that carried no rung
    ///    requirement).
    /// 2. **Every named field claims Esc, by NAME.** The `assert_eq!`
    ///    below is a SET-EQUALITY check of NAMES — the macro's field-name
    ///    list vs. `setters`' (deduplicated) first elements — not a
    ///    count. There is deliberately no opt-out list: an `EXCLUDED`
    ///    escape hatch existed while `palette_open` and
    ///    `keymap_recorder_open` sat on this struct, and it let a field
    ///    skip guarantees 3 and 4 entirely just by being named in it.
    ///    Those two fields were write-only leftovers of the pre-#557
    ///    subscription closure — their real Esc handling is
    ///    `claim_command_palette` / `claim_keymap_recorder` in
    ///    `app/dispatch/input.rs` — so both they and the hatch are gone,
    ///    and a new field now has exactly one way to satisfy this test:
    ///    wire it a rung. This catches a name that isn't a
    ///    real field at all (a typo). It does NOT, by itself, catch a
    ///    correctly-labeled entry whose closure mutates the WRONG field:
    ///    for a genuinely new struct field `new_field_open`, the entry
    ///    `("new_field_open", |o| o.prefs_open = true)` passes this check
    ///    cleanly, because `"new_field_open"` IS a real field (the
    ///    exhaustive destructure in point 1 forces it into the macro's
    ///    list) and the label matches. Guarantee 4 below is what catches
    ///    that case.
    /// 3. **Every "claims Esc" field actually resolves to `Some(_)`.** The
    ///    first loop runs each `setters` closure through `only` and
    ///    asserts non-`None`. This alone does not catch the mis-wire in
    ///    point 2 either — `prefs_open` really does resolve to `Some(_)`.
    /// 4. **No two `setters` entries resolve to the same `Message`.** The
    ///    second loop asserts distinctness (by `Debug` string, since
    ///    `Message` isn't `PartialEq`) across every entry's resolved
    ///    message. A closure that (accidentally or via copy-paste) sets
    ///    the wrong field produces a DUPLICATE of that field's real entry
    ///    — e.g. the `new_field_open` mis-wire above would resolve to
    ///    `PreferencesMsg::Close` twice, once under each of two different
    ///    names — which this catches even though 2 and 3 both pass it.
    ///
    /// What it does NOT guarantee: which exact `Message` variant a rung
    /// returns (that's `every_modal_claims_escape`, above), or relative
    /// ordering between rungs (that's the dedicated precedence tests).
    #[test]
    fn every_ladder_field_claims_escape() {
        macro_rules! open_overlays_field_names {
            ($($field:ident),+ $(,)?) => {{
                let OpenOverlays { $($field: _),+ } = OpenOverlays::default();
                [$(stringify!($field)),+]
            }};
        }
        let field_names = open_overlays_field_names![
            find_replace_open,
            kbd_shortcuts_open,
            about_open,
            first_run_tour_open,
            autosave_recovery_open,
            prefs_open,
            annotate_open,
            erc_open,
            erc_save_profile_open,
            rename_open,
            remove_open,
            enable_vc_open,
            library_create_options_open,
            passive_calculator_open,
            annotate_reset_confirm_open,
            reprefix_confirm_open,
            app_quit_confirm_open,
            project_close_confirm_open,
            create_child_sheet_confirm_open,
            grid_properties_open,
            selection_filter_custom_open,
            library_picker_open,
            library_document_options_open,
            library_updates_open,
            library_primitive_picker_open,
            close_library_confirm_open,
            recovery_kind,
            error_notice_open,
            error_notice_detached,
            netlist_incomplete_prompt_open,
            netlist_prompt_detached,
            print_preview_open,
            bom_preview_open,
            net_color_custom_open,
            delete_confirm,
            edit_row_modal,
        ];

        type EscapeSetter = fn(&mut OpenOverlays);
        // `recovery_kind` appears three times under the SAME field name —
        // one entry per `RecoveryKind` variant — which is exactly what the
        // name-based (not count-based) equality check below is for: three
        // entries collapse to one name, same as any other field.
        let setters: &[(&str, EscapeSetter)] = &[
            ("find_replace_open", |o| o.find_replace_open = true),
            ("kbd_shortcuts_open", |o| o.kbd_shortcuts_open = true),
            ("about_open", |o| o.about_open = true),
            ("first_run_tour_open", |o| o.first_run_tour_open = true),
            ("autosave_recovery_open", |o| {
                o.autosave_recovery_open = true
            }),
            ("erc_open", |o| o.erc_open = true),
            ("erc_save_profile_open", |o| {
                o.erc_open = true;
                o.erc_save_profile_open = true
            }),
            ("annotate_open", |o| o.annotate_open = true),
            ("prefs_open", |o| o.prefs_open = true),
            ("rename_open", |o| o.rename_open = true),
            ("remove_open", |o| o.remove_open = true),
            ("enable_vc_open", |o| o.enable_vc_open = true),
            ("library_create_options_open", |o| {
                o.library_create_options_open = true
            }),
            ("passive_calculator_open", |o| {
                o.passive_calculator_open = true
            }),
            ("annotate_reset_confirm_open", |o| {
                o.annotate_reset_confirm_open = true
            }),
            ("reprefix_confirm_open", |o| o.reprefix_confirm_open = true),
            ("app_quit_confirm_open", |o| o.app_quit_confirm_open = true),
            ("project_close_confirm_open", |o| {
                o.project_close_confirm_open = true
            }),
            ("create_child_sheet_confirm_open", |o| {
                o.create_child_sheet_confirm_open = true
            }),
            ("grid_properties_open", |o| o.grid_properties_open = true),
            ("selection_filter_custom_open", |o| {
                o.selection_filter_custom_open = true
            }),
            ("library_document_options_open", |o| {
                o.library_document_options_open = true
            }),
            ("library_primitive_picker_open", |o| {
                o.library_primitive_picker_open = true
            }),
            ("library_updates_open", |o| o.library_updates_open = true),
            ("close_library_confirm_open", |o| {
                o.close_library_confirm_open = true
            }),
            ("library_picker_open", |o| o.library_picker_open = true),
            ("recovery_kind", |o| {
                o.recovery_kind = Some(RecoveryKind::LibraryMissing)
            }),
            ("recovery_kind", |o| {
                o.recovery_kind = Some(RecoveryKind::GitMissing)
            }),
            ("recovery_kind", |o| {
                o.recovery_kind = Some(RecoveryKind::BrokenBinding)
            }),
            ("error_notice_open", |o| o.error_notice_open = true),
            ("netlist_incomplete_prompt_open", |o| {
                o.netlist_incomplete_prompt_open = true
            }),
            ("print_preview_open", |o| o.print_preview_open = true),
            ("bom_preview_open", |o| o.bom_preview_open = true),
            ("net_color_custom_open", |o| o.net_color_custom_open = true),
            // Not a `bool` — the rung carries the owning library path, which
            // is the whole reason the ladder had to leave the subscription
            // (#535). Any path works here; the assertion is on which
            // message the rung resolves to, not on the path's value.
            ("delete_confirm", |o| {
                o.delete_confirm = Some(std::path::PathBuf::from("fixture.snxlib"))
            }),
            ("edit_row_modal", |o| {
                o.edit_row_modal = Some(std::path::PathBuf::from("fixture.snxlib"))
            }),
        ];

        // Where a notice lives, not a modal of its own: no rung.
        let macro_fields: std::collections::BTreeSet<&str> = field_names
            .into_iter()
            .filter(|name| !matches!(*name, "error_notice_detached" | "netlist_prompt_detached"))
            .collect();
        let claims_escape: std::collections::BTreeSet<&str> =
            setters.iter().map(|(name, _)| *name).collect();

        assert_eq!(
            macro_fields, claims_escape,
            "every OpenOverlays field must claim Esc — the macro's field \
             list and `setters` must name the same set of fields. A field \
             with no `setters` entry has no rung, so Esc falls through and \
             silently resets the canvas tool behind the open modal"
        );
        for (name, set) in setters {
            assert!(
                only(*set).is_some(),
                "{name} is on the Esc ladder but escape_message() fell \
                 through to `None` — Esc would silently reset the canvas \
                 tool behind it"
            );
        }

        // Distinctness: a name-only check (above) cannot tell a correctly
        // labeled entry from one whose closure copy-pasted a DIFFERENT
        // field's mutation — both pass the set-equality and `is_some()`
        // checks. Two entries resolving to the exact same `Message` is
        // that mis-wire's signature, so assert every resolved message is
        // unique. `Message` isn't `PartialEq`, so compare `Debug` output.
        let mut seen: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
        for (name, set) in setters {
            let resolved = format!("{:?}", only(*set).expect("checked non-None above"));
            if let Some(first_name) = seen.insert(resolved.clone(), name) {
                panic!(
                    "{first_name} and {name} both resolve to {resolved} — one \
                     of them is mutating the wrong field"
                );
            }
        }
    }
}
