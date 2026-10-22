//! Help-menu facts: the version block, the licence line and the external
//! pages the Help menu and the About dialog link to.
//!
//! Pure data and one URL opener — no iced state — so the
//! About dialog, the Help menu handlers and the tests all read the same
//! values.

/// Product version, from the crate manifest (`workspace.package.version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// SPDX identifier shown on the About card.
pub const LICENSE_NAME: &str = "Apache-2.0";

/// Repository root, from the crate manifest.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// An external page reachable from Help ▸ Documentation, Help ▸ Report an
/// Issue and the About dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLink {
    /// The user guide (`docs/` in the repository).
    Documentation,
    /// The issue tracker.
    IssueTracker,
    /// The `LICENSE` file.
    License,
}

impl ExternalLink {
    /// The page this link opens.
    pub fn url(self) -> String {
        match self {
            Self::Documentation => format!("{REPOSITORY}/tree/trunk/docs"),
            Self::IssueTracker => format!("{REPOSITORY}/issues"),
            Self::License => format!("{REPOSITORY}/blob/trunk/LICENSE"),
        }
    }
}

/// Plain-text block the About dialog's "Copy version info" button puts on
/// the clipboard — what a bug report needs first.
pub fn version_info_text() -> String {
    format!(
        "Signex {VERSION}\nLicense: {LICENSE_NAME}\nOS: {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Open `link` in the default browser without blocking `update`. A launch
/// failure is turned into a message by `on_failed` (given the
/// user-readable reason) so the caller can put it on the error card.
pub fn open_link<M: Send + 'static>(
    link: ExternalLink,
    on_failed: impl FnOnce(String) -> M + Send + 'static,
) -> iced::Task<M> {
    let url = link.url();
    iced::Task::future(async move {
        let result = tokio::task::spawn_blocking({
            let url = url.clone();
            move || webbrowser::open(&url)
        })
        .await;
        match result {
            Ok(Ok(())) => None,
            Ok(Err(err)) => Some(on_failed(format!(
                "Could not open {url} in a browser: {err}"
            ))),
            Err(err) => Some(on_failed(format!(
                "The browser launch for {url} did not complete: {err}"
            ))),
        }
    })
    .then(|message| match message {
        Some(message) => iced::Task::done(message),
        None => iced::Task::none(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_the_workspace_manifest() {
        let manifest = include_str!("../../../Cargo.toml");
        let line = format!("version = \"{VERSION}\"");
        assert!(
            manifest.contains(&line),
            "About version {VERSION} must equal workspace.package.version"
        );
    }

    #[test]
    fn version_info_names_version_licence_and_os() {
        let text = version_info_text();
        assert!(text.contains(VERSION));
        assert!(text.contains(LICENSE_NAME));
        assert!(text.contains(std::env::consts::OS));
    }

    #[test]
    fn links_point_at_the_repository() {
        assert!(ExternalLink::IssueTracker.url().ends_with("/issues"));
        assert!(ExternalLink::License.url().ends_with("/LICENSE"));
        assert!(ExternalLink::Documentation.url().starts_with(REPOSITORY));
    }
}
