//! Keeps `docs/USER_GUIDE.md` honest against the shipped UI (#322).
//!
//! The guide names menu items and panel labels. A rename in the code that
//! leaves the guide behind sends a first-time user looking for a control that
//! is no longer there, so every label the walkthrough relies on must still
//! exist in the menu source, and the guide must not repeat caveats that have
//! been resolved.

const GUIDE: &str = include_str!("../../../../docs/USER_GUIDE.md");
const PANELS_FTL: &str = include_str!("../../../signex-i18n/locales/en-US/panels.ftl");

/// Menu labels the guide's walkthrough tells the user to click, as spelled in
/// the menu model (ASCII `...`; the guide writes the ellipsis character).
const WALKTHROUGH_MENU_LABELS: &[&str] = &[
    "New Project",
    "Open Library...",
    "Place Component...",
    "Net Label",
    "Annotate Schematics Quietly",
    "Electrical Rules Check",
    "Save",
];

#[test]
fn guide_has_end_to_end_walkthrough() {
    assert!(
        GUIDE.contains("## Walkthrough: a saved schematic with a wired symbol"),
        "the guide needs the end-to-end walkthrough section (#322)"
    );
}

#[test]
fn guide_drops_resolved_stock_library_caveat() {
    // #328 shipped: the stock library mounts on every launch.
    assert!(
        !GUIDE.contains("is also not shipped yet"),
        "the stock library caveat is stale; #328 is closed"
    );
    assert!(
        GUIDE.contains("**Stock**"),
        "the guide must describe the Stock section of the Components panel"
    );
}

/// Every row label of the menu bar as the en-US model shows it: the labels
/// are catalog text, no longer literals in the menu source.
fn menu_labels() -> Vec<String> {
    use signex_app::menu_bar::MenuContext;
    use signex_app::menu_bar::model::{Menu, MenuEntry, menu_bar_model};
    fn collect(menu: &Menu, out: &mut Vec<String>) {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Item(item) => out.push(item.label.clone()),
                MenuEntry::Submenu(sub) => collect(sub, out),
                MenuEntry::Separator => {}
            }
        }
    }
    let mut labels = Vec::new();
    for menu in &menu_bar_model(&MenuContext::default()).menus {
        collect(menu, &mut labels);
    }
    labels
}

#[test]
fn walkthrough_menu_labels_exist_in_the_menu_source() {
    let menu_labels = menu_labels();
    for label in WALKTHROUGH_MENU_LABELS {
        assert!(
            GUIDE.contains(&label.replace("...", "\u{2026}")) || GUIDE.contains(label),
            "guide does not mention {label:?}"
        );
        assert!(
            menu_labels.iter().any(|shown| shown == label),
            "menu source no longer has {label:?}; update docs/USER_GUIDE.md"
        );
    }
}

#[test]
fn guide_stock_section_matches_the_panel() {
    // The panel's text lives in its en-US catalog since #2225.
    assert!(PANELS_FTL.contains("Stock library not found"));
    assert!(GUIDE.contains("Stock library not found"));
}

#[test]
fn guide_describes_placing_and_has_no_merge_leftovers() {
    // The trunk merge for #1160 once produced a guide that said placement
    // does not work yet next to the steps that make it work.
    assert!(
        !GUIDE.contains("No menu item or button reaches it yet"),
        "the guide still says placement is not wired"
    );
    assert!(GUIDE.contains("press **Place** at the end of a part's row"));
    for marker in ["<<<<<<< ", "||||||| ", ">>>>>>> "] {
        assert!(
            !GUIDE.contains(marker),
            "merge marker {marker:?} in the guide"
        );
    }
}
