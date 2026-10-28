//! The Body row of a placed symbol's Properties (#1233): Normal or Alternate
//! (De Morgan), disabled with a tooltip when the library symbol has no
//! alternate body.

use super::super::*;
use signex_types::schematic::BodyStyle;
use std::fmt;

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

impl fmt::Display for BodyChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Normal => "Normal",
            Self::Alternate => "Alternate (De Morgan)",
            Self::AlternateMissing => "Alternate (not in symbol; drawn as Normal)",
        })
    }
}

/// Append the Body row for the panel's `Body Style` state (empty = not a
/// library symbol, no row).
pub(super) fn push_body_row<'a>(
    c: Column<'a, PanelMsg>,
    id: uuid::Uuid,
    state: &str,
    muted: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Column<'a, PanelMsg> {
    let pick = move |choice: BodyChoice| PanelMsg::SymbolBodyStylePicked(id, choice.style());
    match state {
        BODY_NORMAL_CAN_SWITCH => c.push(form_pick_row(
            "Body",
            vec![BodyChoice::Normal, BodyChoice::Alternate],
            BodyChoice::Normal,
            pick,
            muted,
        )),
        BODY_ALTERNATE => c.push(form_pick_row(
            "Body",
            vec![BodyChoice::Normal, BodyChoice::Alternate],
            BodyChoice::Alternate,
            pick,
            muted,
        )),
        // The only way out of a dangling alternate is back to Normal.
        BODY_ALTERNATE_MISSING => c.push(form_pick_row(
            "Body",
            vec![BodyChoice::Normal],
            BodyChoice::AlternateMissing,
            pick,
            muted,
        )),
        BODY_NORMAL => c.push(iced::widget::tooltip(
            form_input_row("Body", "Normal", muted, input_bg, input_bdr),
            iced::widget::text("This library symbol has no alternate (De Morgan) body").size(11),
            iced::widget::tooltip::Position::Bottom,
        )),
        _ => c,
    }
}
