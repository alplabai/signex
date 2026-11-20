//! The pins of a placed symbol's Properties view: the clicked pin's own
//! section (mockup row 50) and the compact Pins table. Read-only: pin fields change in
//! the symbol editor, not on a placed part.

use super::super::*;
use super::illustrations::pin::{NeighbourPin, PinLabels, PinSketch, pin_illustration};
use super::part_panel::{PartPanelMsg, illustration_box, link_row, part_msg};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::row;
use signex_engine::info_keys;
use signex_i18n::{MessageId, Translatable, ids, t};
use std::collections::HashMap;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Column widths as `FillPortion`s: designator, name, type, net.
const COLUMN_PORTIONS: [u16; 4] = [2, 3, 5, 4];
/// Height of a table row and of the focused row's accent edge, in body sizes.
const ROW_HEIGHT_EM: f32 = 2.0;
/// Width of the focused row's accent edge, px.
const FOCUS_EDGE: f32 = 2.0;
/// Height of the pin illustration, in caption sizes.
const PIN_ILLUSTRATION_EM: f32 = 13.0;

/// One pin as the Properties rows carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PinRow {
    pub designator: String,
    pub name: String,
    pub electrical_type: String,
    pub net: Option<String>,
    pub hidden: bool,
    pub focused: bool,
    /// The pin length in mm as the engine wrote it ("2.54").
    pub length: Option<String>,
    /// The library rotation of the pin, whole degrees.
    pub rotation_deg: Option<i32>,
}

/// The pin rows in `selection_info`, in order. Only the first row with the
/// focused key is marked, so a symbol with a repeated pin number highlights
/// one row.
pub(super) fn pin_rows(info: &[(String, String)]) -> Vec<PinRow> {
    let focus = info
        .iter()
        .find(|(k, _)| k == info_keys::FOCUSED_PIN)
        .map(|(_, v)| v.as_str());
    // One pass for the nets and the geometry; the first row of a key wins.
    let mut nets: HashMap<&str, &str> = HashMap::new();
    let mut geometry: HashMap<&str, &str> = HashMap::new();
    for (k, v) in info {
        if let Some(pin) = k.strip_prefix(info_keys::PIN_NET_PREFIX) {
            nets.entry(pin).or_insert(v.as_str());
        } else if let Some(pin) = k.strip_prefix(info_keys::PIN_GEOMETRY_PREFIX) {
            geometry.entry(pin).or_insert(v.as_str());
        }
    }
    let mut focus_taken = false;
    info.iter()
        .filter_map(|(k, v)| {
            let designator = k.strip_prefix(info_keys::PIN_PREFIX)?.to_string();
            let mut cells = v.split('\t');
            let name = cells.next().unwrap_or_default().to_string();
            let electrical_type = cells.next().unwrap_or_default().to_string();
            let hidden = cells.next() == Some("Yes");
            let focused = !focus_taken && focus == Some(designator.as_str());
            focus_taken |= focused;
            let mut geo = geometry.get(designator.as_str()).map(|g| g.split('\t'));
            let length = geo.as_mut().and_then(Iterator::next).map(str::to_string);
            let rotation_deg = geo
                .as_mut()
                .and_then(Iterator::next)
                .and_then(|r| r.parse().ok());
            Some(PinRow {
                net: nets.get(designator.as_str()).map(|n| (*n).to_string()),
                focused,
                designator,
                name,
                electrical_type,
                hidden,
                length,
                rotation_deg,
            })
        })
        .collect()
}

/// The direction a pin extends in, from its library rotation: 0 is right,
/// counter-clockwise from there.
pub(super) fn orientation_label(rotation_deg: i32) -> MessageId {
    match ((rotation_deg.rem_euclid(360) + 45) / 90) % 4 {
        1 => ids::PROPERTIES_PIN_ORIENTATION_UP,
        2 => ids::PROPERTIES_PIN_ORIENTATION_LEFT,
        3 => ids::PROPERTIES_PIN_ORIENTATION_DOWN,
        _ => ids::PROPERTIES_PIN_ORIENTATION_RIGHT,
    }
}

/// The focused pin's drawing data: the pin, and the pins before and after
/// it in the list, drawn faint.
fn pin_sketch(rows: &[PinRow], index: usize) -> PinSketch {
    let pin = &rows[index];
    let neighbour = |row: Option<&PinRow>| {
        row.map(|r| NeighbourPin {
            number: r.designator.clone(),
            name: r.name.clone(),
        })
    };
    PinSketch {
        number: pin.designator.clone(),
        name: pin.name.clone(),
        electrical_type: pin.electrical_type.clone(),
        net: pin.net.clone(),
        length_mm: pin
            .length
            .as_deref()
            .and_then(|l| l.parse::<f32>().ok())
            .filter(|l| l.is_finite())
            .unwrap_or(0.0),
        hidden: pin.hidden,
        neighbours: [
            neighbour(index.checked_sub(1).and_then(|i| rows.get(i))),
            neighbour(rows.get(index + 1)),
        ],
    }
}

/// The clicked pin's own section, ahead of the part's (#2086, mockup row 50): its
/// drawing, its facts as read-only rows and the two ways out, to the symbol
/// editor and to the pin's net.
pub(super) fn push_focused_pin_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &PanelContext,
    muted: Color,
) -> Column<Element<'a, PanelMsg>> {
    let rows = pin_rows(&ctx.selection_info);
    let Some(index) = rows.iter().position(|row| row.focused) else {
        return col;
    };
    let pin = rows[index].clone();
    let sketch = pin_sketch(&rows, index);
    let title = Translatable::new(ids::PROPERTIES_PIN_TITLE).arg("pin", pin.designator.as_str());
    col.push(
        ctx.collapsible(
            FOCUSED_PIN_SECTION_KEY,
            title,
            &ctx.collapsed_sections,
            move || focused_pin_body(ctx, pin, sketch, muted),
        )
        .boxed(),
    )
}

fn focused_pin_body<'a>(
    ctx: &PanelContext,
    pin: PinRow,
    sketch: PinSketch,
    muted: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let PanelPalette { input_bg, .. } = PanelPalette::from_tokens(&ctx.tokens);
    let length = pin.length.clone().unwrap_or_default();
    let labels = PinLabels {
        type_leader:
            t!(i18n, ids::PROPERTIES_PIN_ILLUSTRATION_TYPE, "type" => pin.electrical_type.as_str())
                .into(),
        length: t!(i18n, ids::PROPERTIES_PIN_ILLUSTRATION_LENGTH, "length" => length.as_str())
            .into(),
    };
    let drawing = pin_illustration(&sketch, labels).height_em(PIN_ILLUSTRATION_EM);
    let mut c: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(illustration_box(drawing, ctx))
        .push(
            container(
                text(t!(i18n, ids::PROPERTIES_PIN_READ_ONLY_NOTE))
                    .size(TS.caption)
                    .color(resolve(&ctx.tokens).ink4),
            )
            .padding([0.0, SP.m])
            .boxed(),
        );
    let hidden = i18n.message_plain(
        if pin.hidden {
            ids::PROPERTIES_YES
        } else {
            ids::PROPERTIES_NO
        },
        &[],
    );
    let length_value = pin
        .length
        .as_deref()
        .map(|l| i18n.message_plain(ids::PROPERTIES_PIN_LENGTH_VALUE, &[("length", l.into())]))
        .unwrap_or_default();
    let orientation = pin
        .rotation_deg
        .map(|deg| i18n.message_plain(orientation_label(deg), &[]))
        .unwrap_or_default();
    let read_only = |c: Column<Element<'a, PanelMsg>>, label, value: &str| {
        c.push(form_input_row(t!(i18n, label), value, muted, input_bg))
    };
    c = read_only(c, ids::PROPERTIES_DESIGNATOR, &pin.designator);
    c = read_only(c, ids::PROPERTIES_NAME, &pin.name);
    // The type is the engine's label (#2060).
    c = read_only(c, ids::PROPERTIES_PIN_TYPE, &pin.electrical_type);
    c = match &pin.net {
        Some(net) => c.push(link_row(
            t!(i18n, ids::PROPERTIES_NET),
            net,
            part_msg(PartPanelMsg::PinNetShowRequested(net.clone())),
            muted,
            &ctx.tokens,
        )),
        None => read_only(c, ids::PROPERTIES_NET, ""),
    };
    c = read_only(c, ids::PROPERTIES_PIN_LENGTH, &length_value);
    c = read_only(c, ids::PROPERTIES_PIN_ORIENTATION, &orientation);
    c = read_only(c, ids::PROPERTIES_HIDDEN, &hidden);
    c.push(pin_actions(ctx, pin.net))
}

/// Edit in Symbol Editor and Show net, small buttons in one row. The editor
/// opens the symbol; it has no way to focus one pin yet.
fn pin_actions<'a>(ctx: &PanelContext, net: Option<String>) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let edit = signex_widgets::button(
        text(t!(i18n, ids::PROPERTIES_PIN_EDIT_IN_SYMBOL_EDITOR)).size(TS.caption),
    )
    .padding([SP.xs, SP.m])
    .on_press(PanelMsg::PlacedPartAction(
        crate::library::PlacedPartAction::EditSymbol,
    ))
    .style(super::super::form_look::ghost_button);
    let show_net =
        signex_widgets::button(text(t!(i18n, ids::PROPERTIES_PIN_SHOW_NET)).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press_maybe(net.map(|n| part_msg(PartPanelMsg::PinNetShowRequested(n))))
            .style(crate::styles::ghost_button(&ctx.tokens));
    container(
        iced::widget::Row::<Element<'a, PanelMsg>>::new()
            .push(edit.boxed())
            .push(show_net.boxed())
            .spacing(SP.s)
            .wrap()
            .vertical_spacing(SP.s),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Collapse key of the clicked pin's section.
const FOCUSED_PIN_SECTION_KEY: &str = "sel_focused_pin";

/// The Pins section: a compact table with the pin count in the header. It
/// follows the real collapse state; a pin click opens it through `update`
/// (`PINS_SECTION_KEY`).
pub(super) fn push_pins_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &PanelContext,
    primary: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    // The count is a cheap scan; the rows are built only if the section is open.
    let pin_count = ctx
        .selection_info
        .iter()
        .filter(|(k, _)| k.starts_with(info_keys::PIN_PREFIX))
        .count();
    if pin_count == 0 {
        return col;
    }
    let is_collapsed = ctx.collapsed_sections.contains(PINS_SECTION_KEY);
    let col = col.push(counted_header(ctx, pin_count, is_collapsed));
    if is_collapsed {
        return col;
    }
    col.push(pins_table(ctx, primary, border_c))
}

/// The Pins header: chevron, the upper-cased title and the count pill.
fn counted_header<'a>(
    ctx: &PanelContext,
    count: usize,
    is_collapsed: bool,
) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let ink = resolve(&ctx.tokens).ink4;
    let title = i18n.upper_for_display(&i18n.message_plain(ids::PROPERTIES_SECTION_PINS, &[]));
    let pill = container(
        text(t!(i18n, ids::PROPERTIES_PINS_COUNT, "count" => count.to_string())).size(TS.caption),
    )
    .padding([0.0, SP.s])
    .style(crate::styles::pill(
        &ctx.tokens,
        crate::styles::PillTone::Neutral,
    ));
    let label = row![
        signex_widgets::chevron::chevron(!is_collapsed, TS.section, ink),
        // The catalog title, upper-cased for the locale.
        text(signex_i18n::UserText::new(title))
            .size(TS.section)
            .color(ink),
        pill,
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    signex_widgets::button(
        container(label)
            .padding(iced::Padding {
                top: SP.l,
                right: SP.m,
                bottom: SP.s,
                left: SP.m,
            })
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(PINS_SECTION_KEY.to_string()))
    .style(|_: &Theme, _| iced::widget::button::Style::default())
    .boxed()
}

/// The table: a sunken header row, one row per pin, the focused pin's row
/// on the accent with an accent edge, a hidden pin's state in its tooltip.
fn pins_table<'a>(ctx: &PanelContext, primary: Color, border_c: Color) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let r = resolve(&ctx.tokens);
    let rows = pin_rows(&ctx.selection_info);
    // The Net column exists only once the project net index can answer.
    let show_net = rows.iter().any(|row| row.net.is_some());
    let mut table: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    let headings = [
        ids::PROPERTIES_PINS_COLUMN_PIN,
        ids::PROPERTIES_PINS_COLUMN_NAME,
        ids::PROPERTIES_PINS_COLUMN_TYPE,
        ids::PROPERTIES_PINS_COLUMN_NET,
    ]
    .map(|id| i18n.upper_for_display(&i18n.message_plain(id, &[])));
    table = table.push(table_row(
        headings,
        show_net,
        RowLook {
            ink: r.ink3,
            fill: Some(r.sunken),
            edge: None,
            size: TS.caption,
        },
    ));
    let hidden_tip = i18n.message_plain(ids::PROPERTIES_PIN_HIDDEN_TIP, &[]);
    for pin in rows {
        let cells = [
            pin.designator,
            pin.name,
            // raw: engine value (#2060).
            pin.electrical_type,
            pin.net.unwrap_or_default(),
        ];
        let look = if pin.focused {
            RowLook {
                ink: r.accent_soft_ink,
                fill: Some(r.accent_soft),
                edge: Some(crate::styles::ti(ctx.tokens.accent)),
                size: TS.body,
            }
        } else {
            RowLook {
                ink: primary,
                fill: None,
                edge: None,
                size: TS.body,
            }
        };
        let line = table_row(cells, show_net, look);
        table = table.push(if pin.hidden {
            signex_widgets::tooltip(
                line,
                text(signex_i18n::UserText::new(hidden_tip.clone())).size(TS.body),
                iced::widget::tooltip::Position::Bottom,
            )
            .style(crate::styles::context_menu(&ctx.tokens))
            .boxed()
        } else {
            line
        });
    }
    container(
        container(table)
            .width(Length::Fill)
            .clip(true)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    width: 1.0,
                    color: border_c,
                    radius: crate::styles::Radii::GRAPHITE.button.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(iced::Padding {
        top: SP.xs,
        right: SP.m,
        bottom: SP.m,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// How one table row is drawn.
#[derive(Clone, Copy)]
struct RowLook {
    ink: Color,
    fill: Option<Color>,
    /// The accent edge on the left of the focused row.
    edge: Option<Color>,
    size: f32,
}

fn table_row<'a>(cells: [String; 4], show_net: bool, look: RowLook) -> Element<'a, PanelMsg> {
    let height = TS.body * ROW_HEIGHT_EM;
    let mut line: iced::widget::Row<Element<'a, PanelMsg>> =
        row![].spacing(SP.s).align_y(iced::Alignment::Center);
    for (index, cell) in cells.into_iter().enumerate() {
        if index == 3 && !show_net {
            continue;
        }
        line = line.push(
            // raw: a pin's data, or a column heading already rendered from the catalog.
            text(raw(cell))
                .size(look.size)
                .color(look.ink)
                .width(Length::FillPortion(COLUMN_PORTIONS[index]))
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .boxed(),
        );
    }
    let edge_ink = look.edge.unwrap_or(Color::TRANSPARENT);
    let edge = container(Space::new())
        .width(Length::Fixed(FOCUS_EDGE))
        .height(Length::Fixed(height))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(edge_ink)),
            ..container::Style::default()
        });
    let fill = look.fill;
    container(
        row![
            edge,
            container(line)
                .padding([0.0, SP.s])
                .height(Length::Fixed(height))
                .align_y(iced::alignment::Vertical::Center)
                .width(Length::Fill),
        ]
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: fill.map(Background::Color),
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> Vec<(String, String)> {
        vec![
            ("Type".into(), "Symbol".into()),
            ("Pin: 1".into(), "A\tPassive\tNo".into()),
            ("Pin Geometry: 1".into(), "2.54\t180".into()),
            ("Pin: 2".into(), "K\tPassive\tYes".into()),
            ("Pin: 2".into(), "K2\tPassive\tNo".into()),
            (info_keys::FOCUSED_PIN.into(), "2".into()),
            ("Pin Net: 1".into(), "GND".into()),
            ("Pin Net: 2".into(), "VCC".into()),
        ]
    }

    #[test]
    fn every_pin_row_carries_its_facts_and_its_own_net() {
        let rows = pin_rows(&info());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].net.as_deref(), Some("GND"));
        assert!(!rows[0].focused && !rows[0].hidden);
        assert_eq!(rows[1].net.as_deref(), Some("VCC"));
        assert!(rows[1].hidden);
        assert_eq!(rows[1].electrical_type, "Passive");
    }

    #[test]
    fn a_pin_row_carries_its_length_and_rotation() {
        let rows = pin_rows(&info());
        assert_eq!(rows[0].length.as_deref(), Some("2.54"));
        assert_eq!(rows[0].rotation_deg, Some(180));
        assert_eq!(rows[1].length, None, "no geometry row, no length");
    }

    #[test]
    fn the_orientation_follows_the_library_rotation() {
        let en = signex_i18n::I18n::en_us();
        let shown = |deg| en.message_plain(orientation_label(deg), &[]);
        assert_eq!(shown(0), "Right");
        assert_eq!(shown(90), "Up");
        assert_eq!(shown(180), "Left");
        assert_eq!(shown(270), "Down");
        assert_eq!(shown(-90), "Down");
    }

    #[test]
    fn the_focused_pin_is_drawn_between_its_neighbours() {
        let rows = pin_rows(&info());
        let sketch = pin_sketch(&rows, 1);
        assert_eq!(sketch.number, "2");
        let name = |i: usize| sketch.neighbours[i].as_ref().map(|n| n.name.clone());
        assert_eq!(name(0).as_deref(), Some("A"));
        assert_eq!(name(1).as_deref(), Some("K2"));
        assert!(sketch.hidden);
    }

    #[test]
    fn a_repeated_pin_number_highlights_one_row() {
        let rows = pin_rows(&info());
        let focused: Vec<_> = rows.iter().map(|r| r.focused).collect();
        assert_eq!(focused, [false, true, false]);
    }

    #[test]
    fn without_a_clicked_pin_no_row_is_focused() {
        let mut rows = info();
        rows.retain(|(k, _)| k != info_keys::FOCUSED_PIN);
        assert!(pin_rows(&rows).iter().all(|r| !r.focused));
    }

    #[test]
    fn with_no_net_index_the_rows_have_no_net() {
        let mut rows = info();
        rows.retain(|(k, _)| !k.starts_with(info_keys::PIN_NET_PREFIX));
        assert!(pin_rows(&rows).iter().all(|r| r.net.is_none()));
    }

    #[test]
    fn two_thousand_pins_each_get_their_own_net() {
        let mut rows = Vec::new();
        for n in 0..2000 {
            rows.push((format!("Pin: {n}"), format!("P{n}	Passive	No")));
        }
        for n in (0..2000).rev() {
            rows.push((format!("Pin Net: {n}"), format!("NET{n}")));
        }
        let out = pin_rows(&rows);
        assert_eq!(out.len(), 2000);
        for (n, row) in out.iter().enumerate() {
            assert_eq!(row.designator, n.to_string());
            assert_eq!(row.net.as_deref(), Some(format!("NET{n}").as_str()));
        }
    }
}
