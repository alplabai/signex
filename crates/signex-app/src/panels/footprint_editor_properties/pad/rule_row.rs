//! The Rule | Manual row of an expansion (mockup row 125, Caner,
//! 2026-11-19): Rule
//! shows a read-only cell tagged "rule" with no number (a library footprint
//! has no board rule to resolve), Manual an editable mm field. Picking Rule
//! clears the value (`""`), picking Manual while the rule decides stores 0.

use iced::{Element, Length};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::Label;
use signex_widgets::property_illustration::{FieldId, IllustrationHover};
use signex_widgets::segmented;

use super::super::super::{PanelMsg, PanelPalette};
use super::super::illustration::linked_field;
use super::field::{
    Shown, labelled_row, number_input, pair, read_only_cell, segment_style, with_refusals,
};

/// Label and value widths of a row under an enable header: a short label,
/// so the Rule | Manual segment and the value share the rest.
const SUB_ROW_WIDTHS: (Length, Length) = (Length::FillPortion(3), Length::FillPortion(7));

/// `label`, a Rule | Manual segment and the value: the rule tag while the
/// value is `None`, else an mm field (showing `buffer`, refused text, when
/// there is one). `on_input` receives the typed text, `""` for Rule and
/// `"0"` for Manual. Linked to `field` in the illustration.
#[allow(
    clippy::too_many_arguments,
    reason = "the shared row contract of the footprint pad and sketch pad forms"
)]
pub(in crate::panels::footprint_editor_properties) fn rule_or_manual_row<'a>(
    label: Localized,
    field: FieldId,
    value_mm: Option<f64>,
    buffer: Option<String>,
    on_input: impl Fn(String) -> PanelMsg + Clone + 'a,
    palette: PanelPalette,
    tokens: &ThemeTokens,
    hover: &IllustrationHover,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let is_manual = value_mm.is_some() || buffer.is_some();
    let pick = on_input.clone();
    let source = segmented(
        [
            (false, Label::from(t!(i18n, ids::EDITORS_FP_PAD_RULE))),
            (true, Label::from(t!(i18n, ids::EDITORS_FP_PAD_MANUAL))),
        ],
        is_manual,
        segment_style(tokens),
        move |manual: bool| source_picked(manual, is_manual, &pick),
    );
    let stored = value_mm.map(|v| format!("{v:.3}")).unwrap_or_default();
    let shown = Shown::from_buffer(buffer.as_ref(), stored, false, |_| true);
    let value = if is_manual {
        number_input(&shown, on_input, tokens)
    } else {
        read_only_cell(t!(i18n, ids::EDITORS_FP_PAD_RULE_TAG), tokens)
    };
    let row = labelled_row(label, pair(source, value), SUB_ROW_WIDTHS, palette.muted);
    linked_field(
        with_refusals(row, &[shown.refusal], tokens, i18n),
        field,
        hover,
        tokens,
    )
}

/// A value the board's rule always decides for a library pad: `label` and
/// the read-only rule tag (per-pad values need a model field).
pub(super) fn rule_only_row<'a>(
    label: Localized,
    palette: PanelPalette,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let value = read_only_cell(t!(i18n, ids::EDITORS_FP_PAD_RULE_TAG), tokens);
    labelled_row(label, value, SUB_ROW_WIDTHS, palette.muted)
}

/// The message of a Rule | Manual pick: nothing when the pick is the
/// current source, `""` for Rule and `"0"` for Manual.
fn source_picked(
    manual: bool,
    is_manual: bool,
    on_input: &impl Fn(String) -> PanelMsg,
) -> PanelMsg {
    match (manual, is_manual) {
        (true, true) | (false, false) => PanelMsg::Noop,
        (true, false) => on_input("0".to_string()),
        (false, true) => on_input(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::source_picked;
    use crate::panels::PanelMsg;

    fn typed(msg: PanelMsg) -> Option<String> {
        match msg {
            PanelMsg::FpEditorSetNextPadPasteMarginTop(text) => Some(text),
            _ => None,
        }
    }

    /// Rule clears the value, Manual stores 0 while the rule decided, and
    /// picking the current source sends nothing.
    #[test]
    fn rule_sends_an_empty_value_and_manual_sends_zero() {
        let on_input = PanelMsg::FpEditorSetNextPadPasteMarginTop;
        assert_eq!(
            typed(source_picked(false, true, &on_input)),
            Some(String::new())
        );
        assert_eq!(
            typed(source_picked(true, false, &on_input)),
            Some("0".into())
        );
        assert!(matches!(
            source_picked(true, true, &on_input),
            PanelMsg::Noop
        ));
        assert!(matches!(
            source_picked(false, false, &on_input),
            PanelMsg::Noop
        ));
    }
}
