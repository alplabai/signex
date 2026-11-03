//! The Pins section of a placed symbol's Properties view: one read-only row
//! per pin, with the pin a click landed on highlighted. Pin fields change in
//! the symbol editor, not on a placed part.

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use signex_engine::info_keys;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Section key in [`CollapsedSections`].
const SECTION_KEY: &str = "sel_pins";

/// Opacity of the accent behind the focused pin's row.
const FOCUS_ROW_ALPHA: f32 = 0.25;

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

/// The pin rows in `selection_info`, in order, with the focused one marked.
pub(super) fn pin_rows(info: &[(String, String)]) -> Vec<PinRow> {
    let value = |key: &str| info.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let focused = value(info_keys::FOCUSED_PIN);
    info.iter()
        .filter_map(|(k, v)| {
            let designator = k.strip_prefix(info_keys::PIN_PREFIX)?.to_string();
            let mut cells = v.split('\t');
            let name = cells.next().unwrap_or_default().to_string();
            let electrical_type = cells.next().unwrap_or_default().to_string();
            let hidden = cells.next() == Some("Yes");
            Some(PinRow {
                net: value(&format!("{}{designator}", info_keys::PIN_NET_PREFIX)),
                focused: focused.as_deref() == Some(designator.as_str()),
                designator,
                name,
                electrical_type,
                hidden,
            })
        })
        .collect()
}

/// The Pins section; open whenever a pin is focused.
pub(super) fn push_pins_section<'a>(
    col: Column<'a, PanelMsg>,
    ctx: &PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let rows = pin_rows(&ctx.selection_info);
    if rows.is_empty() {
        return col;
    }
    let accent = PanelPalette::from_tokens(&ctx.tokens).accent;
    let open = CollapsedSections::new();
    let collapsed = if rows.iter().any(|r| r.focused) {
        &open
    } else {
        &ctx.collapsed_sections
    };
    let title = format!("Pins ({})", rows.len());
    col.push(collapsible_section(
        SECTION_KEY,
        &title,
        collapsed,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            for row in rows {
                c = c.push(pin_row_view(row, muted, primary, accent));
            }
            c
        },
    ))
}

fn pin_row_view<'a>(
    row: PinRow,
    muted: Color,
    primary: Color,
    accent: Color,
) -> Element<'a, PanelMsg> {
    let hidden = if row.hidden { "hidden" } else { "shown" };
    let net = row.net.unwrap_or_else(|| "no net".to_string());
    let line = format!(
        "{}  {}  {}  {}  {}",
        row.designator, row.name, row.electrical_type, net, hidden
    );
    let colour = if row.focused { primary } else { muted };
    let focused = row.focused;
    container(text(line).size(TS.body).color(colour))
        .padding([4.0, SP.m])
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: focused.then(|| {
                Color {
                    a: FOCUS_ROW_ALPHA,
                    ..accent
                }
                .into()
            }),
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
            (info_keys::FOCUSED_PIN.into(), "2".into()),
            ("Pin Net: 2".into(), "VCC".into()),
        ]
    }

    #[test]
    fn the_focused_pin_row_carries_its_facts_and_net() {
        let rows = pin_rows(&info());
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].focused && rows[0].net.is_none() && !rows[0].hidden);
        let pin = &rows[1];
        assert!(pin.focused && pin.hidden);
        assert_eq!(
            (
                pin.designator.as_str(),
                pin.name.as_str(),
                pin.net.as_deref()
            ),
            ("2", "K", Some("VCC"))
        );
        assert_eq!(pin.electrical_type, "Passive");
    }

    #[test]
    fn without_a_clicked_pin_no_row_is_focused() {
        let mut rows = info();
        rows.retain(|(k, _)| k != info_keys::FOCUSED_PIN);
        assert!(pin_rows(&rows).iter().all(|r| !r.focused));
    }
}
