use super::*;
use signex_types::theme::{ThemeId, theme_tokens};

#[test]
fn disabled_icon_is_tinted_muted() {
    let muted = Color::from_rgb(0.5, 0.5, 0.5);
    assert_eq!(icon_style(true, muted, Color::BLACK).color, Some(muted));
}

/// The Graphite look draws every row icon in one ink, so a menu's icon
/// column reads monochrome.
#[test]
fn graphite_icons_are_monochrome() {
    let graphite = Look::of(&theme_tokens(ThemeId::GraphiteLight));
    let ink = graphite.icon_ink;
    assert_eq!(icon_style(false, graphite.muted, ink).color, Some(ink));
}

/// The fork's `Shrink` compresses contents to their minimum, which
/// collapsed menu rows to boxes without labels; the panel sizes to its
/// content with `Fit` (#2043).
#[test]
fn panel_without_a_width_hint_fits_its_content() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let entries = vec![DropdownEntry::Item(DropdownItem::new("Place", ()))];
    let panel = view(entries, &tokens, None);
    use iced::advanced::Widget as _;
    assert_eq!(panel.size().width, Length::Fit);
}

/// Width of a card of `entries` under the Graphite metrics.
fn fit(entries: &[DropdownEntry<()>], max_w: f32, ui_family: &str) -> f32 {
    MenuMetrics::GRAPHITE.fitted_width(entries, max_w, ui_family)
}

fn labelled(label: &str) -> Vec<DropdownEntry<()>> {
    vec![DropdownEntry::Item(DropdownItem::new(label, ()))]
}

/// A context menu fits its labels: narrower than the cap, wider for a
/// longer label, and held at the cap (#2100).
#[test]
fn a_fitted_menu_is_narrower_than_the_cap_and_grows_with_its_label() {
    let short = fit(&labelled("Copy"), 260.0, "");
    let long = fit(&labelled("Copy Designator And Value"), 260.0, "");
    assert!(short < 260.0 && short < long, "{short} {long}");
    assert_eq!(fit(&labelled(&"W".repeat(200)), 260.0, ""), 260.0);
}

/// Graphite cards are fitted to their content between 200 and 320 px.
#[test]
fn a_graphite_card_is_fitted_between_200_and_320_px() {
    let tiny = fit(&labelled("Cut"), f32::MAX, "Inter");
    let huge = fit(&labelled(&"W".repeat(200)), f32::MAX, "Inter");
    let mid = fit(&labelled("Find Similar Objects Of This Kind"), f32::MAX, "");
    assert_eq!(tiny, 200.0);
    assert_eq!(huge, 320.0);
    assert!(mid > 200.0 && mid < 320.0, "{mid}");
    // A caller's own cap still wins.
    assert_eq!(fit(&labelled(&"W".repeat(200)), 250.0, ""), 250.0);
}

/// A shortcut and a chevron column widen the fitted card.
#[test]
fn the_shortcut_and_chevron_columns_are_part_of_the_fitted_width() {
    let m = MenuMetrics::GRAPHITE;
    let plain = m.fitted_width(&labelled("Find Similar Objects"), f32::MAX, "");
    let with_shortcut = m.fitted_width(
        &[DropdownEntry::Item(
            DropdownItem::new("Find Similar Objects", ()).shortcut("Shift+F"),
        )],
        f32::MAX,
        "",
    );
    let with_chevron = m.fitted_width(
        &[DropdownEntry::Item(
            DropdownItem::new("Find Similar Objects", ()).submenu(),
        )],
        f32::MAX,
        "",
    );
    assert!(with_shortcut > plain, "{with_shortcut} {plain}");
    assert!(with_chevron > plain, "{with_chevron} {plain}");
}

/// Graphite rows are dense: the shared menu label token on 28 px rows, with
/// or without icons.
#[test]
fn graphite_rows_are_28_px_with_dense_type() {
    let m = MenuMetrics::GRAPHITE;
    assert_eq!(m, MenuMetrics::GRAPHITE);
    assert_eq!(m.item_height(false), 28.0);
    assert_eq!(m.item_height(true), 28.0);
    assert_eq!(m.label_size, super::MENU_LABEL_SIZE);
    assert_eq!(m.shortcut_size, super::MENU_SHORTCUT_SIZE);
    assert_eq!(m.icon_size, 16.0);
    assert!(m.mono_shortcut);
}

/// The open launcher's top is the sum of the rows above it, from the card
/// top; no open launcher gives none.
#[test]
fn the_open_launcher_top_counts_the_rows_and_separators_above_it() {
    let m = MenuMetrics::GRAPHITE;
    let entries = vec![
        DropdownEntry::Item(DropdownItem::new("Alpha", ())),
        DropdownEntry::Separator,
        DropdownEntry::Item(DropdownItem::new("Place", ()).submenu().active(true)),
    ];
    assert_eq!(
        m.active_submenu_top(&entries),
        Some(m.panel_pad + m.item_height(false) + m.separator_height())
    );
    let closed = vec![DropdownEntry::Item(
        DropdownItem::new("Place", ()).submenu(),
    )];
    assert_eq!(m.active_submenu_top(&closed), None);
    assert_eq!(
        m.card_height(&entries),
        Some(2.0 * m.panel_pad + 2.0 * m.item_height(false) + m.separator_height())
    );
}
