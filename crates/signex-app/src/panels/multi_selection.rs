//! Properties of a multi-selection (#1258): the fields every selected object
//! of a kind shares, editable at once. The grouping and the bulk edit live in
//! `signex_engine::multi_edit`; this is the view and the text parsing.

use super::*;
use crate::numeric_field::{FieldNumber, parse_field_number};
use crate::styles::{Spacing, TypeScale};
use iced::widget::{checkbox, text_input};
use signex_engine::{MultiEdit, Shared};
use signex_types::schematic::{HAlign, SCHEMATIC_PT_TO_MM};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Shown in place of a value the selection disagrees on.
pub const MIXED: &str = "<mixed>";

/// A text box of the multi-selection form, the key of its draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiField {
    SymbolValue,
    SymbolFootprint,
    SymbolParameter(String),
    LabelFontPt,
    WireWidth,
}

impl MultiField {
    /// How the field is named in a refusal line.
    fn name(&self) -> String {
        match self {
            Self::SymbolValue => "Value".into(),
            Self::SymbolFootprint => "Footprint".into(),
            Self::SymbolParameter(key) => format!("Parameter {key}"),
            Self::LabelFontPt => "Label font size".into(),
            Self::WireWidth => "Wire width".into(),
        }
    }
}

/// Multi-selection form messages, namespaced under `PanelMsg::MultiEdit`.
#[derive(Debug, Clone)]
pub enum MultiEditMsg {
    /// A keystroke in a text box; applied on Enter, not per key, so one
    /// edit is one undo step.
    DraftChanged(MultiField, String),
    /// Enter in a text box.
    DraftSubmitted(MultiField),
    /// A checkbox or segmented button: applied at once.
    Picked(MultiEdit),
}

/// The edit the text `typed` in `field` asks for, or why it cannot apply.
pub fn edit_from_text(field: &MultiField, typed: &str) -> Result<MultiEdit, String> {
    let number = || match parse_field_number(typed) {
        FieldNumber::Number(v) => Ok(v),
        FieldNumber::NotANumber | FieldNumber::Refused => {
            Err(format!("{}: '{typed}' is not a number", field.name()))
        }
    };
    let non_negative = |what: &str| {
        number().and_then(|v| {
            (v >= 0.0)
                .then_some(v)
                .ok_or_else(|| format!("{what} cannot be negative, got '{typed}'"))
        })
    };
    Ok(match field {
        MultiField::SymbolValue => MultiEdit::SymbolValue(typed.to_owned()),
        MultiField::SymbolFootprint => MultiEdit::SymbolFootprint(typed.to_owned()),
        MultiField::SymbolParameter(key) => MultiEdit::SymbolParameter {
            key: key.clone(),
            value: typed.to_owned(),
        },
        MultiField::LabelFontPt => {
            let pt = non_negative("Label font size")?;
            if pt <= 0.0 {
                return Err(format!("Label font size must be above 0, got '{typed}'"));
            }
            MultiEdit::LabelFontSizeMm(pt * SCHEMATIC_PT_TO_MM)
        }
        MultiField::WireWidth => MultiEdit::WireWidthMm(non_negative("Wire width")?),
    })
}

/// A number without trailing zeros (`1.27`, `10`).
fn number_text(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn shared_text(shared: &Shared<String>) -> String {
    shared.same().cloned().unwrap_or_default()
}

fn shared_number(shared: &Shared<f64>, scale: f64) -> String {
    shared
        .same()
        .map(|v| number_text(v / scale))
        .unwrap_or_default()
}

/// The selection's groups as the panel shows them, with the draft in
/// progress (if any).
pub struct MultiView<'a> {
    pub draft: Option<&'a (MultiField, String)>,
}

impl MultiView<'_> {
    fn text_row<'a>(
        &self,
        label: &str,
        field: MultiField,
        stored: String,
        mixed: bool,
        muted: Color,
    ) -> Element<'a, PanelMsg> {
        let drafted = self
            .draft
            .filter(|(f, _)| *f == field)
            .map(|(_, t)| t.clone());
        let shown = drafted.unwrap_or(stored);
        let (changed, submitted) = (field.clone(), field);
        container(
            row![
                property_label(label.to_string(), muted),
                text_input(if mixed { MIXED } else { "" }, &shown)
                    .on_input(move |s| {
                        PanelMsg::MultiEdit(MultiEditMsg::DraftChanged(changed.clone(), s))
                    })
                    .on_submit(PanelMsg::MultiEdit(MultiEditMsg::DraftSubmitted(submitted)))
                    .size(TS.body)
                    .padding(SP.s)
                    .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, PROPERTY_ROW_PAD_X])
        .width(Length::Fill)
        .into()
    }

    fn string_row<'a>(
        &self,
        label: &str,
        field: MultiField,
        shared: &Shared<String>,
        muted: Color,
    ) -> Element<'a, PanelMsg> {
        let mixed = matches!(shared, Shared::Mixed);
        self.text_row(label, field, shared_text(shared), mixed, muted)
    }

    fn number_row<'a>(
        &self,
        label: &str,
        field: MultiField,
        shared: &Shared<f64>,
        scale: f64,
        muted: Color,
    ) -> Element<'a, PanelMsg> {
        let mixed = matches!(shared, Shared::Mixed);
        self.text_row(label, field, shared_number(shared, scale), mixed, muted)
    }
}

/// A checkbox over a flag the group may disagree on; toggling a mixed flag
/// sets it on every object.
fn flag_row<'a>(
    label: &str,
    shared: &Shared<bool>,
    edit: fn(bool) -> MultiEdit,
    muted: Color,
    primary: Color,
) -> Element<'a, PanelMsg> {
    let (checked, state) = match shared {
        Shared::Same(true) => (true, "On"),
        Shared::Same(false) => (false, "Off"),
        Shared::Mixed => (false, MIXED),
    };
    container(
        row![
            property_label(label.to_string(), muted),
            row![
                checkbox(checked)
                    .on_toggle(move |on| PanelMsg::MultiEdit(MultiEditMsg::Picked(edit(on))))
                    .size(TS.heading)
                    .spacing(SP.s),
                text(state)
                    .size(TS.body)
                    .color(if checked { primary } else { muted }),
            ]
            .spacing(SP.m)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

fn justify_row<'a>(
    shared: &Shared<HAlign>,
    muted: Color,
    ctx: &PanelContext,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let hover = crate::styles::ti(ctx.tokens.hover);
    let accent = crate::styles::ti(ctx.tokens.accent);
    let text_inactive = crate::styles::ti(ctx.tokens.text_secondary);
    let button = |label: &str, to: HAlign| {
        seg_btn(
            label,
            shared.same() == Some(&to),
            PanelMsg::MultiEdit(MultiEditMsg::Picked(MultiEdit::LabelJustify(to))),
            accent,
            Color::WHITE,
            text_inactive,
            hover,
            border_c,
        )
    };
    let label = if matches!(shared, Shared::Mixed) {
        format!("Justify {MIXED}")
    } else {
        "Justify".to_owned()
    };
    container(
        row![
            property_label(label, muted),
            row![
                button("Left", HAlign::Left),
                button("Center", HAlign::Center),
                button("Right", HAlign::Right),
            ]
            .spacing(SP.xs)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// The four turns a label offers, as in the single-label panel; a mixed
/// selection shows none chosen.
const LABEL_TURNS: [f64; 4] = [0.0, 90.0, 180.0, 270.0];

fn rotation_row<'a>(
    shared: &Shared<f64>,
    muted: Color,
    ctx: &PanelContext,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let hover = crate::styles::ti(ctx.tokens.hover);
    let accent = crate::styles::ti(ctx.tokens.accent);
    let text_inactive = crate::styles::ti(ctx.tokens.text_secondary);
    let buttons = LABEL_TURNS.map(|deg| {
        seg_btn(
            &format!("{deg:.0}"),
            shared.same().is_some_and(|v| (v - deg).abs() < 1e-6),
            PanelMsg::MultiEdit(MultiEditMsg::Picked(MultiEdit::LabelRotationDegrees(deg))),
            accent,
            Color::WHITE,
            text_inactive,
            hover,
            border_c,
        )
    });
    let label = if matches!(shared, Shared::Mixed) {
        format!("Rotation {MIXED}")
    } else {
        "Rotation".to_owned()
    };
    container(
        row![
            property_label(label, muted),
            row(buttons)
                .spacing(SP.xs)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .into()
}

/// The multi-selection Properties body.
pub fn view_multi_selection<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let view = MultiView {
        draft: ctx.multi_draft.as_ref(),
    };
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);
    col = col.push(
        container(text("Multi-Selection").size(TS.body).color(primary))
            .padding([6.0, SP.m])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(
            text(format!("{} objects selected", ctx.selection_count))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.m]),
    );
    if let Some(g) = &ctx.multi_groups.symbols {
        col = col.push(section_hdr(
            &format!("Symbols ({})", g.count),
            primary,
            border_c,
        ));
        col = col.push(view.string_row("Value", MultiField::SymbolValue, &g.value, muted));
        col = col.push(view.string_row(
            "Footprint",
            MultiField::SymbolFootprint,
            &g.footprint,
            muted,
        ));
        col = col.push(flag_row(
            "DNP",
            &g.dnp,
            MultiEdit::SymbolDnp,
            muted,
            primary,
        ));
        col = col.push(flag_row(
            "In BOM",
            &g.in_bom,
            MultiEdit::SymbolInBom,
            muted,
            primary,
        ));
        col = col.push(flag_row(
            "On board",
            &g.on_board,
            MultiEdit::SymbolOnBoard,
            muted,
            primary,
        ));
        for (key, shared) in &g.parameters {
            col = col.push(view.string_row(
                key,
                MultiField::SymbolParameter(key.clone()),
                shared,
                muted,
            ));
        }
    }
    if let Some(g) = &ctx.multi_groups.labels {
        col = col.push(section_hdr(
            &format!("Labels ({})", g.count),
            primary,
            border_c,
        ));
        col = col.push(view.number_row(
            "Size (pt)",
            MultiField::LabelFontPt,
            &g.font_size_mm,
            SCHEMATIC_PT_TO_MM,
            muted,
        ));
        col = col.push(justify_row(&g.justify, muted, ctx, border_c));
        col = col.push(rotation_row(&g.rotation_degrees, muted, ctx, border_c));
    }
    if let Some(g) = &ctx.multi_groups.wires {
        col = col.push(section_hdr(
            &format!("Wires ({})", g.count),
            primary,
            border_c,
        ));
        col =
            col.push(view.number_row("Width (mm)", MultiField::WireWidth, &g.width_mm, 1.0, muted));
    }
    if ctx.multi_groups.is_empty() {
        col = col.push(
            container(
                text("Nothing in this selection has fields shared for editing")
                    .size(TS.caption)
                    .color(muted),
            )
            .padding([SP.s, SP.m]),
        );
    }
    scrollable(col).width(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_trimmed_and_text_is_taken_verbatim() {
        assert_eq!(number_text(1.27), "1.27");
        assert_eq!(number_text(10.0), "10");
        assert_eq!(
            edit_from_text(&MultiField::SymbolValue, " 4k7").unwrap(),
            MultiEdit::SymbolValue(" 4k7".into())
        );
        assert_eq!(
            edit_from_text(&MultiField::SymbolValue, "").unwrap(),
            MultiEdit::SymbolValue(String::new())
        );
    }

    #[test]
    fn font_size_converts_points_and_refuses_zero_negative_and_words() {
        let MultiEdit::LabelFontSizeMm(mm) =
            edit_from_text(&MultiField::LabelFontPt, "10").unwrap()
        else {
            panic!("a font size edit");
        };
        assert!((mm - 10.0 * SCHEMATIC_PT_TO_MM).abs() < 1e-9);
        for bad in ["0", "-3", "big", ""] {
            assert!(
                edit_from_text(&MultiField::LabelFontPt, bad).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn wire_width_refuses_negative_and_zero_is_the_default() {
        assert!(edit_from_text(&MultiField::WireWidth, "-0.1").is_err());
        assert_eq!(
            edit_from_text(&MultiField::WireWidth, "0").unwrap(),
            MultiEdit::WireWidthMm(0.0)
        );
    }
}
