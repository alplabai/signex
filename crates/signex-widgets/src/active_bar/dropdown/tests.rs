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

/// A row built without the new builders is an ordinary row: not selected,
/// no leading check, no caption; its fill is the hover wash or the open
/// submenu highlight, as before.
#[test]
fn a_default_row_is_not_selected_and_has_no_lead_check_or_caption() {
    let item = DropdownItem::new("Place", ());
    assert!(!item.selected && item.lead_check.is_none() && item.caption.is_none());
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

/// The current choice reads in the accent-soft tokens of both themes, and
/// keeps its fill under the pointer (a hover does not wash it out).
#[test]
fn the_selected_row_uses_the_accent_soft_tokens() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        let look = Look::of(&tokens);
        let soft = tokens
            .extended
            .accent_soft
            .as_ref()
            .expect("graphite token");
        let ink = tokens
            .extended
            .accent_soft_ink
            .as_ref()
            .expect("graphite token");
        assert_eq!(look.selected_fill, theme_ext::to_color(soft), "{id:?}");
        assert_eq!(look.selected_ink, theme_ext::to_color(ink), "{id:?}");
        for status in [button::Status::Active, button::Status::Hovered] {
            assert_eq!(
                row_fill(status, look, false, false, true),
                Some(look.selected_fill),
                "{id:?} {status:?}"
            );
        }
    }
    let item = DropdownItem::new("Polygon Pour", ()).selected(true);
    assert!(item.selected);
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

/// A caption is measured like a shortcut: the card widens for it.
#[test]
fn a_caption_widens_the_fitted_card() {
    let long = "W".repeat(18);
    let bare = fit(&labelled(&long), f32::MAX, "");
    let captioned = fit(
        &[DropdownEntry::Item(
            DropdownItem::new(long.as_str(), ()).caption("0.0254 mm"),
        )],
        f32::MAX,
        "",
    );
    assert!(captioned > bare, "{captioned} {bare}");
}

/// A swatch row reserves the icon column like an icon row, so a card mixing
/// the two starts every label at one x, and the swatch fits inside it.
#[test]
fn a_swatch_row_takes_the_icon_column() {
    let swatch_only = vec![DropdownEntry::Item(
        DropdownItem::new("Blue", ()).swatch(Color::from_rgb(0.4, 0.4, 0.93)),
    )];
    assert!(metrics::has_icons(&swatch_only));
    assert!(!metrics::has_icons(&labelled("Blue")));
    let m = MenuMetrics::GRAPHITE;
    assert!(m.swatch_size() > 0.0 && m.swatch_size() < m.icon_size);
}
