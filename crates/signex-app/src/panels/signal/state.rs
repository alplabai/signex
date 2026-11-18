//! View-only state of the Signex AI panel.

/// Which Signex AI surface the segmented control shows. It changes the copy of
/// the empty state only; no AI backend exists yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignalMode {
    #[default]
    Assistant,
    Generator,
}

impl SignalMode {
    pub const ALL: [(SignalMode, &'static str); 2] = [
        (SignalMode::Assistant, "Assistant"),
        (SignalMode::Generator, "Generator"),
    ];

    /// The mode's name on the switch.
    pub fn label(self) -> signex_i18n::MessageId {
        match self {
            SignalMode::Assistant => signex_i18n::ids::PANELS_SIGNAL_MODE_ASSISTANT,
            SignalMode::Generator => signex_i18n::ids::PANELS_SIGNAL_MODE_GENERATOR,
        }
    }

    /// The mode after `msg`.
    pub fn after(self, msg: SignalMsg) -> Self {
        match msg {
            SignalMsg::ModeSelected(mode) => mode,
        }
    }
}

/// Events the Signex AI panel emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalMsg {
    ModeSelected(SignalMode),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_a_mode_selects_it() {
        let mode = SignalMode::default();
        assert_eq!(mode, SignalMode::Assistant);
        assert_eq!(
            mode.after(SignalMsg::ModeSelected(SignalMode::Generator)),
            SignalMode::Generator
        );
    }

    /// #2059: each mode reads in en-US as its name did before the catalog.
    #[test]
    fn each_mode_keeps_its_english_name() {
        let en = signex_i18n::I18n::en_us();
        let name = |mode: SignalMode| en.message_plain(mode.label(), &[]);
        assert_eq!(name(SignalMode::Assistant), "Assistant");
        assert_eq!(name(SignalMode::Generator), "Generator");
    }
}
