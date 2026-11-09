//! The Library / out-of-date rows shared by the symbol and power-port
//! Properties views (#912).

use super::super::*;
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The "Symbol" pick list of a placed symbol whose library row offers
/// alternate drawings (#1230). Nothing when there is only the default.
pub(super) fn push_symbol_choice_row<'a>(
    c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    choices: &[SymbolChoiceItem],
    muted: impl Into<LabelInk>,
) -> Column<Element<'a, PanelMsg>> {
    let Some(current) = choices.iter().find(|i| i.selected) else {
        return c;
    };
    if choices.len() < 2 {
        return c;
    }
    let labels: Vec<String> = choices.iter().map(|i| i.label.clone()).collect();
    let items = choices.to_vec();
    c.push(form_pick_row(
        "Symbol",
        labels,
        current.label.clone(),
        move |label| {
            let lib_id = items
                .iter()
                .find(|i| i.label == label)
                .map(|i| i.lib_id.clone())
                .unwrap_or_default();
            PanelMsg::SwitchSymbolChoice(id, lib_id)
        },
        muted,
    ))
}

/// One entry of the Footprint pick list, told apart by its position so two
/// footprints with the same label stay distinct.
#[derive(Clone, PartialEq, Eq)]
struct FootprintEntry {
    index: usize,
    label: String,
}

impl std::fmt::Display for FootprintEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// Label of the free-text footprint field. Under the pick list it is an
/// override: a name typed there is held over the picked binding (shown in
/// the list as "(edited)"); alone it is the part's only footprint field.
pub(super) fn footprint_text_label(has_pick_list: bool) -> &'static str {
    if has_pick_list {
        "Footprint override"
    } else {
        "Footprint"
    }
}

/// The footprint rows of a placed symbol (#1234): a pick list of the
/// footprints its library row offers, when it offers two or more, above the
/// free-text field every part keeps.
pub(super) fn push_footprint_row<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    footprint: &str,
    choices: &[FootprintChoiceItem],
    muted: impl Into<LabelInk>,
) -> Column<Element<'a, PanelMsg>> {
    let ink: LabelInk = muted.into();
    let current = choices.iter().position(|i| i.selected);
    if let Some(current) = current {
        let entry = |index: usize| FootprintEntry {
            index,
            label: choices[index].label.clone(),
        };
        let options: Vec<FootprintEntry> = (0..choices.len()).map(entry).collect();
        let bindings: Vec<String> = choices.iter().map(|i| i.binding.clone()).collect();
        c = c.push(form_pick_row(
            "Footprint",
            options,
            entry(current),
            move |picked: FootprintEntry| {
                let binding = bindings.get(picked.index).cloned().unwrap_or_default();
                PanelMsg::SwitchFootprintChoice(id, binding)
            },
            ink,
        ));
    }
    c.push(form_edit_row(
        footprint_text_label(current.is_some()),
        footprint,
        ink,
        move |s| PanelMsg::EditSymbolFootprint(id, s),
    ))
}

/// The Library ID row: the id in mono on one line, cut with an ellipsis
/// when the column is narrow, the whole id in a tooltip.
pub(super) fn library_id_row<'a>(
    lib_id: &str,
    label: impl Into<LabelInk>,
    input_bg: Color,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let value = container(
        iced::widget::text(lib_id.to_string())
            .size(TS.body)
            .font(mono)
            .color(crate::styles::tokens::readable_on(input_bg))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill),
    )
    .padding([3, 6])
    .width(Length::Fill)
    .clip(true)
    .style(move |theme: &Theme| {
        let (bg, bdr, radius) = super::super::form_look::value_box(theme);
        container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: radius.into(),
                color: bdr,
                ..Border::default()
            },
            ..container::Style::default()
        }
    });
    let tip = iced::widget::tooltip(
        value,
        container(
            iced::widget::text(lib_id.to_string())
                .size(TS.body)
                .font(mono),
        )
        .padding([SP.xs, 6.0]),
        iced::widget::tooltip::Position::Bottom,
    )
    .style(crate::styles::context_menu(tokens));
    container(
        iced::widget::row![
            property_label("Library ID", label),
            container(tip).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// How the sheet's embedded copy compares with the library (#912, #1845).
#[derive(Clone, Copy)]
pub(super) struct LibraryStatus {
    pub(super) drifted: bool,
    pub(super) missing: bool,
}

/// Text of the out-of-date status.
pub(super) const OUT_OF_DATE: &str = "Library: out of date";

/// Append the readable `Library` row and, when the sheet's embedded copy
/// differs from the library, the "out of date" status with its Update
/// button. Update replaces the sheet's shared copy, so it affects every
/// instance of the part on the sheet - the tooltip says so. On Graphite the
/// status is a warn pill and Update a ghost button.
pub(super) fn push_library_rows<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    library_name: &str,
    status: LibraryStatus,
    muted: impl Into<LabelInk>,
    input_bg: Color,
    tokens: &ThemeTokens,
) -> Column<Element<'a, PanelMsg>> {
    let ink: LabelInk = muted.into();
    if !library_name.is_empty() {
        c = c.push(form_input_row("Library", library_name, ink, input_bg));
    }
    let LibraryStatus { drifted, missing } = status;
    if missing {
        c = c.push(
            iced::widget::text("Library: symbol missing (sheet keeps its embedded copy)")
                .size(TS.body)
                .color(ink.color)
                .boxed(),
        );
    }
    if drifted {
        c = c.push(out_of_date_row(id, tokens));
    }
    c
}

/// The out-of-date status and its Update button.
fn out_of_date_row<'a>(id: uuid::Uuid, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let update = iced::widget::button(iced::widget::text("Update from library").size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(PanelMsg::UpdateSymbolFromLibrary(id))
        .style(super::super::form_look::ghost_button);
    let status: Element<'a, PanelMsg> = container(iced::widget::text(OUT_OF_DATE).size(TS.caption))
        .padding([2.0, 8.0])
        .style(crate::styles::pill(tokens, crate::styles::PillTone::Warn))
        .boxed();
    iced::widget::row![
        status,
        iced::widget::tooltip(
            update,
            iced::widget::text("Updates every instance of this part on the sheet").size(TS.body),
            iced::widget::tooltip::Position::Bottom,
        ),
    ]
    .spacing(SP.m)
    .padding([SP.xs, SP.m])
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The placed-part actions of the Properties panel, label first.
fn part_action_buttons() -> Vec<(&'static str, PanelMsg)> {
    use crate::library::PlacedPartAction as A;
    [
        ("Open Datasheet", A::OpenDatasheet),
        ("Edit Symbol", A::EditSymbol),
        ("Change Component\u{2026}", A::ChangeComponent),
    ]
    .into_iter()
    .map(|(label, action)| (label, PanelMsg::PlacedPartAction(action)))
    .collect()
}

/// A row of buttons for the actions that lead from the selected placed
/// part into its library (#1301, #1262, #1260).
pub(super) fn push_part_actions<'a>(
    c: Column<Element<'a, PanelMsg>>,
) -> Column<Element<'a, PanelMsg>> {
    // Wrapping row: at a narrow dock width the last button drops to the
    // next line instead of being clipped (#1747).
    let buttons = iced::widget::Row::with_children(part_action_buttons().into_iter().map(
        |(label, message)| -> Element<'a, PanelMsg> {
            iced::widget::button(iced::widget::text(label).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(message)
                .style(super::super::form_look::ghost_button)
                .boxed()
        },
    ))
    .spacing(SP.m)
    .wrap()
    .vertical_spacing(SP.s);
    c.push(
        iced::widget::container(buttons)
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::PlacedPartAction as A;

    #[test]
    fn the_part_actions_carry_the_issue_labels_and_their_actions() {
        let buttons = part_action_buttons();
        let labels: Vec<&str> = buttons.iter().map(|(label, _)| *label).collect();
        assert_eq!(
            labels,
            ["Open Datasheet", "Edit Symbol", "Change Component\u{2026}"]
        );
        let actions: Vec<A> = buttons
            .into_iter()
            .map(|(_, message)| match message {
                PanelMsg::PlacedPartAction(a) => a,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(
            actions,
            [A::OpenDatasheet, A::EditSymbol, A::ChangeComponent]
        );
    }

    #[test]
    fn the_text_field_under_a_footprint_pick_list_is_the_override() {
        assert_eq!(footprint_text_label(true), "Footprint override");
        assert_eq!(footprint_text_label(false), "Footprint");
    }
}
