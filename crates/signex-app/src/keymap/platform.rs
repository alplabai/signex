//! The operating system a shortcut is resolved and shown for.
//!
//! Tests pin a platform instead of reading the host, so every OS's rule is
//! checked on every CI runner.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[serde(rename = "macos")]
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    /// The OS this build runs on. Every non-macOS, non-Windows target is
    /// treated as Linux.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    /// What this OS calls the logo key.
    pub const fn logo_key_name(self) -> &'static str {
        match self {
            Self::MacOs => "Cmd",
            Self::Windows => "Win",
            Self::Linux => "Super",
        }
    }
}
