//! Shortcut labels in the message language (#2314): modifier and key words
//! come from the catalog, the macOS symbols are language-neutral. Profile
//! files never use these; they keep [`KeyStroke::to_config_string`].

use crate::keymap::{KeyStroke, KeyToken, Modifiers, Platform, ShortcutTrigger};
use signex_i18n::{I18n, MessageId, key_ids};

/// One key word, plain: labels are stored, split on `+` and measured.
fn word(i18n: &I18n, id: MessageId) -> String {
    i18n.message_plain(id, &[])
}

impl Platform {
    /// The catalog id of what this OS calls the logo key.
    pub const fn logo_key_id(self) -> MessageId {
        match self {
            Self::MacOs => key_ids::KEY_CMD,
            Self::Windows => key_ids::KEY_WIN,
            Self::Linux => key_ids::KEY_SUPER,
        }
    }
}

impl Modifiers {
    /// [`Modifiers::display_for`] with the words in the language of `i18n`.
    pub fn display_in(self, platform: Platform, i18n: &I18n) -> String {
        let m = self.resolved_for(platform);
        if platform == Platform::MacOs {
            return [
                (m.ctrl, '\u{2303}'),
                (m.alt, '\u{2325}'),
                (m.shift, '\u{21e7}'),
                (m.command, '\u{2318}'),
            ]
            .into_iter()
            .filter_map(|(on, symbol)| on.then_some(symbol))
            .collect();
        }
        [
            (m.ctrl, key_ids::KEY_CTRL),
            (m.command, platform.logo_key_id()),
            (m.alt, key_ids::KEY_ALT),
            (m.shift, key_ids::KEY_SHIFT),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, id)| word(i18n, id))
        .collect::<Vec<_>>()
        .join("+")
    }
}

impl KeyToken {
    /// The key as a label shows it in the language of `i18n`. A character
    /// or function key is the same in every language.
    pub fn display_in(&self, i18n: &I18n) -> String {
        let id = match self {
            Self::Character(_) | Self::Function(_) => return self.to_string(),
            Self::Escape => key_ids::KEY_ESC,
            Self::Enter => key_ids::KEY_ENTER,
            Self::Tab => key_ids::KEY_TAB,
            Self::Space => key_ids::KEY_SPACE,
            Self::Backspace => key_ids::KEY_BACKSPACE,
            Self::Delete => key_ids::KEY_DEL,
            Self::Insert => key_ids::KEY_INS,
            Self::Home => key_ids::KEY_HOME,
            Self::End => key_ids::KEY_END,
            Self::PageUp => key_ids::KEY_PGUP,
            Self::PageDown => key_ids::KEY_PGDN,
            Self::ArrowUp => key_ids::KEY_UP,
            Self::ArrowDown => key_ids::KEY_DOWN,
            Self::ArrowLeft => key_ids::KEY_LEFT,
            Self::ArrowRight => key_ids::KEY_RIGHT,
        };
        word(i18n, id)
    }
}

impl KeyStroke {
    /// [`KeyStroke::display_for`] with the words in the language of `i18n`.
    pub fn display_in(&self, platform: Platform, i18n: &I18n) -> String {
        let stroke = self.resolved_for(platform);
        let mut text = stroke.modifiers.display_in(platform, i18n);
        if platform != Platform::MacOs && !text.is_empty() {
            text.push('+');
        }
        text.push_str(&stroke.key.display_in(i18n));
        text
    }
}

impl ShortcutTrigger {
    /// [`ShortcutTrigger::display_text_for`] with the words in the language
    /// of `i18n`. A pointer gesture is shown as written.
    pub fn display_text_in(&self, platform: Platform, i18n: &I18n) -> String {
        match self {
            Self::KeySequence(strokes) => strokes
                .iter()
                .map(|stroke| stroke.display_in(platform, i18n))
                .collect::<Vec<_>>()
                .join(" "),
            Self::PointerGesture(gesture) => gesture.clone(),
        }
    }
}
