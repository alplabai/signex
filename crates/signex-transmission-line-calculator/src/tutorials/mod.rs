use super::*;

pub(super) fn tutorial_references_section() -> Element<'static, SmithChartMessage> {
    let mut link_row = row![].spacing(8);
    for link in TUTORIAL_REFERENCE_LINKS {
        link_row = link_row
            .push(button(text(link.label())).on_press(SmithChartMessage::OpenReferenceLink(link)));
    }
    section("Tutorials", vec![link_row.into()])
}
