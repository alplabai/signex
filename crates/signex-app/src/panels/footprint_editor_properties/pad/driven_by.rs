//! The "Driven by sketch" section of a baked pad (mockup row 192, item D5):
//! its source geometry, its pattern instance and its size. Read only.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::text;

use super::super::super::{FootprintEditorPanelContext, PanelMsg, PanelPalette};
use super::super::rows::value_row;
use super::form::PadEditTarget;
use crate::library::editor::footprint::{PadInstance, PadPatternKind, PadSketchSource};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The source row's value.
pub(in crate::panels::footprint_editor_properties) fn source_text(
    source: PadSketchSource,
    i18n: &I18n,
) -> String {
    if source.edges == 0 {
        return t!(i18n, ids::EDITORS_FP_PAD_DRIVEN_POINT).into_string();
    }
    t!(i18n, ids::EDITORS_FP_PAD_DRIVEN_PROFILE, "edges" => source.edges).into_string()
}

/// The instance row's value.
pub(in crate::panels::footprint_editor_properties) fn instance_text(
    instance: Option<PadInstance>,
    i18n: &I18n,
) -> String {
    let Some(instance) = instance else {
        return t!(i18n, ids::EDITORS_FP_PAD_DRIVEN_SINGLE).into_string();
    };
    let kind = match instance.kind {
        PadPatternKind::Linear => "linear",
        PadPatternKind::Grid => "grid",
        PadPatternKind::Circular => "circular",
        PadPatternKind::Mirror => "mirror",
    };
    let place = match instance.row {
        Some(row) => format!("{} · {row}", instance.index),
        None => instance.index.to_string(),
    };
    t!(i18n, ids::EDITORS_FP_PAD_DRIVEN_PATTERN, "kind" => kind, "place" => place).into_string()
}

/// The section of the selected pad; `None` for the next pad or a pad with
/// no sketch geometry behind it.
pub(in crate::panels::footprint_editor_properties) fn view_driven_by<'a>(
    fp: &'a FootprintEditorPanelContext,
    target: PadEditTarget,
    palette: PanelPalette,
    i18n: &I18n,
) -> Option<Element<'a, PanelMsg>> {
    let PadEditTarget::Selected(idx) = target else {
        return None;
    };
    let pad = fp.selected_pad.as_ref().filter(|pad| pad.idx == idx)?;
    let source = pad.sketch_source?;
    let row = |key: signex_i18n::MessageId, value: String| {
        let label = t!(i18n, key).into_string();
        value_row(&label, &value, palette.muted, palette.input_bg)
    };
    let size = t!(
        i18n,
        ids::EDITORS_FP_PAD_DRIVEN_SIZE_VALUE,
        "x" => format!("{:.2}", pad.size_mm[0]),
        "y" => format!("{:.2}", pad.size_mm[1])
    );
    let title = container(
        text(t!(i18n, ids::EDITORS_FP_PAD_DRIVEN_BY))
            .size(TS.caption)
            .color(palette.muted),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill);
    let section: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .push(title.boxed())
        .push(row(
            ids::EDITORS_FP_PAD_DRIVEN_SOURCE,
            source_text(source, i18n),
        ))
        .push(row(
            ids::EDITORS_FP_PAD_DRIVEN_INSTANCE,
            instance_text(source.instance, i18n),
        ))
        .push(row(ids::EDITORS_FP_PAD_DRIVEN_SIZE, size.into_string()));
    Some(section.boxed())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(s: String) -> String {
        s.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn the_source_names_a_point_or_a_profile() {
        let i18n = I18n::en_us();
        let of = |edges| {
            plain(source_text(
                PadSketchSource {
                    edges,
                    instance: None,
                },
                &i18n,
            ))
        };
        assert_eq!(of(0), "Pad point");
        assert_eq!(of(4), "Profile of 4 edges");
    }

    #[test]
    fn the_instance_names_the_pattern_and_the_place() {
        let i18n = I18n::en_us();
        assert_eq!(plain(instance_text(None, &i18n)), "Single pad");
        let linear = PadInstance {
            kind: PadPatternKind::Linear,
            index: 3,
            row: None,
        };
        assert_eq!(
            plain(instance_text(Some(linear), &i18n)),
            "Linear pattern, instance 3"
        );
        let grid = PadInstance {
            kind: PadPatternKind::Grid,
            index: 2,
            row: Some(4),
        };
        assert_eq!(
            plain(instance_text(Some(grid), &i18n)),
            "Grid pattern, instance 2 · 4"
        );
    }
}
