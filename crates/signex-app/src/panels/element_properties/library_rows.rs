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
    muted: Color,
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

/// The footprint rows of a placed symbol (#1234): a pick list of the
/// footprints its library row offers, when it offers two or more, above the
/// free-text field every part keeps.
pub(super) fn push_footprint_row<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    footprint: &str,
    choices: &[FootprintChoiceItem],
    muted: Color,
) -> Column<Element<'a, PanelMsg>> {
    if let Some(current) = choices.iter().position(|i| i.selected) {
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
            muted,
        ));
    }
    c.push(form_edit_row("Footprint", footprint, muted, move |s| {
        PanelMsg::EditSymbolFootprint(id, s)
    }))
}

/// How the sheet's embedded copy compares with the library (#912, #1845).
#[derive(Clone, Copy)]
pub(super) struct LibraryStatus<'a> {
    pub(super) drifted: bool,
    /// Placed instances of the part on the sheet that draw with an older
    /// version of the library symbol (0: unknown or none).
    pub(super) older_instances: usize,
    /// The selected instance already draws with the current version, so
    /// "Only this one" has nothing to do for it.
    pub(super) this_one_current: bool,
    pub(super) missing: bool,
    /// The UI language's catalog, for the labels these rows render.
    pub(super) i18n: &'a signex_i18n::I18n,
}

/// Append the readable `Library` row and, when the sheet's embedded copy
/// differs from the library, the "out of date" row with its Update button.
/// Update replaces the sheet's shared copy, so it affects every instance of
/// the part on the sheet - the tooltip says so.
pub(super) fn push_library_rows<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    library_name: &str,
    status: LibraryStatus<'_>,
    muted: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Column<Element<'a, PanelMsg>> {
    use signex_i18n::{ids, t};
    if !library_name.is_empty() {
        c = c.push(form_input_row(
            "Library",
            library_name,
            muted,
            input_bg,
            input_bdr,
        ));
    }
    let LibraryStatus {
        drifted,
        older_instances,
        this_one_current,
        missing,
        i18n,
    } = status;
    if missing {
        c = c.push(
            signex_widgets::text(t!(i18n, ids::PROPERTIES_LIBRARY_SYMBOL_MISSING))
                .size(TS.body)
                .color(muted)
                .boxed(),
        );
    }
    if drifted {
        let update = signex_widgets::button(
            signex_widgets::text(t!(i18n, ids::PROPERTIES_LIBRARY_UPDATE)).size(TS.body),
        )
        .padding([SP.xs, SP.m])
        .on_press(PanelMsg::UpdateSymbolFromLibrary(id));
        let only_this = signex_widgets::button(
            signex_widgets::text(t!(i18n, ids::PROPERTIES_LIBRARY_ONLY_THIS_ONE)).size(TS.body),
        )
        .padding([SP.xs, SP.m])
        .on_press(PanelMsg::UpdateSymbolFromLibraryOnly(id));
        let note = match older_instances {
            0 => signex_widgets::text(t!(i18n, ids::PROPERTIES_LIBRARY_OUT_OF_DATE)),
            n => signex_widgets::text(
                t!(i18n, ids::PROPERTIES_LIBRARY_OLDER_INSTANCES, "count" => n),
            ),
        };
        let mut actions = iced::widget::row![
            note.size(TS.body).color(muted),
            signex_widgets::tooltip(
                update,
                signex_widgets::text(t!(i18n, ids::PROPERTIES_LIBRARY_UPDATE_TIP)).size(TS.body),
                iced::widget::tooltip::Position::Bottom,
            ),
        ];
        if !this_one_current {
            actions = actions.push(
                signex_widgets::tooltip(
                    only_this,
                    signex_widgets::text(t!(i18n, ids::PROPERTIES_LIBRARY_ONLY_THIS_ONE_TIP))
                        .size(TS.body),
                    iced::widget::tooltip::Position::Bottom,
                )
                .boxed(),
            );
        }
        c = c.push(
            actions
                .spacing(SP.m)
                .padding([SP.xs, SP.m])
                .align_y(iced::Alignment::Center)
                .boxed(),
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
pub(super) fn push_part_actions<'a>(
    c: Column<Element<'a, PanelMsg>>,
) -> Column<Element<'a, PanelMsg>> {
    // Wrapping row: at a narrow dock width the last button drops to the
    // next line instead of being clipped (#1747).
    let buttons = iced::widget::Row::with_children(part_action_buttons().into_iter().map(
        |(label, message)| -> Element<'a, PanelMsg> {
            signex_widgets::button(signex_widgets::text(signex_widgets::raw(label)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(message)
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
}
