//! Input routing: which app-level consumer owns a keyboard event, and
//! in which window (#557 phase 1, #558).
//!
//! # Two tiers, and this is the second
//!
//! The keyboard subscription filters on `event::Status::Ignored`
//! (`app/bootstrap/subscription.rs`), and iced computes that status per
//! window from the widget tree. A widget that calls
//! `shell.capture_event()` makes the event invisible here. So **widgets
//! arbitrate first and this router only ever sees their leftovers.**
//!
//! Widget capture is per-key, not per-widget, and that is load-bearing:
//! a focused `text_input` captures printable text, Backspace/Delete/
//! Home/End/ArrowLeft/ArrowRight, Ctrl+C/X/V/A — and Escape, which it
//! eats to unfocus itself (`iced_widget-0.14.2/src/text_input.rs`). It
//! does NOT capture F1, ArrowUp/ArrowDown, or unrecognised chords. The
//! command palette's arrow navigation works *only because* its focused
//! search field ignores those keys. No consumer below may therefore be
//! described as "sees every stroke"; the honest spec is "sees every
//! stroke no widget wanted". If the chord recorder's UI ever gains a
//! focused input, its capture semantics change silently — that is the
//! trap this paragraph exists to disarm.
//!
//! The footprint canvas composes for free: its own key path returns
//! `.and_capture()` for the keys it owns
//! (`library/editor/footprint/canvas/input/keys.rs`), so those never
//! arrive, and the ones it declines do.
//!
//! # Why the decision lives in `update`
//!
//! It used to live in the subscription closure, deciding against a
//! `KeyContext` snapshot baked when the subscription was last rebuilt —
//! i.e. one update stale, and blind to which window produced the key.
//! #535 made that argument for Esc and moved it here; this module
//! finishes the job for the other consumers.
//!
//! Deleting the snapshot also deletes churn: `Subscription::with` folds
//! its value into the subscription's identity hash, so every command
//! palette / recorder / shortcuts open-close used to tear down and
//! respawn the keyboard stream. The subscription's identity is now
//! constant for the process lifetime.
//!
//! # Precedence is NOT paint order
//!
//! The tempting move after #535 is "input precedence = reverse
//! `PAINT_ORDER`". It is wrong, and the counterexample is in the tree:
//! the chord recorder lives inside Preferences, which paints far BELOW
//! the command palette (`view/overlay_id.rs`), yet the recorder outranks
//! it for key claims — and must, or recording Ctrl+Shift+P into a
//! binding would be stolen by the palette it is trying to bind.
//!
//! Input precedence is about capture *exclusivity*; paint order is about
//! *stacking*. They correlate; they are not the same fact. So
//! [`CLAIM_ORDER`] is its own contract, in `PAINT_ORDER`'s style but not
//! derived from it. Its payoff is smaller than `PAINT_ORDER`'s — there
//! is no second reader that must agree with the order — and the honest
//! reason it exists is the exhaustive match: a new consumer cannot
//! compile until it states its claim, which is what stops the next
//! capture being added as one more branch of an `if` chain.
//!
//! # Not in scope
//!
//! The mouse subscription is a second ad-hoc router (three
//! `listen().map` variants swapped on drag state, each with its own
//! identity churn). It is deliberately untouched — but [`InputTarget`]
//! and [`Claim`] are the shape it would migrate to, so please extend
//! this module rather than inventing a parallel one.

mod key_repeat;
mod text_edit_tab;

use super::*;
use crate::keymap::{KeyStroke, ModifiersExt as _};
use iced::keyboard;
use key_repeat::{HeldKeyContext, runs_on_key_repeat};

/// Where an input event landed, and therefore what the user can see
/// while pressing the key.
///
/// This is #554's `EscapeSource` promoted: Esc was merely the first key
/// that needed to know its window. Derived from `WindowKind` by an
/// exhaustive match, so a new window kind is a compile error rather than
/// a silent inheritance of the main window's behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputTarget {
    /// The main window — or a synthesised event naming no window at all
    /// (`cancel_current_tool` from the palette or a keymap binding,
    /// `app/command/bridge.rs`), which resolves the same way.
    Main,
    /// An undocked document tab. Renders a full duplicate of the main
    /// view, overlay stack included, so anything painted in the main
    /// window is painted here too.
    UndockedTab,
    /// A modal showing in its own OS window (#547).
    DetachedModal(crate::app::state::ModalId),
    /// A detached dock panel. No canvas, no tool, no overlay stack.
    DetachedPanel,
    /// A detached Component Preview. Addressable through
    /// `library.editors`, but nothing in it answers a bare key today.
    ComponentEditor,
}

impl InputTarget {
    /// Whether this window paints the main overlay stack.
    ///
    /// `view_main_for` pushes `collect_overlays()` unconditionally and is
    /// shared by the main window and every undocked tab; every other
    /// window kind renders only its own body. So this is exactly the
    /// question "would the user see an overlay from here" — which is
    /// what the window-gated consumers need in phase 2 (#555).
    pub(crate) fn paints_overlay_stack(self) -> bool {
        matches!(self, Self::Main | Self::UndockedTab)
    }
}

/// One consumer's answer for one event.
///
/// [`Claim::Swallow`] is the point of this type. Exclusivity used to be
/// spelled `Message::Noop`, which simultaneously means "key released",
/// "stroke the keymap cannot express" and "the palette ate it" — so no
/// test could assert that an event was deliberately swallowed. These are
/// now three distinct answers.
///
/// Exclusivity is a per-event answer rather than a per-consumer flag on
/// purpose: the palette is exclusive-*with-leaks* (Esc, the arrows and
/// Ctrl+Shift+P leave it as [`Claim::Consume`]) and the recorder claims
/// `ModifiersChanged` as well as key presses. A boolean would
/// misrepresent both.
#[derive(Debug)]
enum Claim {
    /// Mine — dispatch this message.
    Consume(Box<Message>),
    /// Mine, and deliberately nothing happens. Must not fall through.
    Swallow,
    /// Not mine — ask the next consumer.
    Pass,
}

impl Claim {
    /// [`Claim::Consume`] of `message`, boxed: a message is far larger than
    /// the other answers.
    fn consume(message: Message) -> Self {
        Claim::Consume(Box::new(message))
    }
}

/// The app-level keyboard consumers, as identities rather than as
/// branches of an `if` chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputConsumer {
    /// Preferences ▸ Keyboard Shortcuts, while recording a chord.
    KeymapRecorder,
    CommandPalette,
    /// Tab, Shift+Tab and Esc on a Rules tab or Project tab drawing's open
    /// value label or held handle (#2530, #2685), and Esc on the Project
    /// tab's larger preview window; passes when none is open.
    RulePreview,
    /// The Esc ladder (#535 / #547 / #554), resolved in `escape.rs`.
    Escape,
    /// Enter creates the project while the New Project dialog is open (#2232).
    NewProject,
    /// Space ticks the focused row and Enter executes while the Update PCB
    /// dialog is open (#2371).
    UpdatePcb,
    /// Enter creates the board or sheet while New PCB / New Schematic is open.
    DocumentSetup,
    /// Enter picks Place while a Components row menu is open (#2535).
    ComponentsRowMenu,
    /// ↑ and ↓ move the Place Component picker's selection (mockup row 129).
    LibraryPicker,
    /// Enter saves all in Close Project and is swallowed in Remove (#2262).
    ConfirmDefault,
    /// Enter / Shift+Enter step through the Find results (mockup row 67).
    FindStep,
    /// F1 toggles the keyboard-shortcuts sheet.
    ShortcutsSheet,
    /// Tab moves the in-place editor along a part's fields (mockup row 65).
    TextEditTab,
    /// Ctrl/Alt+1-8 — the digit is data the keymap profile cannot
    /// express, which is why these stay hardcoded.
    SelectionSlots,
    /// The fallback: resolve against the active keymap profile.
    Keymap,
}

/// The arbitration contract: the first consumer that does not
/// [`Claim::Pass`] wins.
///
/// Order is behaviour. It is ported verbatim from the `if` chain this
/// module replaced, and it is NOT derived from `PAINT_ORDER` — see the
/// module docs for the counterexample that rules that out.
const CLAIM_ORDER: [InputConsumer; 15] = [
    InputConsumer::KeymapRecorder,
    InputConsumer::CommandPalette,
    InputConsumer::RulePreview,
    InputConsumer::Escape,
    InputConsumer::NewProject,
    InputConsumer::UpdatePcb,
    InputConsumer::DocumentSetup,
    InputConsumer::ComponentsRowMenu,
    InputConsumer::LibraryPicker,
    InputConsumer::ConfirmDefault,
    InputConsumer::FindStep,
    InputConsumer::ShortcutsSheet,
    InputConsumer::TextEditTab,
    InputConsumer::SelectionSlots,
    InputConsumer::Keymap,
];

/// A key the app claims before the keymap profile is consulted, whatever
/// modifiers are held. The claim fns and the Keyboard Shortcuts pane's
/// "overridden" marker both read these, so there is one list.
pub(crate) struct FixedKey {
    named: keyboard::key::Named,
    /// What the key does, shown in the pane's "overridden by" note.
    pub(crate) action: &'static str,
}

impl FixedKey {
    pub(crate) fn matches(&self, key: &keyboard::Key) -> bool {
        matches!(key.as_ref(), keyboard::Key::Named(named) if named == self.named)
    }
}

pub(crate) const ESCAPE_KEY: FixedKey = FixedKey {
    named: keyboard::key::Named::Escape,
    action: "Esc (cancel / close)",
};
const SHORTCUTS_SHEET_KEY: FixedKey = FixedKey {
    named: keyboard::key::Named::F1,
    action: "F1 (Keyboard Shortcuts)",
};

/// Every fixed key, for [`shadowing_fixed_key`].
const FIXED_APP_KEYS: [FixedKey; 2] = [ESCAPE_KEY, SHORTCUTS_SHEET_KEY];

/// The fixed app key that claims `stroke` before any profile binding can
/// see it, if any. A chord containing such a stroke is unreachable.
pub(crate) fn shadowing_fixed_key(stroke: &KeyStroke) -> Option<&'static str> {
    FIXED_APP_KEYS
        .iter()
        .find(|fixed| {
            KeyStroke::from_iced(
                &keyboard::Key::Named(fixed.named),
                keyboard::Modifiers::empty(),
            )
            .is_some_and(|fixed_stroke| fixed_stroke.key == stroke.key)
        })
        .map(|fixed| fixed.action)
}

/// [`shadowing_fixed_key`] over a trigger's display text (`"Ctrl+F1"`,
/// `"P W"`); pointer gestures and unparsable text are never shadowed.
pub(crate) fn shadowing_fixed_key_for_trigger(trigger: &str) -> Option<&'static str> {
    match crate::keymap::ShortcutTrigger::parse(trigger) {
        Ok(crate::keymap::ShortcutTrigger::KeySequence(strokes)) => {
            strokes.iter().find_map(shadowing_fixed_key)
        }
        _ => None,
    }
}

impl Signex {
    /// Classify the window an input event landed in.
    ///
    /// The main window is deliberately absent from `ui_state.windows`
    /// (only `ui_state.main_window_id` names it), so a miss maps to
    /// [`InputTarget::Main`] — as does a window already dropped from the
    /// map on a close frame, which is the answer the pre-#547 code gave.
    pub(crate) fn input_target(&self, window: Option<iced::window::Id>) -> InputTarget {
        use crate::app::state::WindowKind;

        let Some(window) = window else {
            return InputTarget::Main;
        };
        match self.ui_state.windows.get(&window) {
            None => InputTarget::Main,
            Some(WindowKind::DetachedModal(modal)) => InputTarget::DetachedModal(*modal),
            Some(WindowKind::UndockedTab { .. }) => InputTarget::UndockedTab,
            Some(WindowKind::DetachedPanel(_)) => InputTarget::DetachedPanel,
            Some(WindowKind::ComponentEditor { .. }) => InputTarget::ComponentEditor,
        }
    }

    /// The single entry point for `Message::KeyInput`.
    pub(crate) fn handle_key_input(
        &mut self,
        window: iced::window::Id,
        event: keyboard::Event,
    ) -> Task<Message> {
        if self.is_repeat_of_resuming_esc(&event) {
            return Task::none();
        }
        let (message, swallowed_by) = self.route_key_by(window, &event);
        // #1378 - a key press that is not a keymap stroke (Esc, F1, a
        // selection slot, a palette or recorder key) abandons the pending
        // chord instead of leaving it armed behind the user's back.
        //
        // A bare modifier or lock key is the exception: it is never a
        // stroke, but it is how the user reaches `Shift+G` mid-chord, and
        // on Windows a held one auto-repeats `KeyPressed`. So is the
        // auto-repeat of a run-once command, the only key the keymap
        // swallows: the same key is still down, so the chord wait runs out
        // and fires once. A repeat any other consumer drops still cancels.
        let run_once_repeat = swallowed_by == Some(InputConsumer::Keymap);
        if matches!(&event, keyboard::Event::KeyPressed { key, .. } if !is_modifier_key(key))
            && !run_once_repeat
            && !matches!(message, Some(Message::Ui(UiMsg::KeymapStroke { .. })))
        {
            self.cancel_pending_chord();
        }
        match message {
            Some(message) => self.update(message),
            None => Task::none(),
        }
    }

    /// `route_key` for the lock tests in `handlers/detached_modals.rs`.
    #[cfg(test)]
    pub(crate) fn route_key_for_test(
        &self,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Option<Message> {
        self.route_key(window, event)
    }

    /// The message [`Self::route_key_by`] returns, for the tests.
    #[cfg(test)]
    fn route_key(&self, window: iced::window::Id, event: &keyboard::Event) -> Option<Message> {
        self.route_key_by(window, event).0
    }

    /// Walk [`CLAIM_ORDER`] and return the one message this event earns,
    /// or `None` when it was swallowed or nobody wanted it, with the
    /// consumer that swallowed it when one did (`None` for a claimed,
    /// locked-out or unwanted event).
    ///
    /// `&self` on purpose: this is the testable heart. Build a `Signex`,
    /// set state, feed an event, assert the message — none of which was
    /// possible while these branches lived in a subscription closure.
    fn route_key_by(
        &self,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> (Option<Message>, Option<InputConsumer>) {
        let target = self.input_target(Some(window));
        // A confirm dialog in its own window locks the main window and the
        // undocked tabs: no stroke reaches the keymap or a tool there.
        // Esc still resolves, so the lock can always be dismissed.
        if matches!(target, InputTarget::Main | InputTarget::UndockedTab)
            && self.locking_confirm_window().is_some()
            && !matches!(event, keyboard::Event::KeyPressed { key, .. } if ESCAPE_KEY.matches(key))
        {
            return (None, None);
        }
        for consumer in CLAIM_ORDER {
            match self.claim(consumer, target, window, event) {
                Claim::Consume(message) => return (Some(*message), None),
                Claim::Swallow => return (None, Some(consumer)),
                Claim::Pass => continue,
            }
        }
        (None, None)
    }

    /// What `consumer` makes of this event.
    ///
    /// Exhaustive over [`InputConsumer`]: a new consumer will not compile
    /// until it states its claim.
    fn claim(
        &self,
        consumer: InputConsumer,
        target: InputTarget,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Claim {
        match consumer {
            InputConsumer::KeymapRecorder => self.claim_keymap_recorder(target, event),
            InputConsumer::CommandPalette => self.claim_command_palette(target, event),
            InputConsumer::RulePreview => {
                use crate::app::new_project::preview::SetupPreviewKey;
                let own_window =
                    target == InputTarget::DetachedModal(crate::app::state::ModalId::NewProject);
                match self.setup_preview_key(own_window, target.paints_overlay_stack(), event) {
                    Some(SetupPreviewKey::Consume(message)) => return Claim::consume(*message),
                    Some(SetupPreviewKey::Swallow) => return Claim::Swallow,
                    None => {}
                }
                // The Differential Pair Classes dialog sits above the tabs.
                let key = self
                    .diff_pair_preview_key(event)
                    .or_else(|| self.rules_drawing_key(window, event))
                    .or_else(|| self.project_preview_key(window, event))
                    .or_else(|| self.gerber_viewer_key(window, event))
                    .or_else(|| self.notebook_key(window, event));
                match (target, key) {
                    (InputTarget::Main | InputTarget::UndockedTab, Some(message)) => {
                        Claim::consume(message)
                    }
                    _ => Claim::Pass,
                }
            }
            InputConsumer::Escape => Self::claim_escape(window, event),
            InputConsumer::NewProject => self.claim_new_project(target, event),
            InputConsumer::UpdatePcb => self.claim_update_pcb(target, event),
            InputConsumer::DocumentSetup => self.claim_document_setup(target, event),
            InputConsumer::ComponentsRowMenu => {
                self.claim_components_row_menu(target, window, event)
            }
            InputConsumer::LibraryPicker => self
                .picker_arrow_message(target, event)
                .map_or(Claim::Pass, Claim::consume),
            InputConsumer::ConfirmDefault => self.claim_confirm_default(target, event),
            InputConsumer::FindStep => self.claim_find_step(target, window, event),
            InputConsumer::ShortcutsSheet => self.claim_shortcuts_sheet(target, event),
            InputConsumer::TextEditTab => self.claim_text_edit_tab(target, window, event),
            InputConsumer::SelectionSlots => Self::claim_selection_slots(event),
            InputConsumer::Keymap => self.claim_keymap(target, window, event),
        }
    }

    /// Chord recorder (Preferences ▸ Keyboard Shortcuts). Exclusive while
    /// open: every raw stroke belongs to the binding under edit and must
    /// NOT reach the live keymap resolver, or recording a shortcut would
    /// also fire it. The pending chord buffer is left untouched — it is
    /// only advanced by the resolver, which we skip.
    ///
    /// Held modifiers are claimed too: they drive the live "Ctrl+…" hint
    /// before a key lands.
    fn claim_keymap_recorder(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        self.claim_keymap_recorder_for(target, event, crate::keymap::Platform::current())
    }

    /// [`Self::claim_keymap_recorder`] recording as `platform` does: on
    /// macOS a Cmd press is stored as `Mod`.
    fn claim_keymap_recorder_for(
        &self,
        target: InputTarget,
        event: &keyboard::Event,
        platform: crate::keymap::Platform,
    ) -> Claim {
        if self.ui_state.preferences_keymap_recorder.is_none() {
            return Claim::Pass;
        }
        // #555 — only in the window the user is recording into. A stroke
        // typed elsewhere is aimed at that other window, so it routes
        // normally. The accepted cost, recorded in #557: such a stroke
        // can fire a live command while a chord is being recorded, which
        // is what the blanket capture used to prevent — but the user
        // deliberately moved focus there.
        if !self.recorder_visible_in(target) {
            return Claim::Pass;
        }
        match event {
            keyboard::Event::ModifiersChanged(modifiers) => Claim::consume(Message::Preferences(
                PreferencesMsg::Inner(crate::preferences::PrefMsg::KeymapRecorderModifiersChanged(
                    crate::keymap::Modifiers::from_iced(*modifiers).recorded_for(platform),
                )),
            )),
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
                match KeyStroke::from_iced(key, *modifiers) {
                    Some(stroke) => Claim::consume(Message::Preferences(PreferencesMsg::Inner(
                        crate::preferences::PrefMsg::KeymapRecorderKeyPressed(
                            stroke.recorded_for(platform),
                        ),
                    ))),
                    // Unexpressible as a `KeyStroke` (a bare modifier
                    // press). Still the recorder's — swallowed, not
                    // passed down to fire a live command mid-recording.
                    None => Claim::Swallow,
                }
            }
            _ => Claim::Swallow,
        }
    }

    /// Whether the chord recorder is on screen in `target`.
    ///
    /// The recorder lives inside the Preferences body, which renders
    /// either as the in-window card (main window and every undocked tab)
    /// or, once detached, only in its own OS window — `view/chrome.rs`
    /// passes `preferences_keymap_recorder` to both. So "where the
    /// recorder is visible" is exactly "where Preferences is painted".
    fn recorder_visible_in(&self, target: InputTarget) -> bool {
        use crate::app::state::ModalId;

        if self.modal_detached(ModalId::Preferences) {
            target == InputTarget::DetachedModal(ModalId::Preferences)
        } else {
            target.paints_overlay_stack()
        }
    }

    /// Command palette. Captures most input while open so typing into
    /// the search field doesn't fire tool shortcuts (`p`, `w`, `l`, …).
    /// Only navigation and dismiss keys leak through.
    fn claim_command_palette(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        if !self.ui_state.command_palette.open {
            return Claim::Pass;
        }
        // #555 — the palette paints in the main window and in undocked
        // tabs, nowhere else. It used to swallow keys typed into a
        // detached modal's own window, where it is not on screen: with
        // the palette open, an Esc there closed the invisible palette
        // instead of the modal the user was looking at.
        if !target.paints_overlay_stack() {
            return Claim::Pass;
        }
        let keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        } = event
        else {
            return Claim::Swallow;
        };
        let letter = crate::keymap::shortcut_char(key, *physical_key);
        match (key.as_ref(), *modifiers) {
            (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                Claim::consume(Message::CommandPalette(CommandPaletteMsg::Close))
            }
            (keyboard::Key::Named(keyboard::key::Named::ArrowDown), _) => {
                Claim::consume(Message::CommandPalette(CommandPaletteMsg::MoveSelection(1)))
            }
            (keyboard::Key::Named(keyboard::key::Named::ArrowUp), _) => Claim::consume(
                Message::CommandPalette(CommandPaletteMsg::MoveSelection(-1)),
            ),
            // Toggle: Ctrl+Shift+P while open closes.
            (keyboard::Key::Character(_), m) if letter == Some('p') && m.command() && m.shift() => {
                Claim::consume(Message::CommandPalette(CommandPaletteMsg::Close))
            }
            _ => Claim::Swallow,
        }
    }

    /// Esc is forwarded raw and resolved in `dispatch/escape.rs` against
    /// live state, carrying the window it was typed in (#535 / #547).
    ///
    /// Deliberately NOT routed through the keymap resolver: that advances
    /// the multi-stroke chord buffer and consults the active profile,
    /// neither of which Esc has ever done.
    fn claim_escape(window: iced::window::Id, event: &keyboard::Event) -> Claim {
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return Claim::Pass;
        };
        if !ESCAPE_KEY.matches(key) {
            return Claim::Pass;
        }
        Claim::consume(Message::Input(InputMsg::EscapePressed {
            window: Some(window),
        }))
    }

    /// Enter creates the project while the New Project dialog is open. Every
    /// other key is left to the dialog's fields and the keymap.
    fn claim_new_project(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        let Some(state) = self.ui_state.new_project.as_ref() else {
            return Claim::Pass;
        };
        // Its own window, or the main window while the dialog has none (the
        // in-window fallback): never a main window that sits under the dialog.
        let in_own_window =
            target == InputTarget::DetachedModal(crate::app::state::ModalId::NewProject);
        let in_main_fallback = target.paints_overlay_stack()
            && self
                .detached_window_of(crate::app::state::ModalId::NewProject)
                .is_none();
        if !in_own_window && !in_main_fallback {
            return Claim::Pass;
        }
        match event {
            // An open dropdown list owns Enter: it never submits the dialog.
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                ..
            } if state.dropdown_open => Claim::Swallow,
            // The Save as profile name field owns Enter: it saves the
            // profile, never creates the project (#2232).
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                ..
            } if state.save_profile_name.is_some() => Claim::consume(Message::from(
                crate::app::NewProjectMsg::SaveProfileConfirmed,
            )),
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                ..
            } => Claim::consume(Message::from(crate::app::NewProjectMsg::SubmitRequested)),
            _ => Claim::Pass,
        }
    }

    /// Space ticks the focused row and Enter executes while the Update PCB
    /// dialog is in front. The footprint picker over it keeps both keys.
    fn claim_update_pcb(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        let open = self.ui_state.update_pcb.as_ref().is_some_and(|s| !s.apply);
        if !open || !target.paints_overlay_stack() || self.library.primitive_picker.is_some() {
            return Claim::Pass;
        }
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return Claim::Pass;
        };
        if !modifiers.is_empty() {
            return Claim::Pass;
        }
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Space) => {
                Claim::consume(Message::from(crate::app::UpdatePcbMsg::FocusedRowToggled))
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => {
                Claim::consume(Message::from(crate::app::UpdatePcbMsg::ExecuteRequested))
            }
            _ => Claim::Pass,
        }
    }

    /// The key the row menu shows beside Place picks Place, the menu's first
    /// row, while a Components row menu is open in this window (#2535). That
    /// is the one-stroke binding of `placement_accept` the menu hint names;
    /// a profile that binds none places from the menu by click only (#2631).
    fn claim_components_row_menu(
        &self,
        target: InputTarget,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Claim {
        let Some(menu) = self.library.components_panel.row_menu.as_ref() else {
            return Claim::Pass;
        };
        let here = match menu.window {
            None => target == InputTarget::Main,
            Some(owner) => owner == window,
        };
        let keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        } = event
        else {
            return Claim::Pass;
        };
        let Some([hinted]) = self
            .ui_state
            .active_keymap
            .shown_sequence_for_id(crate::panels::components_panel::ROW_PLACE_COMMAND)
        else {
            return Claim::Pass;
        };
        let pressed = KeyStroke::from_iced(key, *modifiers).as_ref() == Some(hinted)
            || KeyStroke::physical_fallback(key, *physical_key, *modifiers).as_ref()
                == Some(hinted);
        if !here || !pressed {
            return Claim::Pass;
        }
        let place = Message::Library(
            crate::library::LibraryMessage::ComponentsPanelRowMenuPicked(
                crate::library::state::ComponentsRowAction::Place,
            ),
        );
        Claim::consume(match menu.window {
            None => place,
            Some(owner) => {
                crate::app::dispatch::window_action::scope_row_menu_message(owner, place)
            }
        })
    }

    /// Enter in the New PCB / New Schematic dialog (main window only): an
    /// open list keeps it, the profile name field saves, else Create.
    fn claim_document_setup(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        let Some(state) = self.ui_state.document_setup.as_ref() else {
            return Claim::Pass;
        };
        let keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            ..
        } = event
        else {
            return Claim::Pass;
        };
        if !target.paints_overlay_stack() {
            return Claim::Pass;
        }
        use crate::app::{DocumentSetupMsg as D, NewProjectMsg as N};
        if state.draft.dropdown_open {
            Claim::Swallow
        } else if state.draft.save_profile_name.is_some() {
            Claim::consume(Message::from(D::Setup(N::SaveProfileConfirmed)))
        } else {
            Claim::consume(Message::from(D::SubmitRequested))
        }
    }

    /// Enter in the Remove or Close Project confirm, in the window that shows
    /// its card: Close Project saves all; Remove swallows it, so Enter never
    /// removes or deletes (`confirm_enter_message`).
    fn claim_confirm_default(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        use crate::app::state::ModalId;
        let keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            modifiers,
            ..
        } = event
        else {
            return Claim::Pass;
        };
        if !modifiers.is_empty() {
            return Claim::Pass;
        }
        let front = match target {
            InputTarget::DetachedModal(modal) => Some(modal),
            _ if target.paints_overlay_stack() => self.confirm_in_front(),
            _ => None,
        };
        match front {
            Some(modal @ (ModalId::RemoveDialog | ModalId::ProjectCloseConfirm)) => self
                .confirm_enter_message(modal)
                .map_or(Claim::Swallow, Claim::consume),
            _ => Claim::Pass,
        }
    }

    /// Enter / Shift+Enter step the Find results (mockup row 67) in the
    /// window that shows the dialog: its own window, or the window it was
    /// opened in while nothing is painted above it there.
    fn claim_find_step(
        &self,
        target: InputTarget,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Claim {
        use crate::app::state::ModalId;
        let in_dialog = target == InputTarget::DetachedModal(ModalId::FindReplace)
            || (target.paints_overlay_stack()
                && self.detached_window_of(ModalId::FindReplace).is_none()
                && self.overlay_belongs_to(self.ui_state.find_replace.window, window)
                && self.find_in_front());
        self.find_step_key(in_dialog, event)
            .map_or(Claim::Pass, Claim::consume)
    }

    /// Whether Find is the frontmost open overlay: the one Esc would close.
    fn find_in_front(&self) -> bool {
        matches!(
            self.escape_overlay_message(),
            Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close
            ))
        )
    }

    /// F1 toggles the keyboard-shortcuts sheet: open if closed, close if
    /// open. Stays hardcoded because it depends on which modal is open —
    /// app state, not the keymap profile.
    fn claim_shortcuts_sheet(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return Claim::Pass;
        };
        if !SHORTCUTS_SHEET_KEY.matches(key) {
            return Claim::Pass;
        }
        // #555 — a dead key where the sheet cannot appear. Toggling it
        // over in the main window would change something the user cannot
        // see from the window they are typing in; the same call #554 made
        // for Esc in a detached panel.
        //
        // `Swallow`, not `Pass`: F1 is expressible as a `KeyStroke`
        // (`KeyToken::Function(1)`), so passing would hand it to the
        // keymap profile — which today can never see F1 at all, because
        // this consumer claims it in every window. Falling through would
        // make a profile binding for F1 suddenly reachable here, which is
        // more than the decision asked for.
        let in_own_window =
            target == InputTarget::DetachedModal(crate::app::state::ModalId::KeyboardShortcuts);
        if !target.paints_overlay_stack() && !in_own_window {
            return Claim::Swallow;
        }
        Claim::consume(if self.ui_state.keyboard_shortcuts_open {
            Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)
        } else {
            Message::Menu(MenuMessage::OpenKeyboardShortcuts)
        })
    }

    /// Ctrl+1-8 store selection memory, Alt+1-8 recall it. These carry
    /// the digit as data the keymap profile format cannot express, so
    /// they stay hardcoded.
    ///
    /// The `selection_slot_from_key` guard is load-bearing (#127):
    /// without it these arms matched EVERY Ctrl/Alt chord and swallowed
    /// it, shadowing Ctrl+C/X/V/D before they reached the keymap
    /// resolver below.
    fn claim_selection_slots(event: &keyboard::Event) -> Claim {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return Claim::Pass;
        };
        let keyboard::Key::Character(c) = key.as_ref() else {
            return Claim::Pass;
        };
        let Some(slot) = selection_slot_from_key(c) else {
            return Claim::Pass;
        };
        let m = *modifiers;
        if m.command() && !m.alt() {
            return Claim::consume(Message::Selection(
                crate::app::selection_request::SelectionRequest::StoreSlot { slot },
            ));
        }
        if m.alt() && !m.command() {
            return Claim::consume(Message::Selection(
                crate::app::selection_request::SelectionRequest::RecallSlot { slot },
            ));
        }
        Claim::Pass
    }

    /// Everything else routes through the active keymap profile: forward
    /// the raw stroke, resolved in `dispatch/keymap.rs` where the
    /// multi-stroke chord buffer lives in `UiState`.
    ///
    /// A stroke iced cannot express as a `KeyStroke` (a bare modifier
    /// press) is passed: nobody wanted it. The auto-repeat of a stroke that
    /// runs a held-key-once command is swallowed: the keymap saw the
    /// key, and `handle_key_input` keeps the pending chord for it.
    fn claim_keymap(
        &self,
        target: InputTarget,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Claim {
        let keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            repeat,
            ..
        } = event
        else {
            return Claim::Pass;
        };
        let Some(logical) = KeyStroke::from_iced(key, *modifiers) else {
            return Claim::Pass;
        };
        let stroke = KeyStroke::physical_fallback(key, *physical_key, *modifiers)
            .filter(|_| !self.stroke_matches_anything(window, target, &logical))
            .unwrap_or(logical);
        // Some commands run once on a held key: docs/DECISIONS.md, "Held keys".
        if *repeat && self.repeat_is_ignored(window, target, &stroke) {
            return Claim::Swallow;
        }
        Claim::consume(Message::Ui(UiMsg::KeymapStroke {
            window: Some(window),
            stroke,
        }))
    }

    /// Whether the logical stroke, alone or after the pending chord prefix,
    /// is a binding or the start of one. Only then does it keep priority over
    /// the physical-key fallback (#1645).
    fn stroke_matches_anything(
        &self,
        window: iced::window::Id,
        target: InputTarget,
        stroke: &KeyStroke,
    ) -> bool {
        let contexts = self.shortcut_contexts(Some(window));
        let keymap = &self.ui_state.active_keymap;
        [
            self.after_pending_chord(target, stroke),
            vec![stroke.clone()],
        ]
        .iter()
        .any(|sequence| {
            let lookup = keymap.lookup(sequence, &contexts);
            lookup.command.is_some() || lookup.matched || lookup.pending
        })
    }

    /// Whether `stroke` would run a command a held key runs only once
    /// ([`runs_on_key_repeat`]): the command after the pending chord prefix,
    /// or, when that sequence is no binding and no prefix, the stroke's own
    /// command.
    fn repeat_is_ignored(
        &self,
        window: iced::window::Id,
        target: InputTarget,
        stroke: &KeyStroke,
    ) -> bool {
        let contexts = self.shortcut_contexts(Some(window));
        let keymap = &self.ui_state.active_keymap;
        let chord = keymap.lookup(&self.after_pending_chord(target, stroke), &contexts);
        let command = match chord.command {
            Some(command) => Some(command),
            None if chord.matched || chord.pending => None,
            None => {
                keymap
                    .lookup(std::slice::from_ref(stroke), &contexts)
                    .command
            }
        };
        // The run the command would bend is the one in the key's window: an
        // undocked tab acts in its own window, every other window in the main.
        let acting = (target == InputTarget::UndockedTab).then_some(window);
        let ctx = HeldKeyContext {
            run_open: self.has_open_run_in(acting),
        };
        command.is_some_and(|command| !runs_on_key_repeat(&command, ctx))
    }

    /// The pending chord prefix of `target`, if any, followed by `stroke`.
    fn after_pending_chord(&self, target: InputTarget, stroke: &KeyStroke) -> Vec<KeyStroke> {
        let mut sequence = if self.ui_state.keymap_pending_target == Some(target) {
            self.ui_state.keymap_pending_sequence.clone()
        } else {
            Vec::new()
        };
        sequence.push(stroke.clone());
        sequence
    }
}

/// A key press that only changes modifier or lock state. iced reports these
/// as `KeyPressed` with a named key, and `KeyStroke::from_iced` rejects
/// them, so they route to nothing.
fn is_modifier_key(key: &keyboard::Key) -> bool {
    use keyboard::key::Named;
    matches!(
        key,
        keyboard::Key::Named(
            Named::Shift
                | Named::Control
                | Named::Alt
                | Named::AltGraph
                | Named::Super
                | Named::Meta
                | Named::Hyper
                | Named::CapsLock
                | Named::NumLock
                | Named::ScrollLock
                | Named::Fn
                | Named::FnLock
                | Named::Symbol
                | Named::SymbolLock
        )
    )
}

/// Which selection-memory slot a digit key names, if any.
///
/// Moved here with its only consumer (`claim_selection_slots`) when the
/// keyboard branches left the subscription.
fn selection_slot_from_key(key: &str) -> Option<usize> {
    match key {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        "5" => Some(4),
        "6" => Some(5),
        "7" => Some(6),
        "8" => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod components_row_menu_tests;

#[cfg(test)]
mod key_repeat_tests;

#[cfg(test)]
mod physical_key_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{ModalId, WindowKind};

    fn quiet_app() -> Signex {
        let (mut app, _boot) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        app
    }

    fn main_window(app: &Signex) -> iced::window::Id {
        app.ui_state
            .main_window_id
            .expect("Signex::new opens the main window")
    }

    fn unidentified() -> keyboard::key::Physical {
        keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified)
    }

    /// Give `kind` its own OS window and hand back that window's id.
    fn open_window(app: &mut Signex, kind: WindowKind) -> iced::window::Id {
        let id = iced::window::Id::unique();
        app.ui_state.windows.insert(id, kind);
        id
    }

    fn press(key: keyboard::Key, modifiers: keyboard::Modifiers) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: unidentified(),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }
    }

    fn named(key: keyboard::key::Named) -> keyboard::Event {
        press(keyboard::Key::Named(key), keyboard::Modifiers::default())
    }

    fn character(c: &str, modifiers: keyboard::Modifiers) -> keyboard::Event {
        press(keyboard::Key::Character(c.into()), modifiers)
    }

    fn released(key: keyboard::Key) -> keyboard::Event {
        keyboard::Event::KeyReleased {
            key: key.clone(),
            modified_key: key,
            physical_key: unidentified(),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
        }
    }

    /// A recorder mid-capture. `KeymapRecorderState` has no `Default` —
    /// it is always seeded from the binding being edited.
    fn recording() -> crate::app::KeymapRecorderState {
        crate::app::KeymapRecorderState::new(
            crate::keymap::BindingSlot {
                binding: 0,
                trigger: 0,
            },
            crate::keymap::AppCommandId::new("cancel_current_tool").expect("valid command id"),
            "Cancel Current Tool".to_string(),
            crate::keymap::ShortcutContext::Global,
            "Esc".to_string(),
        )
    }

    // ── The contract itself ─────────────────────────────────────────

    #[test]
    fn claim_order_lists_every_consumer_exactly_once() {
        for consumer in [
            InputConsumer::KeymapRecorder,
            InputConsumer::CommandPalette,
            InputConsumer::RulePreview,
            InputConsumer::Escape,
            InputConsumer::NewProject,
            InputConsumer::UpdatePcb,
            InputConsumer::DocumentSetup,
            InputConsumer::ComponentsRowMenu,
            InputConsumer::LibraryPicker,
            InputConsumer::ConfirmDefault,
            InputConsumer::FindStep,
            InputConsumer::ShortcutsSheet,
            InputConsumer::TextEditTab,
            InputConsumer::SelectionSlots,
            InputConsumer::Keymap,
        ] {
            assert_eq!(
                CLAIM_ORDER.iter().filter(|c| **c == consumer).count(),
                1,
                "{consumer:?} must appear in CLAIM_ORDER exactly once"
            );
        }
    }

    /// Enter (the default profile's place key) places the row while its
    /// menu is open, and is the keymap's again once the menu has closed
    /// (#2535). Other profiles: `components_row_menu_tests`.
    #[test]
    fn enter_places_the_row_whose_menu_is_open() {
        let mut app = quiet_app();
        let main = main_window(&app);
        app.library.components_panel.row_menu = Some(crate::library::state::ComponentsRowMenu {
            key: crate::library::state::ComponentsPanelRowKey {
                library_path: "lib.snxlib".into(),
                table: "t".into(),
                row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(1)),
            },
            x: 0.0,
            y: 0.0,
            window: None,
        });
        let routed = app.route_key(main, &named(keyboard::key::Named::Enter));
        assert!(
            matches!(
                routed,
                Some(Message::Library(
                    crate::library::LibraryMessage::ComponentsPanelRowMenuPicked(
                        crate::library::state::ComponentsRowAction::Place
                    )
                ))
            ),
            "{routed:?}"
        );
        app.library.components_panel.row_menu = None;
        let routed = app.route_key(main, &named(keyboard::key::Named::Enter));
        assert!(
            !matches!(
                routed,
                Some(Message::Library(
                    crate::library::LibraryMessage::ComponentsPanelRowMenuPicked(_)
                ))
            ),
            "{routed:?}"
        );
    }

    #[test]
    fn input_target_classifies_every_window_kind() {
        let mut app = quiet_app();
        assert_eq!(app.input_target(None), InputTarget::Main);
        assert_eq!(
            app.input_target(app.ui_state.main_window_id),
            InputTarget::Main
        );
        assert_eq!(
            app.input_target(Some(iced::window::Id::unique())),
            InputTarget::Main,
            "a window already dropped from the map falls back to Main"
        );

        let modal = open_window(&mut app, WindowKind::DetachedModal(ModalId::ErcDialog));
        let tab = open_window(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );
        let panel = open_window(
            &mut app,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );
        let editor = open_window(
            &mut app,
            WindowKind::ComponentEditor {
                library_path: std::path::PathBuf::from("/tmp/parts.snxlib"),
                table: "Resistors".to_string(),
                row_id: signex_library::RowId::new(),
            },
        );

        assert_eq!(
            app.input_target(Some(modal)),
            InputTarget::DetachedModal(ModalId::ErcDialog)
        );
        assert_eq!(app.input_target(Some(tab)), InputTarget::UndockedTab);
        assert_eq!(app.input_target(Some(panel)), InputTarget::DetachedPanel);
        assert_eq!(app.input_target(Some(editor)), InputTarget::ComponentEditor);
    }

    #[test]
    fn only_main_and_undocked_paint_the_overlay_stack() {
        assert!(InputTarget::Main.paints_overlay_stack());
        assert!(InputTarget::UndockedTab.paints_overlay_stack());
        assert!(!InputTarget::DetachedModal(ModalId::Preferences).paints_overlay_stack());
        assert!(!InputTarget::DetachedPanel.paints_overlay_stack());
        assert!(!InputTarget::ComponentEditor.paints_overlay_stack());
    }

    // ── Consumer 1: the chord recorder ──────────────────────────────

    #[test]
    fn the_recorder_claims_every_stroke_while_open() {
        let mut app = quiet_app();
        let w = main_window(&app);
        app.ui_state.preferences_keymap_recorder = Some(recording());

        assert!(
            matches!(
                app.route_key(w, &character("w", keyboard::Modifiers::default())),
                Some(Message::Preferences(_))
            ),
            "a stroke typed while recording belongs to the binding under edit"
        );
        assert!(
            matches!(
                app.route_key(
                    w,
                    &keyboard::Event::ModifiersChanged(keyboard::Modifiers::CTRL)
                ),
                Some(Message::Preferences(_))
            ),
            "held modifiers drive the live Ctrl+… hint"
        );
    }

    #[test]
    fn the_recorder_outranks_the_palette() {
        // Not paint order: Preferences paints far BELOW the palette, yet
        // recording Ctrl+Shift+P must bind it, not let the palette eat it.
        let mut app = quiet_app();
        let w = main_window(&app);
        app.ui_state.preferences_keymap_recorder = Some(recording());
        app.ui_state.command_palette.open = true;

        let command_shift_p = character(
            "p",
            keyboard::Modifiers::COMMAND | keyboard::Modifiers::SHIFT,
        );
        assert!(
            matches!(
                app.route_key(w, &command_shift_p),
                Some(Message::Preferences(_))
            ),
            "the recorder claims the stroke the palette would otherwise close on"
        );
    }

    #[test]
    fn the_recorder_swallows_what_it_cannot_express() {
        let mut app = quiet_app();
        let w = main_window(&app);
        app.ui_state.preferences_keymap_recorder = Some(recording());

        assert!(
            app.route_key(
                w,
                &released(keyboard::Key::Named(keyboard::key::Named::Control))
            )
            .is_none(),
            "swallowed, not passed down to fire a live command mid-recording"
        );
    }

    // ── Consumer 2: the command palette ─────────────────────────────

    #[test]
    fn the_palette_leaks_only_its_four_navigation_keys() {
        let mut app = quiet_app();
        let w = main_window(&app);
        app.ui_state.command_palette.open = true;

        for (event, what) in [
            (named(keyboard::key::Named::Escape), "Escape closes"),
            (named(keyboard::key::Named::ArrowDown), "ArrowDown moves"),
            (named(keyboard::key::Named::ArrowUp), "ArrowUp moves"),
            (
                character(
                    "p",
                    keyboard::Modifiers::COMMAND | keyboard::Modifiers::SHIFT,
                ),
                "Cmd/Ctrl+Shift+P toggles",
            ),
        ] {
            assert!(
                matches!(app.route_key(w, &event), Some(Message::CommandPalette(_))),
                "{what}"
            );
        }
    }

    #[test]
    fn the_palette_swallows_tool_shortcuts_so_typing_does_not_fire_them() {
        let mut app = quiet_app();
        let w = main_window(&app);
        app.ui_state.command_palette.open = true;

        assert!(
            app.route_key(w, &character("w", keyboard::Modifiers::default()))
                .is_none(),
            "`w` typed into the search field must not arm the Wire tool"
        );
    }

    // ── Consumer 3: Escape ──────────────────────────────────────────

    #[test]
    fn escape_is_forwarded_raw_with_its_window() {
        let app = quiet_app();
        let w = main_window(&app);

        assert!(
            matches!(
                app.route_key(w, &named(keyboard::key::Named::Escape)),
                Some(Message::Input(InputMsg::EscapePressed { window: Some(id) })) if id == w
            ),
            "the ladder resolves in `escape.rs` against live state"
        );
    }

    #[test]
    fn enter_creates_only_while_the_new_project_dialog_is_open() {
        let mut app = quiet_app();
        let w = main_window(&app);
        let submit = |m: &Option<Message>| {
            matches!(
                m,
                Some(Message::Project(crate::app::ProjectMsg::Setup(
                    crate::app::NewProjectMsg::SubmitRequested
                )))
            )
        };
        assert!(!submit(
            &app.route_key(w, &named(keyboard::key::Named::Enter))
        ));
        app.ui_state.new_project = Some(Box::new(crate::app::NewProjectState::new(String::new())));
        assert!(submit(
            &app.route_key(w, &named(keyboard::key::Named::Enter))
        ));
        assert!(
            !submit(&app.route_key(w, &character("a", keyboard::Modifiers::default()))),
            "other keys reach the fields"
        );
        let _ = app.update(Message::from(crate::app::NewProjectMsg::DropdownToggled(
            true,
        )));
        assert!(
            app.route_key(w, &named(keyboard::key::Named::Enter))
                .is_none(),
            "an open dropdown owns Enter"
        );
        // A real pick closes the list, so Enter submits again.
        let _ = app.update(Message::from(crate::app::NewProjectMsg::SheetSizePicked(
            "A4".into(),
        )));
        assert!(submit(
            &app.route_key(w, &named(keyboard::key::Named::Enter))
        ));
    }

    // ── Consumer 4: F1 ──────────────────────────────────────────────

    #[test]
    fn f1_toggles_the_shortcuts_sheet_both_ways() {
        let mut app = quiet_app();
        let w = main_window(&app);

        assert!(
            matches!(
                app.route_key(w, &named(keyboard::key::Named::F1)),
                Some(Message::Menu(_))
            ),
            "closed sheet opens"
        );
        app.ui_state.keyboard_shortcuts_open = true;
        assert!(
            matches!(
                app.route_key(w, &named(keyboard::key::Named::F1)),
                Some(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts))
            ),
            "open sheet closes"
        );
    }

    // ── Consumer 5: selection slots ─────────────────────────────────

    #[test]
    fn command_and_alt_digits_carry_the_slot_as_data() {
        let app = quiet_app();
        let w = main_window(&app);

        assert!(matches!(
            app.route_key(w, &character("3", keyboard::Modifiers::COMMAND)),
            Some(Message::Selection(
                crate::app::selection_request::SelectionRequest::StoreSlot { slot: 2 }
            ))
        ));
        assert!(matches!(
            app.route_key(w, &character("3", keyboard::Modifiers::ALT)),
            Some(Message::Selection(
                crate::app::selection_request::SelectionRequest::RecallSlot { slot: 2 }
            ))
        ));
    }

    /// #127 — without the `selection_slot_from_key` guard these arms
    /// matched EVERY Ctrl/Alt chord and swallowed it, shadowing
    /// Ctrl+C/X/V/D before they reached the keymap resolver.
    #[test]
    fn a_non_digit_command_chord_falls_through_to_the_keymap() {
        let app = quiet_app();
        let w = main_window(&app);

        assert!(
            matches!(
                app.route_key(w, &character("c", keyboard::Modifiers::COMMAND)),
                Some(Message::Ui(UiMsg::KeymapStroke { .. }))
            ),
            "Cmd/Ctrl+C must reach the keymap resolver, not the slot consumer"
        );
    }

    #[test]
    fn digits_without_ctrl_or_alt_fall_through_to_the_keymap() {
        let app = quiet_app();
        let w = main_window(&app);

        assert!(matches!(
            app.route_key(w, &character("3", keyboard::Modifiers::default())),
            Some(Message::Ui(UiMsg::KeymapStroke { .. }))
        ));
    }

    // ── Consumer 6: the keymap fallback ─────────────────────────────

    #[test]
    fn an_ordinary_stroke_reaches_the_keymap_resolver() {
        let app = quiet_app();
        let w = main_window(&app);

        assert!(matches!(
            app.route_key(w, &character("w", keyboard::Modifiers::default())),
            Some(Message::Ui(UiMsg::KeymapStroke { .. }))
        ));
    }

    #[test]
    fn events_nobody_wants_produce_no_message() {
        let app = quiet_app();
        let w = main_window(&app);

        // These used to become `Message::Noop`, which woke `update` for
        // nothing. Now they are simply unclaimed.
        assert!(
            app.route_key(
                w,
                &keyboard::Event::ModifiersChanged(keyboard::Modifiers::CTRL)
            )
            .is_none(),
            "an idle modifier change belongs to nobody"
        );
        assert!(
            app.route_key(w, &released(keyboard::Key::Character("w".into())))
                .is_none(),
            "key releases belong to nobody"
        );
    }

    // ── #555: the window gates ──────────────────────────────────────

    /// The headline symptom. With the palette open, an Esc typed inside a
    /// detached modal's own window used to close the palette — which is
    /// not painted there — so the modal the user was looking at never saw
    /// the key.
    #[test]
    fn an_open_palette_no_longer_eats_escape_in_a_window_it_does_not_paint_in() {
        let mut app = quiet_app();
        app.ui_state.command_palette.open = true;
        let modal = open_window(&mut app, WindowKind::DetachedModal(ModalId::ErcDialog));

        assert!(
            matches!(
                app.route_key(modal, &named(keyboard::key::Named::Escape)),
                Some(Message::Input(InputMsg::EscapePressed { window: Some(id) })) if id == modal
            ),
            "Esc belongs to the modal in this window, not to an invisible palette"
        );
    }

    #[test]
    fn an_open_palette_stops_swallowing_keys_in_windows_it_does_not_paint_in() {
        let mut app = quiet_app();
        app.ui_state.command_palette.open = true;
        let panel = open_window(
            &mut app,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );

        assert!(
            matches!(
                app.route_key(panel, &character("w", keyboard::Modifiers::default())),
                Some(Message::Ui(UiMsg::KeymapStroke { .. }))
            ),
            "the search field is not on screen here, so the key routes normally"
        );
    }

    #[test]
    fn the_palette_still_owns_its_keys_where_it_does_paint() {
        let mut app = quiet_app();
        app.ui_state.command_palette.open = true;
        let tab = open_window(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );

        assert!(
            app.route_key(tab, &character("w", keyboard::Modifiers::default()))
                .is_none(),
            "an undocked tab paints the whole overlay stack, palette included"
        );
    }

    #[test]
    fn f1_is_a_dead_key_where_the_sheet_cannot_appear() {
        let mut app = quiet_app();
        let modal = open_window(&mut app, WindowKind::DetachedModal(ModalId::ErcDialog));
        let panel = open_window(
            &mut app,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );

        for window in [modal, panel] {
            assert!(
                app.route_key(window, &named(keyboard::key::Named::F1))
                    .is_none(),
                "toggling the sheet in another window would change what the \
                 user cannot see"
            );
        }
        assert!(
            !app.ui_state.keyboard_shortcuts_open,
            "and nothing opened as a side effect"
        );
    }

    #[test]
    fn the_recorder_claims_only_where_preferences_is_painted_inline() {
        let mut app = quiet_app();
        app.ui_state.preferences_keymap_recorder = Some(recording());
        let main = main_window(&app);
        let elsewhere = open_window(&mut app, WindowKind::DetachedModal(ModalId::ErcDialog));

        assert!(
            matches!(
                app.route_key(main, &character("w", keyboard::Modifiers::default())),
                Some(Message::Preferences(_))
            ),
            "Preferences is inline, so the main window is where recording happens"
        );
        assert!(
            matches!(
                app.route_key(elsewhere, &character("w", keyboard::Modifiers::default())),
                Some(Message::Ui(UiMsg::KeymapStroke { .. }))
            ),
            "a stroke aimed at another window routes normally (#557 decision 1)"
        );
    }

    #[test]
    fn a_detached_preferences_moves_the_recorder_with_it() {
        let mut app = quiet_app();
        app.ui_state.preferences_keymap_recorder = Some(recording());
        let prefs = open_window(&mut app, WindowKind::DetachedModal(ModalId::Preferences));
        let main = main_window(&app);

        assert!(
            matches!(
                app.route_key(prefs, &character("w", keyboard::Modifiers::default())),
                Some(Message::Preferences(_))
            ),
            "the detached Preferences body renders the recorder"
        );
        assert!(
            matches!(
                app.route_key(main, &character("w", keyboard::Modifiers::default())),
                Some(Message::Ui(UiMsg::KeymapStroke { .. }))
            ),
            "the in-window card is not painted once detached, so the main \
             window is no longer where recording happens"
        );
    }

    /// Moved here with the helper. The `None` half is a regression
    /// guard for #103: the Ctrl/Alt+1-8 arms are gated on
    /// `selection_slot_from_key(c).is_some()`, and these letters
    /// returning `None` is exactly what lets Ctrl+C/X/V/D reach the
    /// keymap resolver instead of being shadowed into a no-op.
    #[test]
    fn selection_slot_only_matches_digits_one_through_eight() {
        for (i, key) in ["1", "2", "3", "4", "5", "6", "7", "8"].iter().enumerate() {
            assert_eq!(selection_slot_from_key(key), Some(i));
        }
        for key in ["c", "x", "v", "d", "g", "s", "a", "z", "0", "9", "", "12"] {
            assert_eq!(
                selection_slot_from_key(key),
                None,
                "{key} must not resolve to a selection slot"
            );
        }
    }
}
