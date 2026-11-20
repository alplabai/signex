//! Pad Features (mockup row 125): closed until opened. The surface finish of each
//! side, then the testpoint flags as a two-column checkbox grid.

use iced::widget::{Column, container, row};
use iced::{Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_sketch::attr::PadFeature;
use signex_widgets::{CheckboxLabel as _, button, text};

use super::super::super::super::{CollapsedSections, PanelMsg};
use super::{
    Form, pad_feature_bottom_msg, pad_feature_top_msg, pad_pick_row_w,
    pad_testpoint_bottom_assembly_msg, pad_testpoint_bottom_fab_msg,
    pad_testpoint_top_assembly_msg, pad_testpoint_top_fab_msg,
};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The section's key in the collapsed set marks it OPEN: the section starts
/// closed, and `PanelMsg::ToggleSection` flips the key either way.
const OPEN_KEY: &str = "fp_pad_features_open";

/// Whether Pad Features is open: only once the user opened it.
pub(super) fn is_open(collapsed: &CollapsedSections) -> bool {
    collapsed.contains(OPEN_KEY)
}

pub(super) fn push<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
) -> Column<Element<'a, PanelMsg>> {
    let open = is_open(form.collapsed);
    col = col.push(header(
        t!(form.i18n, ids::EDITORS_FP_PAD_SECTION_FEATURES),
        open,
        form,
    ));
    if !open {
        return col;
    }
    let (v, target, i18n) = (form.values, form.target, form.i18n);
    let (fw, muted) = (form.fw(), form.palette.muted);
    col = col.push(pad_pick_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_TOP_SIDE),
        PadFeature::ALL,
        v.feature_top,
        move |f: PadFeature| pad_feature_top_msg(target, f),
        muted,
    ));
    col = col.push(pad_pick_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_BOTTOM_SIDE),
        PadFeature::ALL,
        v.feature_bottom,
        move |f: PadFeature| pad_feature_bottom_msg(target, f),
        muted,
    ));
    col = col.push(
        container(
            text(t!(i18n, ids::EDITORS_PAD_TESTPOINT))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed(),
    );
    let tp = v.testpoint;
    let check = |label: Localized, on: bool, msg: fn(super::PadEditTarget, bool) -> PanelMsg| {
        signex_widgets::checkbox(on)
            .labelled(label)
            .on_toggle(move |value| msg(target, value))
            .size(TS.label)
            .text_size(TS.caption)
            .spacing(SP.s)
            .width(Length::Fill)
            .style(crate::panels::form_look::checkbox_style)
            .boxed()
    };
    let grid = Column::new()
        .spacing(SP.xs)
        .push(
            row![
                check(
                    t!(i18n, ids::EDITORS_PAD_TOP_ASSEMBLY),
                    tp.top_assembly,
                    pad_testpoint_top_assembly_msg,
                ),
                check(
                    t!(i18n, ids::EDITORS_PAD_TOP_FAB),
                    tp.top_fab,
                    pad_testpoint_top_fab_msg,
                ),
            ]
            .spacing(SP.m),
        )
        .push(
            row![
                check(
                    t!(i18n, ids::EDITORS_PAD_BOTTOM_ASSEMBLY),
                    tp.bottom_assembly,
                    pad_testpoint_bottom_assembly_msg,
                ),
                check(
                    t!(i18n, ids::EDITORS_PAD_BOTTOM_FAB),
                    tp.bottom_fab,
                    pad_testpoint_bottom_fab_msg,
                ),
            ]
            .spacing(SP.m),
        );
    col.push(
        container(grid)
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    )
}

/// A section header like the panel's others, open when `open`.
fn header<'a>(title: Localized, open: bool, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let (ink, rule) = (form.palette.primary, form.palette.border);
    let label = row![
        signex_widgets::chevron::chevron(open, TS.label, ink),
        text(title).size(TS.label).color(ink),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let head = button(container(label).padding([SP.s, SP.m]).width(Length::Fill))
        .padding(0)
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(OPEN_KEY.to_string()))
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered)
                .then_some(iced::Background::Color(rule)),
            border: Border::default(),
            ..button::Style::default()
        });
    Column::new()
        .width(Length::Fill)
        .push(head.boxed())
        .push(crate::panels::thin_sep(rule))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pad Features starts closed and opens once its key is toggled in.
    #[test]
    fn pad_features_start_closed() {
        let mut collapsed = CollapsedSections::new();
        assert!(!is_open(&collapsed));
        collapsed.insert(OPEN_KEY.to_string());
        assert!(is_open(&collapsed));
    }
}
