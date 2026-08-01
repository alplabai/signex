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

use super::*;
use crate::keymap::KeyStroke;
use iced::keyboard;

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
    #[allow(dead_code)] // phase 2 (#555) is its first caller
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
    Consume(Message),
    /// Mine, and deliberately nothing happens. Must not fall through.
    Swallow,
    /// Not mine — ask the next consumer.
    Pass,
}

/// The app-level keyboard consumers, as identities rather than as
/// branches of an `if` chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputConsumer {
    /// Preferences ▸ Keyboard Shortcuts, while recording a chord.
    KeymapRecorder,
    CommandPalette,
    /// The Esc ladder (#535 / #547 / #554), resolved in `escape.rs`.
    Escape,
    /// F1 toggles the keyboard-shortcuts sheet.
    ShortcutsSheet,
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
const CLAIM_ORDER: [InputConsumer; 6] = [
    InputConsumer::KeymapRecorder,
    InputConsumer::CommandPalette,
    InputConsumer::Escape,
    InputConsumer::ShortcutsSheet,
    InputConsumer::SelectionSlots,
    InputConsumer::Keymap,
];

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
        match self.route_key(window, &event) {
            Some(message) => self.update(message),
            None => Task::none(),
        }
    }

    /// Walk [`CLAIM_ORDER`] and return the one message this event earns,
    /// or `None` when it was swallowed or nobody wanted it.
    ///
    /// `&self` on purpose: this is the testable heart. Build a `Signex`,
    /// set state, feed an event, assert the message — none of which was
    /// possible while these branches lived in a subscription closure.
    fn route_key(&self, window: iced::window::Id, event: &keyboard::Event) -> Option<Message> {
        let target = self.input_target(Some(window));
        for consumer in CLAIM_ORDER {
            match self.claim(consumer, target, window, event) {
                Claim::Consume(message) => return Some(message),
                Claim::Swallow => return None,
                Claim::Pass => continue,
            }
        }
        None
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
            InputConsumer::Escape => Self::claim_escape(window, event),
            InputConsumer::ShortcutsSheet => self.claim_shortcuts_sheet(target, event),
            InputConsumer::SelectionSlots => Self::claim_selection_slots(event),
            InputConsumer::Keymap => Self::claim_keymap(event),
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
        // Phase 1 ports this window-blind, exactly as it was. Phase 2
        // (#555) gates it on the window that paints Preferences.
        let _ = target;
        if self.ui_state.preferences_keymap_recorder.is_none() {
            return Claim::Pass;
        }
        match event {
            keyboard::Event::ModifiersChanged(modifiers) => Claim::Consume(Message::Preferences(
                PreferencesMsg::Inner(crate::preferences::PrefMsg::KeymapRecorderModifiersChanged(
                    crate::keymap::Modifiers::from_iced(*modifiers),
                )),
            )),
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
                match KeyStroke::from_iced(key, *modifiers) {
                    Some(stroke) => Claim::Consume(Message::Preferences(PreferencesMsg::Inner(
                        crate::preferences::PrefMsg::KeymapRecorderKeyPressed(stroke),
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

    /// Command palette. Captures most input while open so typing into
    /// the search field doesn't fire tool shortcuts (`p`, `w`, `l`, …).
    /// Only navigation and dismiss keys leak through.
    fn claim_command_palette(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        // Phase 1 ports this window-blind. Phase 2 (#555) gates it on
        // `target.paints_overlay_stack()`.
        let _ = target;
        if !self.ui_state.command_palette.open {
            return Claim::Pass;
        }
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return Claim::Swallow;
        };
        match (key.as_ref(), *modifiers) {
            (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                Claim::Consume(Message::CommandPalette(CommandPaletteMsg::Close))
            }
            (keyboard::Key::Named(keyboard::key::Named::ArrowDown), _) => {
                Claim::Consume(Message::CommandPalette(CommandPaletteMsg::MoveSelection(1)))
            }
            (keyboard::Key::Named(keyboard::key::Named::ArrowUp), _) => Claim::Consume(
                Message::CommandPalette(CommandPaletteMsg::MoveSelection(-1)),
            ),
            // Toggle: Ctrl+Shift+P while open closes.
            (keyboard::Key::Character(c), m)
                if c.eq_ignore_ascii_case("p") && m.command() && m.shift() =>
            {
                Claim::Consume(Message::CommandPalette(CommandPaletteMsg::Close))
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
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                Claim::Consume(Message::EscapePressed {
                    window: Some(window),
                })
            }
            _ => Claim::Pass,
        }
    }

    /// F1 toggles the keyboard-shortcuts sheet: open if closed, close if
    /// open. Stays hardcoded because it depends on which modal is open —
    /// app state, not the keymap profile.
    fn claim_shortcuts_sheet(&self, target: InputTarget, event: &keyboard::Event) -> Claim {
        // Phase 1 ports this window-blind. Phase 2 (#555) decides what
        // F1 does in a window that paints no overlay stack.
        let _ = target;
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return Claim::Pass;
        };
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::F1) => {
                Claim::Consume(if self.ui_state.keyboard_shortcuts_open {
                    Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)
                } else {
                    Message::Menu(MenuMessage::OpenKeyboardShortcuts)
                })
            }
            _ => Claim::Pass,
        }
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
            return Claim::Consume(Message::Selection(
                crate::app::selection_request::SelectionRequest::StoreSlot { slot },
            ));
        }
        if m.alt() && !m.command() {
            return Claim::Consume(Message::Selection(
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
    /// press) is ignored. Being last, `Pass` and `Swallow` are equivalent
    /// here — `Pass` says the honest thing: nobody wanted it.
    fn claim_keymap(event: &keyboard::Event) -> Claim {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return Claim::Pass;
        };
        match KeyStroke::from_iced(key, *modifiers) {
            Some(stroke) => Claim::Consume(Message::Ui(UiMsg::KeymapStroke(stroke))),
            None => Claim::Pass,
        }
    }
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
            InputConsumer::Escape,
            InputConsumer::ShortcutsSheet,
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

        let open = |app: &mut Signex, kind: WindowKind| {
            let id = iced::window::Id::unique();
            app.ui_state.windows.insert(id, kind);
            id
        };
        let modal = open(&mut app, WindowKind::DetachedModal(ModalId::ErcDialog));
        let tab = open(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );
        let panel = open(
            &mut app,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );
        let editor = open(
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
                Some(Message::EscapePressed { window: Some(id) }) if id == w
            ),
            "the ladder resolves in `escape.rs` against live state"
        );
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
                Some(Message::Ui(UiMsg::KeymapStroke(_)))
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
            Some(Message::Ui(UiMsg::KeymapStroke(_)))
        ));
    }

    // ── Consumer 6: the keymap fallback ─────────────────────────────

    #[test]
    fn an_ordinary_stroke_reaches_the_keymap_resolver() {
        let app = quiet_app();
        let w = main_window(&app);

        assert!(matches!(
            app.route_key(w, &character("w", keyboard::Modifiers::default())),
            Some(Message::Ui(UiMsg::KeymapStroke(_)))
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
