//! The pad header (#125): "Pad 3" with its kind pill, then "3 of 8 pads ·
//! Edit in Sketch ▸". While a pad is being placed: "Next pad" and the TAB
//! pause hint.

use iced::widget::{Row, column, container};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::super::super::{FootprintEditorPanelContext, FootprintModeKind, PanelMsg};
use super::form::PadEditTarget;
use crate::library::editor::footprint::state::PadSide;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The two header lines of a selected pad, as the catalog renders them.
pub(in crate::panels::footprint_editor_properties) struct PadHeading {
    /// "Pad 3".
    pub(in crate::panels::footprint_editor_properties) title: Localized,
    /// The kind pill: "SMD", "Through-hole", "NPTH"…
    pub(in crate::panels::footprint_editor_properties) kind: Localized,
    /// "3 of 8 pads".
    pub(in crate::panels::footprint_editor_properties) place: Localized,
}

/// The header of the selected pad outside placement. `None` keeps the
/// footprint header: no pad selected, a placement running, or an index past
/// the pad count.
pub(in crate::panels::footprint_editor_properties) fn selected_pad_heading(
    fp: &FootprintEditorPanelContext,
    i18n: &I18n,
) -> Option<PadHeading> {
    if fp.mode_kind != FootprintModeKind::Pads || fp.placement_active || fp.placement_paused {
        return None;
    }
    let pad = fp.selected_pad.as_ref()?;
    // The editor's own pads, unsaved ones included: the list `idx` indexes.
    if pad.idx >= fp.editor_pad_count {
        return None;
    }
    Some(PadHeading {
        title: t!(i18n, ids::EDITORS_FP_PAD_TITLE, "number" => pad.number.as_str()),
        kind: t!(i18n, pill_message(pad.kind, pad.side)),
        place: t!(
            i18n,
            ids::EDITORS_FP_PAD_PLACE,
            "index" => pad.idx + 1,
            "pads" => fp.editor_pad_count
        ),
    })
}

/// The catalog name of a pad kind, as the header pill shows it. A plain
/// pad reads SMD or Through-hole by its layer, as the form below it does:
/// the Layer picker is what makes a pad Multi-Layer.
fn pill_message(kind: signex_library::PadKind, side: PadSide) -> MessageId {
    use signex_library::PadKind;
    match kind {
        PadKind::Smd | PadKind::Tht if side == PadSide::All => ids::EDITORS_PAD_KIND_THROUGH_HOLE,
        PadKind::Smd | PadKind::Tht => ids::EDITORS_PAD_KIND_SMD,
        PadKind::NptHole => ids::EDITORS_FP_PAD_PILL_NPTH,
        PadKind::ConnectorPad => ids::EDITORS_PAD_KIND_CONNECTOR,
        PadKind::Castellated => ids::EDITORS_PAD_KIND_CASTELLATED,
        PadKind::Fiducial => ids::EDITORS_PAD_KIND_FIDUCIAL,
        // `PadKind` is non-exhaustive.
        _ => ids::EDITORS_PAD_KIND_UNKNOWN,
    }
}

/// The header of `target`: the selected pad, or the next pad placed.
pub(in crate::panels::footprint_editor_properties) fn view_pad_header<'a>(
    fp: &'a FootprintEditorPanelContext,
    target: PadEditTarget,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let faint = resolve(tokens).ink4;
    let (title, kind, second): (Localized, MessageId, Element<'a, PanelMsg>) = match target {
        PadEditTarget::Next => {
            let (hint, ink) = if fp.placement_paused {
                (ids::EDITORS_PAD_PLACEMENT_PAUSED, ti(tokens.warning))
            } else {
                (ids::EDITORS_PAD_PLACEMENT_PAUSE_HINT, faint)
            };
            (
                t!(i18n, ids::EDITORS_FP_PAD_NEXT_TITLE),
                pill_message(fp.next_pad_kind, fp.next_pad_side),
                text(t!(i18n, hint)).size(TS.caption).color(ink).boxed(),
            )
        }
        PadEditTarget::Selected(pad_idx) => {
            let (number, kind) = fp
                .selected_pad
                .as_ref()
                .map(|pad| (pad.number.as_str(), pill_message(pad.kind, pad.side)))
                .unwrap_or(("", ids::EDITORS_PAD_KIND_SMD));
            let mut line: Row<Element<'a, PanelMsg>> =
                Row::new().spacing(SP.xs).align_y(iced::Alignment::Center);
            if let Some(heading) = selected_pad_heading(fp, i18n) {
                line = line
                    .push(text(heading.place).size(TS.caption).color(faint).boxed())
                    .push(
                        text(t!(i18n, ids::EDITORS_FP_PAD_SEPARATOR))
                            .size(TS.caption)
                            .color(faint)
                            .boxed(),
                    );
            }
            line = line.push(edit_in_sketch(pad_idx, tokens, i18n));
            (
                t!(i18n, ids::EDITORS_FP_PAD_TITLE, "number" => number),
                kind,
                line.boxed(),
            )
        }
    };
    let font = crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT);
    let mut first: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .push(
            text(title)
                .size(TS.label)
                .color(ti(tokens.text))
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..font
                })
                .boxed(),
        )
        .push(
            container(text(t!(i18n, kind)).size(TS.caption))
                .padding([0.0, SP.s])
                .style(crate::styles::pill(tokens, PillTone::Neutral))
                .boxed(),
        );
    // Several pads selected: the form shows the primary pad only.
    if fp.selected_pad_count > 1 {
        first = first.push(
            text(t!(i18n, ids::EDITORS_FOOTPRINT_PADS_SELECTED, "count" => fp.selected_pad_count))
                .size(TS.caption)
                .color(ti(tokens.accent))
                .boxed(),
        );
    }
    container(column![first, second].spacing(SP.xs))
        .padding(iced::Padding {
            top: SP.m,
            right: SP.m,
            bottom: SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// The "Edit in Sketch ▸" link: opens the pad's profile in Sketch mode
/// (a no-op for a pad without one).
fn edit_in_sketch<'a>(pad_idx: usize, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (ink, hover) = (r.accent_text, r.accent_soft);
    button(text(t!(i18n, ids::EDITORS_FP_PAD_EDIT_IN_SKETCH)).size(TS.caption))
        .padding([0.0, SP.xs])
        .on_press(PanelMsg::FpEditorEditPadInSketch { pad_idx })
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        })
        .boxed()
}
