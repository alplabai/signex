//! Graphite look of the status bar and its popovers: sizes, styles, the
//! read-out texts and the small controls the clusters are made of. Legacy
//! themes never reach these helpers.

use iced::widget::{Text, Widget, container, row};
use iced::{Background, Border, Color, Element, Padding, Theme};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::ViewAngleReadout;
use crate::app::StatusBarRequest;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, HAIRLINE, LINE_HEIGHT, Radii, Spacing, TypeScale, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// Width of the Panels popover (the mock's 236 px menu card).
pub const POPUP_WIDTH: f32 = 236.0;
/// Inner padding of the Panels popover.
pub const POPUP_PADDING: f32 = 4.0;
/// Gap between the popover's right edge and the window edge.
pub const POPUP_RIGHT_MARGIN: f32 = 6.0;
/// Gap between a popover's bottom edge and the status bar.
pub const POPUP_GAP: f32 = 4.0;
/// Radius of the small controls in the bar (pills, Panels button).
const BAR_CONTROL_RADIUS: f32 = 5.0;

/// Padding of the bar: 12 px left, 4 px right, centred vertically.
pub const BAR_PADDING: Padding = Padding {
    top: 0.0,
    right: 4.0,
    bottom: 0.0,
    left: 12.0,
};

/// Gap between the bar's segments and around a cluster rule.
pub(super) const BAR_GAP: f32 = SP.xl;
/// Gap between the members of one cluster.
pub(super) const CLUSTER_GAP: f32 = SP.m;
/// The X column: fixed so the bar does not jitter as the number changes
/// width (the mock's 96 px).
pub(super) const X_COLUMN: f32 = TS.body * 8.0;
/// The Y column (the mock's 84 px).
pub(super) const Y_COLUMN: f32 = TS.body * 7.0;
/// The unit button after Y: room for "mil" in the mono face.
pub(super) const UNIT_COLUMN: f32 = TS.body * 2.5;
/// Height of a cluster rule.
const RULE_HEIGHT: f32 = TS.body;
/// Side of the Panels button's icon.
pub(super) const PANELS_ICON: f32 = TS.body;

/// Where the Grid read-out starts, from the window's left edge: the bar's
/// left padding, the cursor cluster and the rule after it. The Grid & Snap
/// popover is anchored here.
pub fn grid_readout_x() -> f32 {
    BAR_PADDING.left
        + X_COLUMN
        + CLUSTER_GAP
        + Y_COLUMN
        + CLUSTER_GAP
        + UNIT_COLUMN
        + BAR_GAP
        + HAIRLINE
        + BAR_GAP
}

/// Height of a Panels popover row.
pub fn popup_row_height() -> f32 {
    ChromeMetrics::DEFAULT.field
}

/// Height of a group caption in the Panels popover: the section line with
/// the menu caption's padding above and below.
pub fn popup_caption_height() -> f32 {
    popup_caption_padding().top + TS.section * LINE_HEIGHT + popup_caption_padding().bottom
}

/// Padding of a group caption: 6 px above, 2 px below, the row inset at the
/// sides (the mock's `.mh`).
pub fn popup_caption_padding() -> Padding {
    Padding {
        top: SP.s + SP.xs,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    }
}

/// Total height of a popover with `rows` rows.
pub fn popup_height(rows: usize) -> f32 {
    rows as f32 * popup_row_height() + 2.0 * POPUP_PADDING
}

/// Total height of the Panels popover: `rows` rows under `captions` group
/// captions.
pub fn panel_list_height(rows: usize, captions: usize) -> f32 {
    popup_height(rows) + captions as f32 * popup_caption_height()
}

/// The chrome mono face (coordinates, grid, snap).
pub fn mono_font() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Snap toggle: `ok_soft` fill and `ok` ink while snapping, flat and muted
/// while free.
pub fn snap_pill(
    tokens: &ThemeTokens,
    snapping: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = resolve(tokens);
    let (fill, ink) = if snapping {
        (Some(Background::Color(r.ok_soft)), ti(tokens.success))
    } else {
        (None, r.ink3)
    };
    let hover = ti(tokens.hover);
    move |_, status| button::Style {
        background: match (status, fill) {
            (button::Status::Hovered, None) => Some(Background::Color(hover)),
            _ => fill,
        },
        text_color: ink,
        border: Border {
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Active-layer pill: `accent_soft` fill, a `hover` fill under the pointer.
pub fn layer_pill(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = resolve(tokens);
    let (fill, ink, hover) = (r.accent_soft, r.accent_soft_ink, ti(tokens.hover));
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover,
            _ => fill,
        })),
        text_color: ink,
        border: Border {
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The active-layer pill as a plain label: the pill's fill, no hover.
pub fn layer_label(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = resolve(tokens);
    let (fill, ink) = (r.accent_soft, r.accent_soft_ink);
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        text_color: Some(ink),
        border: Border {
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The selection summary as a button: a sunken pill in the text ink, since
/// a click opens Properties.
pub fn selection_pill(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (sunken, ink, hover) = (resolve(tokens).sunken, ti(tokens.text), ti(tokens.hover));
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover,
            _ => sunken,
        })),
        text_color: ink,
        border: Border {
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The bordered Panels button.
pub fn panels_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (raised, line) = (resolve(tokens).raised, ti(tokens.border));
    let (hover, ink) = (ti(tokens.hover), ti(tokens.text));
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover,
            _ => raised,
        })),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: BAR_CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The popover surface: raised fill, hairline, card radius, floating shadow.
pub fn popup(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (raised, line, ink) = (resolve(tokens).raised, ti(tokens.border), ti(tokens.text));
    let shadow = crate::styles::floating_shadow(tokens);
    move |_| container::Style {
        background: Some(Background::Color(raised)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        shadow,
        ..container::Style::default()
    }
}

/// A popover row: flat, with a `hover` fill and the small radius.
pub fn popup_row(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (hover, ink) = (ti(tokens.hover), ti(tokens.text));
    move |_, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(hover)),
            _ => None,
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Ink of the "open" check mark in a popover row.
pub fn check_ink(tokens: &ThemeTokens) -> Color {
    resolve(tokens).accent_text
}

/// Colour of the faint rule between two clusters.
pub(super) fn rule_ink(tokens: &ThemeTokens) -> Color {
    resolve(tokens).line_strong
}

/// A faint vertical rule between two clusters, 12 px tall.
pub(super) fn rule<'a>(tokens: &ThemeTokens) -> Element<'a, StatusBarRequest> {
    let ink = rule_ink(tokens);
    container(iced::widget::Space::new())
        .width(HAIRLINE)
        .height(RULE_HEIGHT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ink)),
            ..container::Style::default()
        })
        .boxed()
}

/// The zoom readout's number: whole percent, no grouping, like the
/// coordinates beside it.
pub(super) fn zoom_value(zoom: f64) -> String {
    format!("{zoom:.0}")
}

/// The zoom segment of the bar: `zoom` as a whole percent.
pub(super) fn zoom_text(i18n: &I18n, zoom: f64) -> Localized {
    t!(i18n, ids::PANELS_STATUS_BAR_ZOOM, "percent" => zoom_value(zoom))
}

/// The cursor readout: `(x, y)` as numbers in `unit`.
pub(super) fn coord_values(unit: Unit, x: f64, y: f64) -> (String, String) {
    match unit {
        Unit::Mm => (format!("{x:.2}"), format!("{y:.2}")),
        Unit::Mil => (format!("{:.1}", x / 0.0254), format!("{:.1}", y / 0.0254)),
        Unit::Inch => (format!("{:.4}", x / 25.4), format!("{:.4}", y / 25.4)),
        Unit::Micrometer => (format!("{:.0}", x * 1000.0), format!("{:.0}", y * 1000.0)),
    }
}

/// The X and Y columns: numbers only, the unit follows once after Y.
pub(super) fn cursor_texts(i18n: &I18n, unit: Unit, x: f64, y: f64) -> (Localized, Localized) {
    let (xs, ys) = coord_values(unit, x, y);
    (
        t!(i18n, ids::PANELS_STATUS_BAR_X, "value" => xs),
        t!(i18n, ids::PANELS_STATUS_BAR_Y, "value" => ys),
    )
}

/// A grid spacing in `unit`, with its unit symbol.
pub(crate) fn grid_value(size_mm: f32, unit: Unit) -> String {
    let mm = f64::from(size_mm);
    match unit {
        Unit::Mm => format!("{mm:.3} {unit}"),
        Unit::Mil => format!("{:.1} {unit}", mm / 0.0254),
        Unit::Inch => format!("{:.4} {unit}", mm / 25.4),
        Unit::Micrometer => format!("{:.0} {unit}", mm * 1000.0),
    }
}

/// The Grid span's value: the spacing in `unit`, or "OFF".
pub(super) fn grid_label(visible: bool, size_mm: f32, unit: Unit) -> String {
    if !visible {
        return "OFF".to_string();
    }
    grid_value(size_mm, unit)
}

/// The Grid read-out: "Grid 1.270 mm", always with its unit, or "Grid OFF".
pub(super) fn grid_text(i18n: &I18n, visible: bool, size_mm: f32, unit: Unit) -> Localized {
    if visible {
        t!(i18n, ids::PANELS_STATUS_BAR_GRID, "grid" => grid_label(true, size_mm, unit))
    } else {
        t!(i18n, ids::PANELS_STATUS_BAR_GRID_OFF)
    }
}

pub(super) fn mono_text(s: String, color: Color) -> Text<'static> {
    // raw: a reading, a unit symbol, or an activity note its caller already
    // rendered from the catalog.
    text(raw(s)).size(TS.body).color(color).font(mono_font())
}

/// A catalog message in the mono face.
pub(super) fn mono_message(s: Localized, color: Color) -> Text<'static> {
    text(s).size(TS.body).color(color).font(mono_font())
}

pub(super) fn mono_button(
    label: Text<'static>,
    request: StatusBarRequest,
) -> button::Button<'static, StatusBarRequest, Text<'static>> {
    button(label)
        .padding([1.0, SP.s])
        .style(button::text)
        .on_press(request)
}

/// The board view's angle (#2316): a caption and the value, a click opens a
/// value field in its place; Enter applies a number.
pub(super) fn view_angle<'a>(
    r: ViewAngleReadout,
    tokens: &ThemeTokens,
) -> Element<'a, StatusBarRequest> {
    let muted = resolve(tokens).ink3;
    let ink = ti(tokens.text);
    let caption = text(r.caption).size(TS.body).color(muted);
    let Some(entry) = r.entry else {
        let readout = button(
            row![caption, mono_text(r.value, ink)]
                .spacing(SP.xs)
                .align_y(iced::Alignment::Center),
        )
        .padding([1.0, SP.s])
        .style(button::text)
        .on_press(StatusBarRequest::ViewAngleClicked);
        let tip = container(text(r.tooltip).size(TS.body))
            .padding(SP.s)
            .style(popup(tokens));
        return signex_widgets::tooltip(readout, tip, signex_widgets::tooltip::Position::Top)
            .boxed();
    };
    let (field_bg, line, accent) = (
        crate::styles::tokens::input_bg(tokens),
        crate::styles::tokens::input_border(tokens),
        ti(tokens.accent),
    );
    let field = signex_widgets::text_input(r.placeholder, entry)
        .id(crate::app::contracts::VIEW_ANGLE_INPUT_ID.clone())
        .on_input(StatusBarRequest::ViewAngleEdited)
        .on_submit(StatusBarRequest::ViewAngleSubmitted)
        .font(mono_font())
        .size(TS.body)
        .padding([0.0, SP.xs])
        // Room for "-359.9" in the mono face.
        .width(TS.body * 5.0)
        .style(move |_: &Theme, status| signex_widgets::text_input::Style {
            background: Background::Color(field_bg),
            border: Border {
                width: 1.0,
                color: match status {
                    signex_widgets::text_input::Status::Focused { .. } => accent,
                    _ => line,
                },
                radius: BAR_CONTROL_RADIUS.into(),
                ..Border::default()
            },
            placeholder: muted,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    row![
        button(caption)
            .padding([1.0, SP.s])
            .style(button::text)
            .on_press(StatusBarRequest::ViewAngleClicked),
        field,
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Label of the Snap pill: "Snap on" while snapping, "Snap off" while free.
pub(super) fn snap_label(snapping: bool) -> MessageId {
    if snapping {
        ids::PANELS_STATUS_BAR_SNAP_ON
    } else {
        ids::PANELS_STATUS_BAR_SNAP_OFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn the_snap_pill_is_filled_only_while_snapping() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        let on = snap_pill(&t, true)(&Theme::Dark, button::Status::Active);
        let off = snap_pill(&t, false)(&Theme::Dark, button::Status::Active);
        assert!(on.background.is_some());
        assert!(off.background.is_none());
        assert_ne!(on.text_color, off.text_color);
    }

    /// The grid spacing reads in the unit the bar shows, not as mm.
    #[test]
    fn the_grid_reads_in_the_shown_unit() {
        assert_eq!(grid_label(true, 0.254, Unit::Mil), "10.0 mil");
        assert_eq!(grid_label(true, 1.27, Unit::Mm), "1.270 mm");
        assert_eq!(grid_label(false, 1.27, Unit::Mm), "OFF");
    }

    /// The unit sits after Y, not by the grid, so the Grid read-out always
    /// names its own unit (mockup row 6, accepted by Caner).
    #[test]
    fn the_grid_read_out_carries_its_unit_in_every_unit() {
        let i18n = I18n::en_us();
        assert_eq!(
            plain(grid_text(&i18n, true, 1.27, Unit::Mm)),
            "Grid 1.270 mm"
        );
        assert_eq!(
            plain(grid_text(&i18n, true, 0.127, Unit::Mil)),
            "Grid 5.0 mil"
        );
        assert_eq!(
            plain(grid_text(&i18n, true, 2.54, Unit::Inch)),
            "Grid 0.1000 in"
        );
        assert_eq!(plain(grid_text(&i18n, false, 2.54, Unit::Mm)), "Grid OFF");
    }

    /// X and Y are numbers only; the unit is written once, after Y.
    #[test]
    fn the_cursor_columns_hold_numbers_only() {
        let i18n = I18n::en_us();
        let (x, y) = cursor_texts(&i18n, Unit::Mm, 127.0, 45.72);
        assert_eq!(plain(x), "X 127.00");
        assert_eq!(plain(y), "Y 45.72");
        let (x, y) = cursor_texts(&i18n, Unit::Mil, 31.75, 22.352);
        assert_eq!(plain(x), "X 1250.0");
        assert_eq!(plain(y), "Y 880.0");
    }

    /// The zoom reads as trunk's `format!("{:.0}%")` did, for every value.
    #[test]
    fn the_zoom_reads_as_a_whole_percent_without_grouping() {
        let i18n = I18n::en_us();
        for zoom in [0.5, 2.5, 12.5, 99.5, 100.0, 1600.0, 12345.4] {
            let shown = plain(zoom_text(&i18n, zoom));
            assert_eq!(shown, format!("{:.0}%", zoom), "zoom {zoom}");
        }
        assert_eq!(zoom_value(0.5), "0");
        assert_eq!(zoom_value(2.5), "2");
        assert_eq!(zoom_value(1600.0), "1600");
    }

    #[test]
    fn the_snap_pill_reads_on_or_off() {
        assert_eq!(snap_label(true), ids::PANELS_STATUS_BAR_SNAP_ON);
        assert_eq!(snap_label(false), ids::PANELS_STATUS_BAR_SNAP_OFF);
    }

    #[test]
    fn the_panels_button_has_a_border() {
        let t = theme_tokens(ThemeId::GraphiteDark);
        let s = panels_button(&t)(&Theme::Dark, button::Status::Active);
        assert_eq!(s.border.width, 1.0);
    }

    /// The selection summary is a sunken pill in the text ink.
    #[test]
    fn the_selection_pill_is_sunken_in_the_text_ink() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let t = theme_tokens(id);
            let s = selection_pill(&t)(&Theme::Dark, button::Status::Active);
            assert_eq!(
                s.background,
                Some(Background::Color(resolve(&t).sunken)),
                "{id:?}"
            );
            assert_eq!(s.text_color, ti(t.text), "{id:?}");
        }
    }

    /// The cluster rules are the faint line token of each Graphite theme.
    #[test]
    fn the_cluster_rule_uses_the_line_token() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let t = theme_tokens(id);
            assert_eq!(rule_ink(&t), resolve(&t).line_strong, "{id:?}");
        }
    }

    #[test]
    fn the_popup_is_a_card_with_a_shadow() {
        let t = theme_tokens(ThemeId::GraphiteLight);
        let s = popup(&t)(&Theme::Dark);
        assert_eq!(
            s.border.radius,
            iced::border::Radius::from(Radii::GRAPHITE.card)
        );
        assert!(s.shadow.blur_radius > 0.0);
        assert_eq!(popup_height(3), 3.0 * 26.0 + 8.0);
    }

    /// The Panels popover is the mock's 236 px card with 26 px rows; each
    /// group caption adds its own fixed height.
    #[test]
    fn the_panel_list_card_is_236_wide_with_26_px_rows() {
        assert_eq!(POPUP_WIDTH, 236.0);
        assert_eq!(popup_row_height(), 26.0);
        assert_eq!(
            panel_list_height(4, 2),
            4.0 * 26.0 + 8.0 + 2.0 * popup_caption_height()
        );
        assert!(popup_caption_height() > TS.section);
    }

    /// The Grid & Snap popover is anchored after the cursor cluster and its
    /// rule: the 96 and 84 px columns, the unit, the gaps and the rule.
    #[test]
    fn the_grid_read_out_starts_after_the_cursor_cluster() {
        assert_eq!(X_COLUMN, 96.0);
        assert_eq!(Y_COLUMN, 84.0);
        let x = grid_readout_x();
        assert!(x > BAR_PADDING.left + X_COLUMN + Y_COLUMN + UNIT_COLUMN);
        assert_eq!(
            x,
            BAR_PADDING.left
                + X_COLUMN
                + Y_COLUMN
                + UNIT_COLUMN
                + 2.0 * CLUSTER_GAP
                + 2.0 * BAR_GAP
                + HAIRLINE
        );
    }
}
