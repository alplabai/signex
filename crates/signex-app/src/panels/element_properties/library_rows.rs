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
    c: Column<'a, PanelMsg>,
    id: uuid::Uuid,
    choices: &[SymbolChoiceItem],
    muted: Color,
) -> Column<'a, PanelMsg> {
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

/// How the sheet's embedded copy compares with the library (#912, #1845).
#[derive(Clone, Copy)]
pub(super) struct LibraryStatus {
    pub(super) drifted: bool,
    pub(super) missing: bool,
}

/// Append the readable `Library` row and, when the sheet's embedded copy
/// differs from the library, the "out of date" row with its Update button.
/// Update replaces the sheet's shared copy, so it affects every instance of
/// the part on the sheet - the tooltip says so.
pub(super) fn push_library_rows<'a>(
    mut c: Column<'a, PanelMsg>,
    id: uuid::Uuid,
    library_name: &str,
    status: LibraryStatus,
    muted: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Column<'a, PanelMsg> {
    if !library_name.is_empty() {
        c = c.push(form_input_row(
            "Library",
            library_name,
            muted,
            input_bg,
            input_bdr,
        ));
    }
    let LibraryStatus { drifted, missing } = status;
    if missing {
        c = c.push(
            iced::widget::text("Library: symbol missing (sheet keeps its embedded copy)")
                .size(TS.body)
                .color(muted),
        );
    }
    if drifted {
        let update = iced::widget::button(iced::widget::text("Update from library").size(TS.body))
            .padding([SP.xs, SP.m])
            .on_press(PanelMsg::UpdateSymbolFromLibrary(id));
        c = c.push(
            iced::widget::row![
                iced::widget::text("Library: out of date")
                    .size(TS.body)
                    .color(muted),
                iced::widget::tooltip(
                    update,
                    iced::widget::text("Updates every instance of this part on the sheet")
                        .size(TS.body),
                    iced::widget::tooltip::Position::Bottom,
                ),
            ]
            .spacing(SP.m)
            .padding([SP.xs, SP.m])
            .align_y(iced::Alignment::Center),
        );
    }
    c
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
pub(super) fn push_part_actions<'a>(c: Column<'a, PanelMsg>) -> Column<'a, PanelMsg> {
    // Wrapping row: at a narrow dock width the last button drops to the
    // next line instead of being clipped (#1747).
    let buttons = iced::widget::Row::with_children(part_action_buttons().into_iter().map(
        |(label, message)| {
            iced::widget::button(iced::widget::text(label).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(message)
                .into()
        },
    ))
    .spacing(SP.m)
    .wrap()
    .vertical_spacing(SP.s);
    c.push(
        iced::widget::container(buttons)
            .padding([SP.xs, SP.m])
            .width(Length::Fill),
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
}
