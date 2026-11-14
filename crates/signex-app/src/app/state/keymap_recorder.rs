//! Chord-recorder state of the Preferences keyboard pane.

/// Chord-recorder overlay state for the Preferences ▸ Keyboard
/// Shortcuts pane. Holds the binding being edited plus the strokes
/// captured so far. The keyboard subscription feeds raw key events
/// here (as `PrefMsg::KeymapRecorderKeyPressed`) while this is `Some`,
/// so recording never triggers a live command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapRecorderState {
    /// The binding trigger being re-recorded (#1378).
    pub slot: crate::keymap::BindingSlot,
    pub command: crate::keymap::AppCommandId,
    pub command_label: String,
    pub context: crate::keymap::ShortcutContext,
    /// The binding's trigger before recording, in profile-file spelling.
    pub original_config: String,
    pub strokes: Vec<crate::keymap::KeyStroke>,
    pub modifiers: crate::keymap::Modifiers,
    pub recording: bool,
}

impl KeymapRecorderState {
    /// Longest chord the recorder will hold before wrapping back to a
    /// single stroke (Altium-style two/three-key gestures fit inside).
    pub const MAX_STROKES: usize = 3;

    pub fn new(
        slot: crate::keymap::BindingSlot,
        command: crate::keymap::AppCommandId,
        command_label: String,
        context: crate::keymap::ShortcutContext,
        trigger: String,
    ) -> Self {
        // Seed the capture buffer with the binding's current key
        // sequence so the user sees what they are replacing. Pointer
        // gestures aren't keyboard-recordable, so they start empty.
        // `trigger` is the binding's profile-file spelling, which parses back
        // to the same strokes (display text on macOS would not).
        let parsed = crate::keymap::ShortcutTrigger::parse(&trigger).ok();
        let strokes = parsed
            .and_then(|trigger| match trigger {
                crate::keymap::ShortcutTrigger::KeySequence(strokes) => Some(strokes),
                crate::keymap::ShortcutTrigger::PointerGesture(_) => None,
            })
            .unwrap_or_default();
        Self {
            slot,
            command,
            command_label,
            context,
            original_config: trigger,
            strokes,
            modifiers: crate::keymap::Modifiers::default(),
            recording: true,
        }
    }

    /// The trigger before recording as this OS shows it in the language of
    /// `i18n`; one that does not parse is shown as written.
    pub fn original_trigger_in(&self, i18n: &signex_i18n::I18n) -> String {
        crate::keymap::shown_in(&self.original_config, i18n)
    }

    /// The recorded strokes as this OS shows them in the language of `i18n`.
    pub fn trigger_text_in(&self, i18n: &signex_i18n::I18n) -> String {
        self.trigger()
            .display_text_in(crate::keymap::Platform::current(), i18n)
    }

    /// The recorded strokes themselves: what Apply stores, so nothing is
    /// re-read from display text.
    pub fn trigger(&self) -> crate::keymap::ShortcutTrigger {
        crate::keymap::ShortcutTrigger::KeySequence(self.strokes.clone())
    }
}
