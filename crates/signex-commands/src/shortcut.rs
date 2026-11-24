//! Where a shortcut applies and the modifiers it holds. Converting a toolkit
//! key event into [`Modifiers`] is the front end's job (`from_iced` in the
//! app).

use serde::{Deserialize, Serialize};

use crate::Platform;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutContext {
    #[default]
    Global,
    Schematic,
    Footprint,
    Pcb,
    Library,
    Modal,
    TextInput,
    CommandPalette,
    Placement,
    /// The board 3D view of the main window (mockup row 175): live with
    /// `Pcb` while the view is shown, and it outranks it.
    #[serde(rename = "board_3d")]
    Board3d,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub command: bool,
    /// The `Mod` token of a profile: the platform's standard modifier,
    /// Cmd on macOS and Ctrl elsewhere (#1645). Only parsed or recorded
    /// strokes carry it; a live keystroke never does. Spelling is kept so a
    /// saved profile reads `Mod+…` on every OS. Compare strokes through
    /// `KeyStroke::resolved_for`, never raw.
    #[serde(default)]
    pub primary: bool,
}

impl Modifiers {
    /// `primary` replaced by the concrete key it means on `platform`: Cmd
    /// on macOS, Ctrl elsewhere, never Super. Idempotent.
    pub fn resolved_for(self, platform: Platform) -> Self {
        if !self.primary {
            return self;
        }
        let mut resolved = Self {
            primary: false,
            ..self
        };
        match platform {
            Platform::MacOs => resolved.command = true,
            Platform::Windows | Platform::Linux => resolved.ctrl = true,
        }
        resolved
    }

    /// The form the Preferences recorder stores: on macOS a Cmd press is
    /// `Mod`, so a profile made on a Mac also works elsewhere. Other OSes
    /// record exactly what was pressed.
    pub fn recorded_for(self, platform: Platform) -> Self {
        if platform == Platform::MacOs && self.command {
            return Self {
                command: false,
                primary: true,
                ..self
            };
        }
        self
    }

    /// The logo key's spelling in profile files, on every OS.
    pub const CONFIG_COMMAND_NAME: &'static str = "Cmd";

    /// What this OS calls the logo key: Cmd on macOS, Win on Windows,
    /// Super elsewhere. The parser accepts all three spellings on every OS.
    pub fn command_key_name() -> &'static str {
        Platform::current().logo_key_name()
    }

    pub fn is_empty(self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.command && !self.primary
    }
}
