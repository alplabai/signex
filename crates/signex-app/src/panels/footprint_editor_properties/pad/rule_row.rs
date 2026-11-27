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
    let stored = value_mm.map(|v| format!("{v:.3}")).unwrap_or_default();
    let shown = Shown::from_buffer(buffer.as_ref(), stored, false, |_| true);
    let row = RuleRow {
        label,
        field,
        is_manual,
        seed: "0",
    };
    row.view(shown, on_input, palette, tokens, hover, i18n)
}

/// A Rule | Manual row whose Manual value is not 0 when picked (#2593: an
/// air gap, spoke width or spoke count), shown as `shown`.
pub(in crate::panels::footprint_editor_properties) struct RuleRow {
    pub label: Localized,
    pub field: FieldId,
    /// Manual: a value is stored or refused text is shown.
    pub is_manual: bool,
    /// The text picking Manual sends.
    pub seed: &'static str,
}

impl RuleRow {
    pub(in crate::panels::footprint_editor_properties) fn view<'a>(
        self,
        shown: Shown,
        on_input: impl Fn(String) -> PanelMsg + Clone + 'a,
        palette: PanelPalette,
        tokens: &ThemeTokens,
        hover: &IllustrationHover,
        i18n: &I18n,
    ) -> Element<'a, PanelMsg> {
        let (is_manual, seed) = (self.is_manual, self.seed);
        let pick = on_input.clone();
        let source = segmented(
            [
                (false, Label::from(t!(i18n, ids::EDITORS_FP_PAD_RULE))),
                (true, Label::from(t!(i18n, ids::EDITORS_FP_PAD_MANUAL))),
            ],
            is_manual,
            segment_style(tokens),
            move |manual: bool| source_picked_with(manual, is_manual, seed, &pick),
        );
        let value = if is_manual {
            number_input(&shown, on_input, tokens)
        } else {
            read_only_cell(t!(i18n, ids::EDITORS_FP_PAD_RULE_TAG), tokens)
        };
        let row = labelled_row(
            self.label,
            pair(source, value),
            SUB_ROW_WIDTHS,
            palette.muted,
        );
        linked_field(
            with_refusals(row, &[shown.refusal], tokens, i18n),
            self.field,
            hover,
            tokens,
        )
    }
}

/// The message of a Rule | Manual pick of an expansion row: nothing when
/// the pick is the current source, `""` for Rule and `"0"` for Manual.
#[cfg(test)]
fn source_picked(
    manual: bool,
    is_manual: bool,
    on_input: &impl Fn(String) -> PanelMsg,
) -> PanelMsg {
    source_picked_with(manual, is_manual, "0", on_input)
}

/// [`source_picked`] with `seed` as the text Manual sends.
fn source_picked_with(
    manual: bool,
    is_manual: bool,
    seed: &str,
    on_input: &impl Fn(String) -> PanelMsg,
) -> PanelMsg {
    match (manual, is_manual) {
        (true, true) | (false, false) => PanelMsg::Noop,
        (true, false) => on_input(seed.to_string()),
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

    /// #2593: a thermal relief row's Manual sends its seed, not 0, which no
    /// pour can use.
    #[test]
    fn manual_sends_the_rows_seed() {
        let on_input = PanelMsg::FpEditorSetNextPadPasteMarginTop;
        assert_eq!(
            typed(super::source_picked_with(true, false, "0.508", &on_input)),
            Some("0.508".into())
        );
    }
}
