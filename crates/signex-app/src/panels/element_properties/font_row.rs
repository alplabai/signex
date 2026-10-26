//! The "Font" row of the Properties panel: the installed families plus
//! "Default" (inherit the sheet default). Shared by every text-bearing
//! selection and by the sheet-wide default in Page Options (#510).

use super::super::*;

/// The pick-list entry meaning "no family of its own".
pub(in crate::panels) const DEFAULT_OPTION: &str = "Default";

/// "Default" and every installed family, built once per catalog. This runs
/// in `view`, so it never scans: until the background scan lands only the
/// bundled families are offered, and the scan's completion message triggers
/// the redraw that shows the rest.
fn base_options() -> Vec<String> {
    use crate::fonts::catalog;
    static SCANNED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    match catalog::installed() {
        Some(scanned) => SCANNED.get_or_init(|| options_of(scanned)).clone(),
        None => options_of(catalog::installed_or_bundled()),
    }
}

fn options_of(catalog: &crate::fonts::catalog::FontCatalog) -> Vec<String> {
    std::iter::once(DEFAULT_OPTION.to_string())
        .chain(catalog.names().iter().cloned())
        .collect()
}

/// The entries to offer: "Default", every installed family, and the
/// stored family itself when this machine does not have it, so the row
/// shows what the document names rather than hiding it.
pub(in crate::panels) fn font_family_options(current: Option<&str>) -> Vec<String> {
    with_current(base_options(), current)
}

fn with_current(mut options: Vec<String>, current: Option<&str>) -> Vec<String> {
    if let Some(current) = current
        && !options.iter().any(|name| name == current)
    {
        options.push(current.to_string());
    }
    options
}

/// `Default` is `None`; any other entry is the family name.
pub(in crate::panels) fn family_from_option(option: String) -> Option<String> {
    (option != DEFAULT_OPTION).then_some(option)
}

pub(in crate::panels) fn font_family_row<'a>(
    label: &str,
    current: Option<&str>,
    muted: Color,
    on_pick: impl Fn(Option<String>) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    // Show the installed spelling when the document spells it differently.
    let shown = current.map(|name| {
        crate::fonts::catalog::installed_or_bundled()
            .canonical(name)
            .unwrap_or(name)
    });
    form_pick_row(
        label,
        font_family_options(shown),
        shown.unwrap_or(DEFAULT_OPTION).to_string(),
        move |option| on_pick(family_from_option(option)),
        muted,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_option_means_inherit_and_any_other_entry_is_the_family() {
        assert_eq!(family_from_option("Default".into()), None);
        assert_eq!(
            family_from_option("Fira Mono".into()),
            Some("Fira Mono".into())
        );
    }

    #[test]
    fn a_stored_family_this_machine_lacks_is_still_offered() {
        let catalog = crate::fonts::catalog::FontCatalog::from_families(["Fira Mono"]);
        let options = with_current(options_of(&catalog), Some("Surely Not Installed 510"));
        assert_eq!(options[0], DEFAULT_OPTION);
        assert!(options.contains(&"Surely Not Installed 510".to_string()));
        assert!(options.contains(&"Fira Mono".to_string()));
        assert!(
            options.contains(&"Iosevka".to_string()),
            "the bundled family is always offered"
        );
    }

    /// `view` calls this on every frame; it must never trigger the blocking
    /// system-font scan.
    #[test]
    fn building_the_options_does_not_scan_the_system_fonts() {
        let scanned_before = crate::fonts::catalog::installed().is_some();
        let options = font_family_options(None);
        assert_eq!(
            crate::fonts::catalog::installed().is_some(),
            scanned_before,
            "font_family_options started the scan"
        );
        assert!(options.contains(&"Iosevka".to_string()));
    }
}
