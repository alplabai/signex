//! The Body row of a placed symbol's Properties (#1233): Normal or Alternate
//! (De Morgan), disabled with a tooltip when the library symbol has no
//! alternate body.

use super::super::*;
use crate::choice::Choice;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::schematic::BodyStyle;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Panel value: the symbol shows the normal body.
pub(crate) const BODY_NORMAL: &str = "Normal";
/// Panel value: the alternate body is stored and the library symbol has it.
pub(crate) const BODY_ALTERNATE: &str = "Alternate";
/// Panel value: no alternate body exists for the unit, the normal one is
/// drawn.
pub(crate) const BODY_ALTERNATE_MISSING: &str = "AlternateMissing";
/// Panel value: the library symbol has an alternate, none is stored.
pub(crate) const BODY_NORMAL_CAN_SWITCH: &str = "NormalCanSwitch";

/// One entry of the Body drop-down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyChoice {
    Normal,
    Alternate,
    /// Alternate is stored but the library symbol has none; drawn as Normal.
    AlternateMissing,
}

impl BodyChoice {
    fn style(self) -> BodyStyle {
        match self {
            Self::Normal => BodyStyle::Normal,
            Self::Alternate | Self::AlternateMissing => BodyStyle::Alternate,
        }
    }
}

impl BodyChoice {
    /// The entry's name in the drop-down.
    fn label(self) -> MessageId {
        match self {
            Self::Normal => ids::PROPERTIES_BODY_NORMAL,
            Self::Alternate => ids::PROPERTIES_BODY_ALTERNATE,
            Self::AlternateMissing => ids::PROPERTIES_BODY_ALTERNATE_MISSING,
        }
    }
}

/// Append the Body row for the panel's `Body Style` state (empty = not a
/// library symbol, no row).
pub(super) fn push_body_row<'a>(
    c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    state: &str,
    muted: Color,
    input_bg: Color,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let pick = move |choice: BodyChoice| PanelMsg::SymbolBodyStylePicked(id, choice.style());
    let named = |choice: BodyChoice| Choice {
        value: choice,
        label: i18n.message_plain(choice.label(), &[]),
    };
    let row = |choices: &[BodyChoice], selected: BodyChoice| {
        form_choice_row(
            t!(i18n, ids::PROPERTIES_BODY),
            choices.iter().map(|&choice| named(choice)).collect(),
            named(selected),
            pick,
            muted,
        )
    };
    match state {
        BODY_NORMAL_CAN_SWITCH => c.push(row(
            &[BodyChoice::Normal, BodyChoice::Alternate],
            BodyChoice::Normal,
        )),
        BODY_ALTERNATE => c.push(row(
            &[BodyChoice::Normal, BodyChoice::Alternate],
            BodyChoice::Alternate,
        )),
        // The only way out of a dangling alternate is back to Normal.
        BODY_ALTERNATE_MISSING => c.push(row(&[BodyChoice::Normal], BodyChoice::AlternateMissing)),
        BODY_NORMAL => c.push(iced::widget::Widget::boxed(signex_widgets::tooltip::<
            _,
            _,
            iced::Theme,
        >(
<<<<<<< HEAD
            form_input_row_untipped("Body", "Normal", muted, input_bg),
            signex_widgets::text::<iced::Theme>(signex_widgets::raw(
                "This library symbol has no alternate (De Morgan) body",
            ))
            .size(TS.body),
=======
            form_input_row(
                t!(i18n, ids::PROPERTIES_BODY),
                &i18n.message_plain(ids::PROPERTIES_BODY_NORMAL, &[]),
                muted,
                input_bg,
            ),
            signex_widgets::text::<iced::Theme>(t!(i18n, ids::PROPERTIES_BODY_NO_ALTERNATE_TIP))
                .size(TS.body),
>>>>>>> origin/trunk
            iced::widget::tooltip::Position::Bottom,
        ))),
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::chosen;

    /// Each entry reads as the `Display` text it had before #2059 part C,
    /// and a choice is found by its value.
    #[test]
    fn each_body_entry_keeps_its_english_name() {
        let en = I18n::en_us();
        let all = [
            (BodyChoice::Normal, "Normal"),
            (BodyChoice::Alternate, "Alternate (De Morgan)"),
            (
                BodyChoice::AlternateMissing,
                "Alternate (not in symbol; drawn as Normal)",
            ),
        ];
        let choices = Choice::named(&all.map(|(b, _)| b), BodyChoice::label, &en);
        for (body, english) in all {
            assert_eq!(en.message_plain(body.label(), &[]), english, "{body:?}");
            assert_eq!(
                chosen(&choices, &body).map(|c| c.label).as_deref(),
                Some(english)
            );
        }
    }
}
