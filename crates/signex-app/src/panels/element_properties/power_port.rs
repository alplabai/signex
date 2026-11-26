//! Properties of a selected power port (mockup row 45), in the order every placed
//! object keeps: the illustration, Location, Properties (Name, the Style
//! tiles, the library rows, Font, Font Size with B / I / U / T), Net (the
//! Name above is the net's name, so Net has no second name field), Net
//! Parameters and State ▸ Locked. High Speed and Differential Pair are
//! greyed and take no input while Power Net is on (decision A-m45): a power
//! rail is neither.

use super::super::*;
use super::font_row::font_family_row;
use super::illustrations;
use super::rows::{PanelStyle, info_row, rotation_degrees};
use crate::panels::PlacedEditMsg;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_types::measure::length_field_text;
use signex_types::schematic::{PowerPortStyle, SelectedItem, SelectedKind};
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

/// The font the port's net name is stored with (mockup row 45): its own
/// family (`None` inherits the sheet default) and its size in points. A
/// port reporting no size reads the 10 pt the canvas draws by default.
pub(super) fn stored_font(info: &[(String, String)]) -> (Option<String>, u32) {
    let family = Some(info_row(info, "Font Family")).filter(|family| !family.is_empty());
    let size_pt = info_row(info, "Text Size").parse().unwrap_or(10);
    (family, size_pt)
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

/// The callouts of the port's drawing, read as its rows read: the style's
/// name, the Font Size points and the Location X and Y (mockup row 45).
pub(super) fn port_callouts(
    ctx: &PanelContext,
    lib_id: &str,
    font_size_pt: u32,
) -> illustrations::PowerPortCallouts {
    let i18n = &ctx.i18n;
    let style = i18n.message_plain(style_label(PowerPortStyle::of(lib_id)), &[]);
    let position = ctx.placed.geometry.map(|geometry| {
        let at = geometry.position;
        let x = length_field_text(at.x, ctx.unit);
        let y = length_field_text(ctx.shown_y(at.y), ctx.unit);
        (x, y)
    });
    illustrations::PowerPortCallouts::new(i18n, &style, font_size_pt, position)
}

/// The port's drawing: the glyph and name in the power colour, from the
/// canvas palette, on its wire stub with the value callouts.
fn port_drawing<'a>(
    ctx: &PanelContext,
    lib_id: &str,
    name: &str,
    font_size_pt: u32,
) -> Element<'a, PanelMsg> {
    let power = signex_widgets::theme_ext::power_port(&ctx.canvas_colors);
    let callouts = port_callouts(ctx, lib_id, font_size_pt);
    illustrations::show(
        illustrations::power_port(lib_id, name, power, callouts),
        ctx,
    )
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
    let (font_family, font_size_pt) = stored_font(&ctx.selection_info);
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;

    col = col.push(port_drawing(ctx, &lib_id, &value, font_size_pt));

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
                c = c.push(name_row(ctx, id, &name, look));
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
                    font_family.as_deref(),
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
                    font_size_pt,
                    move |pt| PanelMsg::EditSymbolValueFontSizePt(id, pt),
                    st,
                ))
            },
        )
        .boxed(),
    );

    // ── Net ──: the read-only Physical Name, then the net flags. Name above
    // is the net's name (DECISIONS 2026-11-22, mockup row 45).
    let physical_name = value;
    col = col.push(
        ctx.collapsible(
            "sel_net",
            ids::PROPERTIES_SECTION_NET,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_PHYSICAL_NAME),
                    &physical_name,
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
    super::rows::push_state_section(col, ctx, locked, PanelMsg::ToggleSymbolLocked(id))
}

/// The one Name field. It names the port's net: what is typed stays a draft
/// until Enter or until the port is left, so a rename is one undo step. A
/// locked port shows it read-only.
fn name_row<'a>(
    ctx: &'a PanelContext,
    id: uuid::Uuid,
    stored: &str,
    look: super::placed_rows::RowLook,
) -> Element<'a, PanelMsg> {
    let label = t!(ctx.i18n, ids::PROPERTIES_NAME);
    if ctx.placed.locked.is_some() {
        return form_input_row(label, stored, look.muted, look.input_bg);
    }
    let item = SelectedItem::new(id, SelectedKind::Symbol);
    let input = signex_widgets::text_input(
        signex_i18n::UserText::new(String::new()),
        ctx.placed.port_name_text(item, stored),
    )
    .on_input(move |text| PanelMsg::PlacedEdit(PlacedEditMsg::PortNameTyped { item, text }))
    .on_submit(PanelMsg::PlacedEdit(PlacedEditMsg::PortNameSubmitted {
        item,
    }))
    .size(TS.body)
    .padding(SP.s)
    .style(crate::panels::form_look::text_input_style)
    .width(Length::FillPortion(PROPERTY_CONTROL_PORTION));
    container(
        row![property_label(label, look.muted), input]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
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
                .padding([SP.xs, SP.s])
                .style(crate::panels::form_look::value_tip),
            iced::widget::tooltip::Position::Bottom,
        )
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
            container(text(t!(i18n, style_label(style))).size(TS.body))
                .padding([SP.xs, SP.s])
                .style(crate::panels::form_look::value_tip),
            iced::widget::tooltip::Position::Bottom,
        )
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

    fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        (pairs.iter())
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Mockup row 45: the Font and Font Size rows read the port's stored
    /// family and size; a port with neither reads Default and 10 pt.
    #[test]
    fn the_font_rows_read_the_ports_stored_font() {
        let stored = rows(&[("Font Family", "Iosevka"), ("Text Size", "14")]);
        assert_eq!(stored_font(&stored), (Some("Iosevka".to_string()), 14));
        assert_eq!(stored_font(&rows(&[("Type", "Power Port")])), (None, 10));
    }

    /// Mockup row 45: the drawn panel shows the stored family beside Font
    /// and the stored size beside Font Size.
    #[test]
    fn the_panel_shows_the_ports_font_and_size() {
        use signex_types::schematic::SelectedKind;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(SelectedKind::Symbol);
        ctx.selected_uuid = Some(uuid::Uuid::now_v7());
        ctx.selection_info = rows(&[
            ("Type", "Power Port"),
            ("Value", "+3V3"),
            ("Library ID", "power:bar"),
            ("Text Size", "36"),
            ("Font Family", "Signex Probe Sans"),
        ]);
        let view = super::super::view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        let shown: Vec<(String, iced::Rectangle)> =
            crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
                .into_iter()
                .map(|t| (t.text.replace(['\u{2068}', '\u{2069}'], ""), t.bounds))
                .collect();
        // The first text right of `label` on its row.
        let beside = |label: &str| {
            let (_, at) = shown.iter().find(|(t, _)| t == label)?;
            let middle = |b: &iced::Rectangle| b.y + b.height / 2.0;
            (shown.iter())
                .filter(|(_, b)| (middle(b) - middle(at)).abs() < 4.0 && b.x > at.x)
                .min_by(|a, b| a.1.x.total_cmp(&b.1.x))
                .map(|(t, _)| t.clone())
        };
        assert_eq!(
            beside("Font").as_deref(),
            Some("Signex Probe Sans"),
            "{shown:?}"
        );
        assert_eq!(beside("Font Size").as_deref(), Some("36"), "{shown:?}");
    }

    /// Mockup row 45: the drawing's callouts read the port's style, its
    /// stored font size and the Location X and Y as the rows show them.
    #[test]
    fn the_drawing_callouts_read_the_ports_rows() {
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.placed.geometry = Some(signex_engine::PlacedGeometry {
            position: signex_types::schematic::Point::new(152.4, 50.8),
            rotation: Some(0.0),
            size: None,
        });
        let plain = |t: &signex_widgets::property_illustration::Text| {
            t.as_str().replace(['\u{2068}', '\u{2069}'], "")
        };
        let callouts = port_callouts(&ctx, "power:arrow", 14);
        assert_eq!(plain(&callouts.style), "Style · Arrow");
        assert_eq!(plain(&callouts.font_size), "Font Size 14");
        let x = length_field_text(152.4, ctx.unit);
        let y = length_field_text(ctx.shown_y(50.8), ctx.unit);
        assert_eq!(plain(&callouts.position), format!("Position {x}, {y}"));
        ctx.placed.geometry = None;
        let bare = port_callouts(&ctx, "power:GND", 10);
        assert_eq!(plain(&bare.style), "Style · Power Ground");
        assert_eq!(plain(&bare.position), "Position");
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
