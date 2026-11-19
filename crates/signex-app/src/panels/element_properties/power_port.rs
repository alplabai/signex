//! Properties of a selected power port (#45), in the order every placed
//! object keeps: the illustration, Location, Properties (Name, the Style
//! tiles, the library rows, Font, Font Size with B / I / U / T), Net, Net
//! Parameters and State ▸ Locked. High Speed and Differential Pair are
//! greyed and take no input while Power Net is on (decision A-m45): a power
//! rail is neither.

use super::super::*;
use super::font_row::font_family_row;
use super::illustrations;
use super::rows::{PanelStyle, info_row, rotation_degrees};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_types::schematic::PowerPortStyle;
use signex_widgets::CheckboxLabel as _;
use signex_widgets::property_illustration::{IllustrationHover, Ink, property_illustration};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Style tiles per row: the eight styles sit in a 4 x 2 grid.
const TILES_PER_ROW: usize = 4;

/// The catalog name of a power-port style.
fn style_label(style: PowerPortStyle) -> MessageId {
    match style {
        PowerPortStyle::Bar => ids::PROPERTIES_POWER_STYLE_BAR,
        PowerPortStyle::Arrow => ids::PROPERTIES_POWER_STYLE_ARROW,
        PowerPortStyle::Wave => ids::PROPERTIES_POWER_STYLE_WAVE,
        PowerPortStyle::Circle => ids::PROPERTIES_POWER_STYLE_CIRCLE,
        PowerPortStyle::PowerGround => ids::PROPERTIES_POWER_STYLE_POWER_GROUND,
        PowerPortStyle::SignalGround => ids::PROPERTIES_POWER_STYLE_SIGNAL_GROUND,
        PowerPortStyle::Earth => ids::PROPERTIES_POWER_STYLE_EARTH,
        PowerPortStyle::Chassis => ids::PROPERTIES_POWER_STYLE_CHASSIS,
    }
}

/// The short name under a style's tile; the tooltip has the full one.
fn tile_label(style: PowerPortStyle) -> MessageId {
    match style {
        PowerPortStyle::PowerGround => ids::PROPERTIES_POWER_STYLE_SHORT_POWER_GROUND,
        PowerPortStyle::SignalGround => ids::PROPERTIES_POWER_STYLE_SHORT_SIGNAL_GROUND,
        other => style_label(other),
    }
}

/// Every style in tile order, each with whether it is the port's style.
pub(super) fn style_tiles(lib_id: &str) -> Vec<(PowerPortStyle, bool)> {
    let current = PowerPortStyle::of(lib_id);
    (PowerPortStyle::ALL.into_iter())
        .map(|style| (style, style == current))
        .collect()
}

/// Whether High Speed and Differential Pair take input: not while Power Net
/// is on (decision A-m45).
pub(super) fn signal_flags_open(power_net: bool) -> bool {
    !power_net
}

/// The message a pick of `style` sends. A ground body hangs below its
/// connection point and every other style rises above it, so the port turns
/// by 180° when the direction flips and keeps its visual orientation.
fn restyled(id: uuid::Uuid, lib_id: &str, rotation: f64, style: PowerPortStyle) -> PanelMsg {
    let flips = PowerPortStyle::of(lib_id).hangs_down() != style.hangs_down();
    PanelMsg::EditPowerPortStyle {
        symbol_id: id,
        new_lib_id: format!("power:{}", style.lib_tag()),
        rotation_degrees: if flips {
            (rotation + 180.0).rem_euclid(360.0)
        } else {
            rotation
        },
    }
}

pub(super) fn push_power_port_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    id: uuid::Uuid,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle {
        muted,
        primary,
        border_c,
        palette,
    } = st;
    let input_bg = palette.input_bg;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let value = get("Value");
    let lib_id = get("Library ID");
    let library_name = get("Library");
    let library_status = super::library_rows::LibraryStatus {
        drifted: get("Library Drift") == "Yes",
        older_instances: get("Library Older Version").parse().unwrap_or(0),
        this_one_current: get("Library This One Current") == "Yes",
        i18n: &ctx.i18n,
        missing: get("Library Status") == "Symbol missing",
    };
    let rotation = rotation_degrees(&get("Rotation"));
    let locked = get("Locked") == "Yes";
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;

    // The glyph and name in the power colour, from the canvas palette.
    let power = signex_widgets::theme_ext::power_port(&ctx.canvas_colors);
    col = col.push(illustrations::show(
        illustrations::power_port(&lib_id, &value, power, i18n),
        ctx,
    ));

    // ── Location ── (#2418): Position and Rotation through the shared rows.
    let look = super::placed_rows::RowLook { muted, input_bg };
    col = super::placed_rows::push_location_section(col, ctx, look);

    // ── Properties ──: Name, Style, the library rows, Font, Font Size.
    let name = value.clone();
    let tiles_lib = lib_id.clone();
    col = col.push(
        ctx.collapsible(
            "sel_props",
            ids::PANELS_PLACED_PROPERTIES,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row(
                    t!(i18n, ids::PROPERTIES_NAME),
                    &name,
                    muted,
                    move |s| PanelMsg::EditSymbolValue(id, s),
                ));
                c = c.push(style_grid(ctx, id, &tiles_lib, rotation, st));
                c = super::library_rows::push_library_rows(
                    c,
                    id,
                    &library_name,
                    library_status,
                    muted,
                    input_bg,
                    &ctx.tokens,
                );
                c = c.push(font_family_row(
                    t!(i18n, ids::PROPERTIES_FONT),
                    None,
                    muted,
                    move |family| {
                        PanelMsg::EditTextFontFamily(
                            signex_engine::TextTarget::SymbolValue(id),
                            family,
                        )
                    },
                    i18n,
                ));
                c.push(super::label::font_size_row(
                    i18n,
                    10,
                    move |pt| PanelMsg::EditSymbolValueFontSizePt(id, pt),
                    st,
                ))
            },
        )
        .boxed(),
    );

    // ── Net ──: the names, then the net flags.
    let net_name = value;
    col = col.push(
        ctx.collapsible(
            "sel_net",
            ids::PROPERTIES_SECTION_NET,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_NET_NAME),
                    &net_name,
                    muted,
                    input_bg,
                ));
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_PHYSICAL_NAME),
                    &net_name,
                    muted,
                    input_bg,
                ));
                // A power port names a power rail: its net is a power net.
                push_net_flags(c, i18n, true, st)
            },
        )
        .boxed(),
    );

    // ── Net Parameters ──: no parameters, rules or classes on a net yet.
    col = col.push(
        ctx.collapsible(
            "sel_net_params",
            ids::PROPERTIES_SECTION_NET_PARAMS,
            &ctx.collapsed_sections,
            move || {
                Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(net_params_tabs(i18n, primary, muted, input_bg))
                    .push(empty_section_row(
                        t!(i18n, ids::PROPERTIES_NET_NO_PARAMETERS),
                        muted,
                        border_c,
                    ))
                    .push(net_params_add_bar(i18n))
            },
        )
        .boxed(),
    );

    // ── State ──: a power port locks like any part.
    super::selected::push_state_section(col, ctx, locked, PanelMsg::ToggleSymbolLocked(id), muted)
}

/// Power Net and High Speed side by side, then Differential Pair. While
/// Power Net is on the other two are veiled and take no input.
fn push_net_flags<'a>(
    c: Column<Element<'a, PanelMsg>>,
    i18n: &'a signex_i18n::I18n,
    power_net: bool,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let open = signal_flags_open(power_net);
    let check = |label: MessageId, checked: bool| -> Element<'a, PanelMsg> {
        signex_widgets::checkbox(checked)
            .labelled(t!(i18n, label))
            .size(TS.heading)
            .text_size(TS.body)
            .spacing(SP.s)
            .style(crate::panels::form_look::checkbox_style)
            .boxed()
    };
    // Power Net follows the port: shown, not toggled here.
    let power = check(ids::PROPERTIES_POWER_NET, power_net);
    let veil = signex_widgets::enable_section::dependents_veil(st.palette.panel_bg, open);
    let gated = |content: Element<'a, PanelMsg>| match veil {
        Some(veil) => signex_widgets::tooltip(
            signex_widgets::inert::Inert::new(content, veil).boxed(),
            container(text(t!(i18n, ids::PROPERTIES_POWER_NET_EXCLUDES)).size(TS.body))
                .padding([SP.xs, SP.s]),
            iced::widget::tooltip::Position::Bottom,
        )
        .style(crate::panels::form_look::value_tip)
        .boxed(),
        None => content,
    };
    let high_speed = gated(check(ids::PROPERTIES_HIGH_SPEED, false));
    let none = i18n.message_plain(ids::PROPERTIES_DIFF_PAIR_NONE, &[]);
    let diff_pair = form_pick_row(
        t!(i18n, ids::PROPERTIES_DIFF_PAIR),
        vec![none.clone()],
        none,
        |_| PanelMsg::Noop,
        st.muted,
    );
    c.push(
        container(
            row![
                container(power).width(Length::FillPortion(1)),
                container(high_speed).width(Length::FillPortion(1)),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, f32::from(PROPERTY_ROW_PAD_X)])
        .width(Length::Fill)
        .boxed(),
    )
    .push(gated(diff_pair))
}

/// The Style row: the styles as a 4 x 2 grid of toggle tiles, each the
/// glyph the canvas draws over its short name, the full name in a tooltip.
fn style_grid<'a>(
    ctx: &'a PanelContext,
    id: uuid::Uuid,
    lib_id: &str,
    rotation: f64,
    st: PanelStyle,
) -> Element<'a, PanelMsg> {
    let i18n: &signex_i18n::I18n = &ctx.i18n;
    let hover = IllustrationHover::default();
    let tile = |style: PowerPortStyle, active: bool| -> Element<'a, PanelMsg> {
        let style_lib = format!("power:{}", style.lib_tag());
        let ink = if active { Ink::Accent } else { Ink::Secondary };
        let glyph = property_illustration(
            illustrations::style_glyph(&style_lib, ink),
            &hover,
            &ctx.tokens,
            TS.caption,
        )
        .map(|_| PanelMsg::Noop)
        .boxed();
        let face = iced::widget::Column::new()
            .push(glyph)
            .push(
                text(t!(i18n, tile_label(style)))
                    .size(TS.caption)
                    .color(st.muted)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .boxed(),
            )
            .spacing(SP.xs)
            .align_x(iced::Alignment::Center)
            .width(Length::Fill);
        let pressed = signex_widgets::button(face)
            .padding(SP.xs)
            .width(Length::Fill)
            .on_press(restyled(id, lib_id, rotation, style))
            .style(crate::panels::form_look::option_tile(active));
        signex_widgets::tooltip(
            pressed,
            container(text(t!(i18n, style_label(style))).size(TS.body)).padding([SP.xs, SP.s]),
            iced::widget::tooltip::Position::Bottom,
        )
        .style(crate::panels::form_look::value_tip)
        .boxed()
    };
    let tiles = style_tiles(lib_id);
    let mut grid = Column::new().spacing(SP.xs).width(Length::Fill);
    for line in tiles.chunks(TILES_PER_ROW) {
        let mut cells = iced::widget::Row::new().spacing(SP.xs).width(Length::Fill);
        for &(style, active) in line {
            cells = cells.push(tile(style, active));
        }
        grid = grid.push(cells);
    }
    container(
        row![
            property_label(t!(i18n, ids::PROPERTIES_STYLE), st.muted),
            container(grid).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Start),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::{Choice, chosen};

    /// Each style reads as `PowerPortStyle::label`, the English the list
    /// showed before, and a choice is found by its value.
    #[test]
    fn each_style_keeps_its_english_name() {
        let en = signex_i18n::I18n::en_us();
        let choices = Choice::named(&PowerPortStyle::ALL, style_label, &en);
        for style in PowerPortStyle::ALL {
            assert_eq!(en.message_plain(style_label(style), &[]), style.label());
            assert_eq!(
                chosen(&choices, &style).map(|c| c.label).as_deref(),
                Some(style.label())
            );
        }
    }

    /// The tiles list every style once, in order, and mark only the port's.
    #[test]
    fn the_tiles_mark_the_ports_style() {
        let tiles = style_tiles("power:Earth");
        let order: Vec<PowerPortStyle> = tiles.iter().map(|(s, _)| *s).collect();
        assert_eq!(order, PowerPortStyle::ALL);
        let on: Vec<PowerPortStyle> = (tiles.iter())
            .filter(|(_, a)| *a)
            .map(|(s, _)| *s)
            .collect();
        assert_eq!(on, [PowerPortStyle::Earth]);
        // #1284: the chassis style is a tile of its own.
        let chassis: Vec<PowerPortStyle> = (style_tiles("power:Chassis").into_iter())
            .filter(|(_, a)| *a)
            .map(|(s, _)| s)
            .collect();
        assert_eq!(chassis, [PowerPortStyle::Chassis]);
    }

    /// The two grounds keep short names under their tiles.
    #[test]
    fn the_grounds_have_short_tile_names() {
        let en = signex_i18n::I18n::en_us();
        let short = |s| en.message_plain(tile_label(s), &[]);
        assert_eq!(short(PowerPortStyle::PowerGround), "Pwr GND");
        assert_eq!(short(PowerPortStyle::SignalGround), "Sig GND");
        assert_eq!(short(PowerPortStyle::Bar), "Bar");
    }

    /// Decision A-m45: High Speed and Differential Pair close while Power Net
    /// is on and open when it is off.
    #[test]
    fn power_net_closes_the_signal_flags() {
        assert!(!signal_flags_open(true));
        assert!(signal_flags_open(false));
    }

    /// Picking a hanging style for a rising port turns it by 180°.
    #[test]
    fn a_flipped_style_turns_the_port() {
        let id = uuid::Uuid::nil();
        let PanelMsg::EditPowerPortStyle {
            new_lib_id,
            rotation_degrees,
            ..
        } = restyled(id, "power:bar", 90.0, PowerPortStyle::PowerGround)
        else {
            panic!("a style edit");
        };
        assert_eq!(new_lib_id, "power:GND");
        assert_eq!(rotation_degrees, 270.0);
    }
}
