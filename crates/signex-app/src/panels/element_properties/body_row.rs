//! The Body row of a placed symbol's Properties (#1233, mockup row 42): a
//! Normal / Alternate (De Morgan) segmented control. Alternate takes no
//! press, and its tooltip says why, when the library symbol has no alternate
//! body.

use super::super::*;
use super::rows::labelled_row;
use crate::panels::form_look;
use iced::Widget;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::schematic::BodyStyle;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
/// The ink of a segment that takes no press (mockup row 42).
const DISABLED_INK_ALPHA: f32 = 0.45;

/// Panel value: the symbol shows the normal body.
pub(crate) const BODY_NORMAL: &str = "Normal";
/// Panel value: the alternate body is stored and the library symbol has it.
pub(crate) const BODY_ALTERNATE: &str = "Alternate";
/// Panel value: no alternate body exists for the unit, the normal one is
/// drawn.
pub(crate) const BODY_ALTERNATE_MISSING: &str = "AlternateMissing";
/// Panel value: the library symbol has an alternate, none is stored.
pub(crate) const BODY_NORMAL_CAN_SWITCH: &str = "NormalCanSwitch";

/// What a Body state stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyChoice {
    Normal,
    Alternate,
    /// Alternate is stored but the library symbol has none; drawn as Normal.
    AlternateMissing,
}

impl BodyChoice {
    /// The full name of the state; the Alternate segment's tooltip.
    fn label(self) -> MessageId {
        match self {
            Self::Normal => ids::PROPERTIES_BODY_NORMAL,
            Self::Alternate => ids::PROPERTIES_BODY_ALTERNATE,
            Self::AlternateMissing => ids::PROPERTIES_BODY_ALTERNATE_MISSING,
        }
    }
}

/// How the two segments read for one panel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BodySegments {
    is_alternate_shown: bool,
    can_pick_alternate: bool,
    alternate_tip: MessageId,
}

/// The segments of the panel's `Body Style` state; `None` is not a library
/// symbol, which has no row.
fn segments_of(state: &str) -> Option<BodySegments> {
    let (is_alternate_shown, can_pick_alternate, alternate_tip) = match state {
        BODY_NORMAL_CAN_SWITCH => (false, true, BodyChoice::Alternate.label()),
        BODY_ALTERNATE => (true, true, BodyChoice::Alternate.label()),
        // The only way out of a dangling alternate is back to Normal.
        BODY_ALTERNATE_MISSING => (true, false, BodyChoice::AlternateMissing.label()),
        BODY_NORMAL => (false, false, ids::PROPERTIES_BODY_NO_ALTERNATE_TIP),
        _ => return None,
    };
    Some(BodySegments {
        is_alternate_shown,
        can_pick_alternate,
        alternate_tip,
    })
}

/// One segment. With no `on_press` it takes no press, and unless it is the
/// active one it is drawn faint.
fn segment<'a>(
    label: signex_i18n::Localized,
    is_active: bool,
    on_press: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let is_faint = on_press.is_none() && !is_active;
    signex_widgets::button(
        signex_widgets::text(label)
            .size(TS.body)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([SP.s, SP.xs])
    .on_press_maybe(on_press)
    .style(move |theme: &Theme, status: iced::widget::button::Status| {
        let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
        let mut style = form_look::segment_cell(theme, is_active, is_hovered && !is_active);
        if is_faint {
            style.text_color = style.text_color.scale_alpha(DISABLED_INK_ALPHA);
        }
        style
    })
    .boxed()
}

/// Append the Body row for the panel's `Body Style` state (empty = not a
/// library symbol, no row).
pub(super) fn push_body_row<'a>(
    c: Column<Element<'a, PanelMsg>>,
    id: uuid::Uuid,
    state: &str,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let Some(shown) = segments_of(state) else {
        return c;
    };
    let pick = |body: BodyStyle| PanelMsg::SymbolBodyStylePicked(id, body);
    let is_alternate = shown.is_alternate_shown;
    let normal = segment(
        t!(i18n, BodyChoice::Normal.label()),
        !is_alternate,
        is_alternate.then(|| pick(BodyStyle::Normal)),
    );
    let alternate = segment(
        t!(i18n, ids::PROPERTIES_BODY_ALTERNATE_SHORT),
        is_alternate,
        (shown.can_pick_alternate && !is_alternate).then(|| pick(BodyStyle::Alternate)),
    );
    let alternate = signex_widgets::tooltip(
        alternate,
        container(signex_widgets::text(t!(i18n, shown.alternate_tip)).size(TS.body))
            .padding([SP.xs, SP.s])
            .style(form_look::value_tip),
        iced::widget::tooltip::Position::Bottom,
    );
    let control = container(
        row![
            container(normal).width(Length::FillPortion(1)),
            container(alternate).width(Length::FillPortion(1)),
        ]
        .spacing(SP.xs),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .style(form_look::segmented_frame);
    c.push(labelled_row(
        t!(i18n, ids::PROPERTIES_BODY),
        control.boxed(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::{Choice, chosen};

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

    /// Mockup row 42: Alternate is greyed with a reason when the symbol has
    /// no alternate body, and live when it has one.
    #[test]
    fn alternate_takes_a_press_only_when_the_symbol_has_one() {
        let plain = segments_of(BODY_NORMAL).expect("a row");
        assert!(!plain.can_pick_alternate && !plain.is_alternate_shown);
        assert_eq!(plain.alternate_tip, ids::PROPERTIES_BODY_NO_ALTERNATE_TIP);
        let switchable = segments_of(BODY_NORMAL_CAN_SWITCH).expect("a row");
        assert!(switchable.can_pick_alternate);
        let dangling = segments_of(BODY_ALTERNATE_MISSING).expect("a row");
        assert!(dangling.is_alternate_shown && !dangling.can_pick_alternate);
        assert_eq!(segments_of(""), None, "not a library symbol");
    }

    fn pressed(state: &str, label: &str) -> Vec<PanelMsg> {
        let en = I18n::en_us();
        let row = push_body_row(Column::new(), uuid::Uuid::nil(), state, &en);
        crate::layout_probe::messages_on_click(row.boxed(), iced::Size::new(320.0, 80.0), label)
    }

    /// A press on a live segment picks that body; the greyed one sends nothing.
    #[test]
    fn the_segments_send_the_body_they_name() {
        assert!(matches!(
            pressed(BODY_NORMAL_CAN_SWITCH, "Alternate").as_slice(),
            [PanelMsg::SymbolBodyStylePicked(_, BodyStyle::Alternate)]
        ));
        assert!(matches!(
            pressed(BODY_ALTERNATE, "Normal").as_slice(),
            [PanelMsg::SymbolBodyStylePicked(_, BodyStyle::Normal)]
        ));
        assert!(pressed(BODY_NORMAL, "Alternate").is_empty());
    }
}
