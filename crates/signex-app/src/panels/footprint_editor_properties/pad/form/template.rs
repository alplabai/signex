//! The pad's Template dropdown (mockup row 125): "(custom)", then the
//! templates the footprint's pads use, with the template's library as a tag.
//! A name no other pad uses stays editable in a Name field under it.

use iced::widget::{Column, Row, container, pick_list};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, text};

use super::super::super::super::{PadOutlineSummary, PanelMsg};
use super::{Form, PadEditTarget, pad_input_row_w, pad_template_msg};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One row of the dropdown; both fields empty is "(custom)".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::panels::footprint_editor_properties) struct TemplateChoice {
    pub(super) template: String,
    pub(super) library: String,
}

impl TemplateChoice {
    fn custom() -> Self {
        Self {
            template: String::new(),
            library: String::new(),
        }
    }

    fn is_custom(&self) -> bool {
        self.template.is_empty()
    }
}

/// What the dropdown offers for one pad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::panels::footprint_editor_properties) struct TemplateChoices {
    /// "(custom)" first, then every template in name order.
    pub(super) rows: Vec<TemplateChoice>,
    /// The pad's own row.
    pub(super) current: TemplateChoice,
    /// Another pad uses this template, so it is picked and not typed.
    pub(super) is_shared: bool,
}

/// The dropdown for a pad holding `template` from `library`. `own` is its
/// index in `pads`; `None` for the next pad placed.
pub(super) fn choices(
    pads: &[PadOutlineSummary],
    own: Option<usize>,
    template: &str,
    library: &str,
) -> TemplateChoices {
    let current = TemplateChoice {
        template: template.to_string(),
        library: library.to_string(),
    };
    let others: Vec<TemplateChoice> = pads
        .iter()
        .enumerate()
        .filter(|(i, pad)| Some(*i) != own && !pad.template.is_empty())
        .map(|(_, pad)| TemplateChoice {
            template: pad.template.clone(),
            library: pad.template_library.clone(),
        })
        .collect();
    let is_shared = others.contains(&current);
    let mut named = others;
    if !current.is_custom() {
        named.push(current.clone());
    }
    named.sort();
    named.dedup();
    let mut rows = vec![TemplateChoice::custom()];
    rows.append(&mut named);
    TemplateChoices {
        rows,
        current,
        is_shared,
    }
}

fn pick_msg(target: PadEditTarget, choice: TemplateChoice) -> PanelMsg {
    let TemplateChoice { template, library } = choice;
    match target {
        PadEditTarget::Next => PanelMsg::FpEditorPickNextPadTemplate { template, library },
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorPickSelectedPadTemplate {
            idx,
            template,
            library,
        },
    }
}

/// The Template row, and the Name field while the name is the pad's own.
pub(super) fn push<'a>(
    col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
) -> Column<Element<'a, PanelMsg>> {
    let (v, target, i18n) = (form.values, form.target, form.i18n);
    let (label_w, field_w) = form.fw();
    let muted = form.palette.muted;
    let offered = &v.template_choices;
    let custom = t!(i18n, ids::EDITORS_PAD_TEMPLATE_CUSTOM).into_string();
    let dropdown = pick_list(
        Some(offered.current.clone()),
        offered.rows.clone(),
        move |choice: &TemplateChoice| {
            if choice.is_custom() {
                custom.clone()
            } else {
                choice.template.clone()
            }
        },
    )
    .graphite()
    .on_select(move |choice| pick_msg(target, choice))
    .padding([SP.xs, SP.m])
    .width(Length::Fill);
    let mut control = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .width(field_w)
        .push(dropdown.boxed());
    if !offered.current.is_custom() {
        let tag: Label<'a> = if offered.current.library.is_empty() {
            t!(i18n, ids::EDITORS_PAD_TEMPLATE_LIBRARY).into()
        } else {
            UserText::new(offered.current.library.clone()).into()
        };
        control = control.push(text(tag).size(TS.caption).color(muted).boxed());
    }
    let line = Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(
            text(t!(i18n, ids::EDITORS_PAD_TEMPLATE))
                .size(TS.caption)
                .color(muted)
                .width(label_w)
                .boxed(),
        )
        .push(control.boxed());
    let col = col.push(
        container(line)
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    if offered.is_shared {
        return col;
    }
    col.push(pad_input_row_w(
        form.fw(),
        t!(i18n, ids::EDITORS_PAD_TEMPLATE_NAME),
        "",
        v.template.clone(),
        move |s| pad_template_msg(target, s),
        muted,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(template: &str, library: &str) -> PadOutlineSummary {
        PadOutlineSummary {
            position_mm: [0.0, 0.0],
            size_mm: [1.0, 1.0],
            rotation_deg: 0.0,
            corners_mm: [[0.0, 0.0]; 4],
            shape: signex_library::PadShape::Rect,
            has_drill: false,
            number: "1".into(),
            template: template.into(),
            template_library: library.into(),
        }
    }

    /// Mockup row 125: "(custom)" first, each template once, in name order.
    #[test]
    fn the_dropdown_lists_custom_then_the_footprints_templates() {
        let pads = [pad("rr155x60", "Lib"), pad("c180", ""), pad("rr155x60", "Lib")];
        let offered = choices(&pads, Some(1), "c180", "");
        let names: Vec<&str> = offered.rows.iter().map(|c| c.template.as_str()).collect();
        assert_eq!(names, ["", "c180", "rr155x60"]);
        assert!(!offered.is_shared, "only this pad uses c180");
    }

    #[test]
    fn a_template_another_pad_uses_is_shared() {
        let pads = [pad("rr155x60", "Lib"), pad("rr155x60", "Lib")];
        assert!(choices(&pads, Some(0), "rr155x60", "Lib").is_shared);
        assert!(choices(&pads, None, "rr155x60", "Lib").is_shared);
    }

    /// Picking a row sets the name and its library in one message.
    #[test]
    fn a_pick_carries_the_name_and_its_library() {
        let picked = TemplateChoice {
            template: "rr155x60".into(),
            library: "Lib".into(),
        };
        assert!(matches!(
            pick_msg(PadEditTarget::Selected(3), picked),
            PanelMsg::FpEditorPickSelectedPadTemplate { idx: 3, template, library }
                if template == "rr155x60" && library == "Lib"
        ));
    }
}
