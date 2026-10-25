//! The "Font" row of the Properties panel: the installed families plus
//! "Default" (inherit the sheet default). Shared by every text-bearing
//! selection and by the sheet-wide default in Page Options (#510).

use super::super::*;

/// The pick-list entry meaning "no family of its own".
pub(in crate::panels) const DEFAULT_OPTION: &str = "Default";

/// The entries to offer: "Default", every installed family, and the
/// stored family itself when this machine does not have it, so the row
/// shows what the document names rather than hiding it.
pub(in crate::panels) fn font_family_options(current: Option<&str>) -> Vec<String> {
    let installed = crate::fonts::system_font_families();
    let mut options = Vec::with_capacity(installed.len() + 2);
    options.push(DEFAULT_OPTION.to_string());
    options.extend(installed.iter().cloned());
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
    form_pick_row(
        label,
        font_family_options(current),
        current.unwrap_or(DEFAULT_OPTION).to_string(),
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
        let options = font_family_options(Some("Surely Not Installed 510"));
        assert_eq!(options[0], DEFAULT_OPTION);
        assert!(options.contains(&"Surely Not Installed 510".to_string()));
        assert!(
            options.contains(&"Iosevka".to_string()),
            "the bundled family is always offered"
        );
    }
}
