//! The Pins section of a placed symbol's Properties view: one read-only row
//! per pin, with the pin a click landed on highlighted. Pin fields change in
//! the symbol editor, not on a placed part.

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use iced::widget::row;
use signex_engine::info_keys;
use std::collections::HashMap;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Opacity of the accent behind the focused pin's row.
const FOCUS_ROW_ALPHA: f32 = 0.25;

/// Column widths as `FillPortion`s: designator, name, type, net, hidden.
const COLUMN_PORTIONS: [u16; 5] = [2, 3, 4, 4, 2];

/// One pin as the Properties rows carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PinRow {
    pub designator: String,
    pub name: String,
    pub electrical_type: String,
    pub net: Option<String>,
    pub hidden: bool,
    pub focused: bool,
}

/// The pin rows in `selection_info`, in order. Only the first row with the
/// focused key is marked, so a symbol with a repeated pin number highlights
/// one row.
pub(super) fn pin_rows(info: &[(String, String)]) -> Vec<PinRow> {
    let focus = info
        .iter()
        .find(|(k, _)| k == info_keys::FOCUSED_PIN)
        .map(|(_, v)| v.as_str());
    // One pass for the nets; the first row of a repeated key wins.
    let mut nets: HashMap<&str, &str> = HashMap::new();
    for (k, v) in info {
        if let Some(pin) = k.strip_prefix(info_keys::PIN_NET_PREFIX) {
            nets.entry(pin).or_insert(v.as_str());
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
            Some(PinRow {
                net: nets.get(designator.as_str()).map(|n| (*n).to_string()),
                focused,
                designator,
                name,
                electrical_type,
                hidden,
            })
        })
        .collect()
}

/// The Pins section. It follows the real collapse state; a pin click opens
/// it through `update` (`PINS_SECTION_KEY`).
pub(super) fn push_pins_section<'a>(
    col: Column<'a, PanelMsg>,
    ctx: &PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    // The count is a cheap scan; the rows are built only if the section is open.
    let pin_count = ctx
        .selection_info
        .iter()
        .filter(|(k, _)| k.starts_with(info_keys::PIN_PREFIX))
        .count();
    if pin_count == 0 {
        return col;
    }
    let accent = PanelPalette::from_tokens(&ctx.tokens).accent;
    let title = format!("Pins ({pin_count})");
    let info = &ctx.selection_info;
    col.push(collapsible_section(
        PINS_SECTION_KEY,
        &title,
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let rows = pin_rows(info);
            // The Net column exists only once the project net index can answer.
            let show_net = rows.iter().any(|r| r.net.is_some());
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(table_row(
                ["Pin", "Name", "Type", "Net", "Hidden"].map(String::from),
                show_net,
                muted,
                None,
            ));
            c = c.push(thin_sep(border_c));
            for pin in rows {
                let cells = [
                    pin.designator,
                    pin.name,
                    pin.electrical_type,
                    pin.net.unwrap_or_default(),
                    if pin.hidden {
                        "Yes".into()
                    } else {
                        String::new()
                    },
                ];
                let colour = if pin.focused { primary } else { muted };
                let backdrop = pin.focused.then_some(Color {
                    a: FOCUS_ROW_ALPHA,
                    ..accent
                });
                c = c.push(table_row(cells, show_net, colour, backdrop));
            }
            c
        },
    ))
}

fn table_row<'a>(
    cells: [String; 5],
    show_net: bool,
    colour: Color,
    backdrop: Option<Color>,
) -> Element<'a, PanelMsg> {
    let mut line = row![].spacing(SP.s);
    for (index, cell) in cells.into_iter().enumerate() {
        if index == 3 && !show_net {
            continue;
        }
        line = line.push(
            text(cell)
                .size(TS.body)
                .color(colour)
                .width(Length::FillPortion(COLUMN_PORTIONS[index]))
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }
    container(line)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: backdrop.map(Into::into),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> Vec<(String, String)> {
        vec![
            ("Type".into(), "Symbol".into()),
            ("Pin: 1".into(), "A\tPassive\tNo".into()),
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
