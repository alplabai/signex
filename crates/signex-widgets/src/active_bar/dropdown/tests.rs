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

/// A row built without the new builders is an ordinary row: not current,
/// no leading check, no hint; its fill is the hover wash or the open
/// submenu highlight, as before.
#[test]
fn a_default_row_has_no_current_look_lead_check_or_hint() {
    let item = DropdownItem::new("Place", ());
    assert!(!item.current && item.lead_check.is_none() && item.hint.is_none());
    let look = Look::of(&theme_tokens(ThemeId::GraphiteDark));
    assert_eq!(
        row_fill(button::Status::Active, look, false, false, false),
        None
    );
    assert_eq!(
        row_fill(button::Status::Hovered, look, false, false, false),
        Some(look.hover)
    );
    assert_eq!(
        row_fill(button::Status::Active, look, false, true, false),
        Some(look.active)
    );
    assert!(!metrics::has_icons(&labelled("Place")));
}

/// The current row has the accent-soft fill and ink in both Graphite
/// themes, and a hover still washes over it.
#[test]
fn the_current_row_is_accent_soft_and_still_hovers() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        let look = Look::of(&tokens);
        assert_eq!(look.current_fill, theme_ext::accent_soft(&tokens));
        assert_eq!(look.current_ink, theme_ext::accent_soft_ink(&tokens));
        let rest = row_fill(button::Status::Active, look, false, false, true);
        assert_eq!(rest, Some(look.current_fill), "{id:?}");
        let hovered = row_fill(button::Status::Hovered, look, false, false, true);
        assert!(hovered.is_some() && hovered != rest && hovered != Some(look.hover));
    }
    let item = DropdownItem::new("Polygon Pour", ()).current(true);
    assert!(item.current);
}

/// A leading check reserves the icon column (on or off) and replaces the
/// trailing check, so it does not widen the card.
#[test]
fn a_lead_check_row_takes_the_icon_column_not_the_trailing_check() {
    let m = MenuMetrics::GRAPHITE;
    let off = vec![DropdownEntry::Item(
        DropdownItem::new("Signal Layers", ()).lead_check(false),
    )];
    assert!(
        metrics::has_icons(&off),
        "an empty column is still a column"
    );
    // Wide enough to clear the 200 px floor, narrow enough for 320 px, so
    // the widths compare unclamped.
    const WIDE: &str = "Assembly Drawings as SVG Files";
    let on = DropdownItem::new(WIDE, ()).lead_check(true);
    assert_eq!(on.lead_check, Some(true));
    let plain = m.fitted_width(&[DropdownEntry::Item(on)], f32::MAX, "");
    let checked = m.fitted_width(
        &[DropdownEntry::Item(
            DropdownItem::new(WIDE, ()).lead_check(true).checked(true),
        )],
        f32::MAX,
        "",
    );
    assert_eq!(plain, checked, "no trailing check beside a leading one");
    let iconless = m.fitted_width(&labelled(WIDE), f32::MAX, "");
    assert!(iconless > m.min_width, "{iconless} clears the floor");
    assert!(plain > iconless, "{plain} {iconless}");
}

/// A hint is part of the fitted width, measured at the caption size.
#[test]
fn a_hint_widens_the_fitted_card() {
    let m = MenuMetrics::GRAPHITE;
    assert_eq!(m.hint_size, theme_ext::TypeScale::DEFAULT.caption);
    let label = "Find Similar Objects";
    let plain = m.fitted_width(&labelled(label), f32::MAX, "");
    let hinted = m.fitted_width(
        &[DropdownEntry::Item(
            DropdownItem::new(label, ()).hint("not available on boards yet"),
        )],
        f32::MAX,
        "",
    );
    assert!(hinted > plain, "{hinted} {plain}");
}

/// Compositing a translucent wash over a fill keeps the fill visible.
#[test]
fn a_wash_over_a_fill_blends_the_two() {
    let base = Color::from_rgb(1.0, 0.0, 0.0);
    let wash = Color::from_rgba(0.0, 0.0, 1.0, 0.5);
    let mixed = over(wash, base);
    assert_eq!(mixed.a, 1.0);
    assert!((mixed.r - 0.5).abs() < 1e-6 && (mixed.b - 0.5).abs() < 1e-6);
    assert_eq!(
        over(Color::TRANSPARENT, Color::TRANSPARENT),
        Color::TRANSPARENT
    );
}
