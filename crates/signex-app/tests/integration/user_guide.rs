//! Keeps `docs/USER_GUIDE.md` honest against the shipped UI (#322).
//!
//! The guide names menu items and panel labels. A rename in the code that
//! leaves the guide behind sends a first-time user looking for a control that
//! is no longer there, so every label the walkthrough relies on must still
//! exist in the menu source, and the guide must not repeat caveats that have
//! been resolved.

const GUIDE: &str = include_str!("../../../../docs/USER_GUIDE.md");
const MENU_SRC: &str = include_str!("../../src/menu_bar/view.rs");
const PANEL_SRC: &str = include_str!("../../src/panels/components_panel/mod.rs");

/// Menu labels the guide's walkthrough tells the user to click, as spelled in
/// `menu_bar/view.rs` (ASCII `...`; the guide writes the ellipsis character).
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

#[test]
fn walkthrough_menu_labels_exist_in_the_menu_source() {
    for label in WALKTHROUGH_MENU_LABELS {
        assert!(
            GUIDE.contains(&label.replace("...", "\u{2026}")) || GUIDE.contains(label),
            "guide does not mention {label:?}"
        );
        assert!(
            MENU_SRC.contains(label),
            "menu source no longer has {label:?}; update docs/USER_GUIDE.md"
        );
    }
}

#[test]
fn guide_stock_section_matches_the_panel() {
    assert!(PANEL_SRC.contains("Stock library not found"));
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
