use super::*;
use iced::ContentFit;
use iced::widget::{column, rule, svg};

mod formula_reference;

pub(super) use formula_reference::FormulaReference;

#[cfg(test)]
mod tests;

const EQUATION_SVGS: [&[u8]; 7] = [
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_0.svg")),
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_1.svg")),
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_2.svg")),
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_3.svg")),
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_4.svg")),
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_5.svg")),
    include_bytes!(concat!(env!("OUT_DIR"), "/smith_chart_equation_6.svg")),
];

pub(super) fn formula_references_section() -> Element<'static, SmithChartMessage> {
    let mut rows = Vec::new();
    for (index, (entry, rendered)) in FORMULA_REFERENCES.iter().zip(EQUATION_SVGS).enumerate() {
        if index > 0 {
            rows.push(rule::horizontal(1).into());
        }
        let equation: Element<'static, SmithChartMessage> = svg(svg::Handle::from_memory(rendered))
            .width(Length::Fill)
            .height(Length::Shrink)
            .content_fit(ContentFit::ScaleDown)
            .style(|theme: &Theme, _| svg::Style {
                color: Some(theme.palette().text),
            })
            .into();
        rows.push(
            column![
                text(entry.item).size(18),
                equation,
                text(entry.notes).size(12)
            ]
            .spacing(6)
            .into(),
        );
    }

    rows.push(rule::horizontal(1).into());
    let mut link_row = row![].spacing(8);
    for link in FORMULA_REFERENCE_LINKS {
        link_row = link_row
            .push(button(text(link.label())).on_press(SmithChartMessage::OpenReferenceLink(link)));
    }
    rows.push(link_row.into());

    section("Equations", rows)
}
