//! What drives a baked pad, in its Properties. A patterned pad shows the
//! "Pad attributes ↔ sketch" section (mockup row 192, item D1): where it comes from in
//! the sketch, then each attribute with its scope, shared by the pattern's
//! source or overridden on this one instance. A pad no pattern places shows
//! nothing here; its header carries "Edit in Sketch ▸" (mockup row 125).

use iced::widget::{Column, Row, container};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use super::super::super::{FootprintEditorPanelContext, PanelMsg, PanelPalette};
use super::form::PadEditTarget;
use crate::library::editor::footprint::{
    PadAttribute, PadInstance, PadPatternKind, PadSketchSource,
};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width shares of the attribute and scope columns.
const ATTRIBUTE_PORTION: u16 = 4;
const SCOPE_PORTION: u16 = 5;

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

/// The scope of one attribute of a pattern instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panels::footprint_editor_properties) enum Scope {
    /// Shared by every instance through the pattern's source pad.
    Source,
    /// The designator, which the pattern's numbering gives each instance.
    Numbering,
    /// Overridden on this instance alone.
    ThisPad,
}

/// The table's rows: each attribute and its scope on `instance`.
pub(in crate::panels::footprint_editor_properties) fn scope_rows(
    instance: PadInstance,
) -> Vec<(PadAttribute, Scope)> {
    PadAttribute::ALL
        .into_iter()
        .map(|attr| {
            let scope = if instance.overrides.contains(attr) {
                Scope::ThisPad
            } else if attr == PadAttribute::Designator {
                Scope::Numbering
            } else {
                Scope::Source
            };
            (attr, scope)
        })
        .collect()
}

fn attribute_label(attr: PadAttribute) -> signex_i18n::MessageId {
    match attr {
        PadAttribute::Designator => ids::EDITORS_PAD_DESIGNATOR,
        PadAttribute::Locked => ids::EDITORS_PAD_LOCKED,
        PadAttribute::ElectricalType => ids::EDITORS_PAD_ELECTRICAL_TYPE,
        PadAttribute::Template => ids::EDITORS_PAD_TEMPLATE,
        PadAttribute::Features => ids::EDITORS_FP_PAD_SECTION_FEATURES,
        PadAttribute::Testpoint => ids::EDITORS_PAD_TESTPOINT,
        PadAttribute::Paste => ids::PROPERTIES_FP_PAD_SCOPE_PASTE,
        PadAttribute::SolderMask => ids::EDITORS_FP_PAD_CALLOUT_SOLDER_MASK,
        PadAttribute::Shape => ids::EDITORS_PAD_SHAPE,
        PadAttribute::Hole => ids::EDITORS_PAD_HOLE_DETAILS,
        PadAttribute::CopperOffset => ids::EDITORS_FP_PAD_COPPER_OFFSET,
        PadAttribute::Rotation => ids::EDITORS_PAD_ROTATION,
        PadAttribute::ThermalRelief => ids::EDITORS_PAD_THERMAL_RELIEF,
    }
}

fn scope_label(scope: Scope) -> signex_i18n::MessageId {
    match scope {
        Scope::Source => ids::PROPERTIES_FP_PAD_SCOPE_SOURCE,
        Scope::Numbering => ids::PROPERTIES_FP_PAD_SCOPE_NUMBERING,
        Scope::ThisPad => ids::EDITORS_PAD_SCOPE_THIS_PAD,
    }
}

/// The section of the selected pad; `None` for the next pad, a pad with
/// no sketch geometry behind it, or one no pattern places.
pub(in crate::panels::footprint_editor_properties) fn view_driven_by<'a>(
    fp: &'a FootprintEditorPanelContext,
    target: PadEditTarget,
    palette: PanelPalette,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, PanelMsg>> {
    let PadEditTarget::Selected(idx) = target else {
        return None;
    };
    let pad = fp.selected_pad.as_ref().filter(|pad| pad.idx == idx)?;
    let inks = ScopeInks {
        label: palette.muted,
        value: palette.primary,
        // An override stands out in the accent text colour, as the header's
        // Edit in Sketch link does (mockup row 192).
        this_pad: resolve(tokens).accent_text,
    };
    scope_section(pad.sketch_source?, inks, i18n)
}

/// The text colours of the scope section.
#[derive(Debug, Clone, Copy)]
struct ScopeInks {
    label: Color,
    value: Color,
    this_pad: Color,
}

/// The "Pad attributes ↔ sketch" section of a pad `source` drives; `None`
/// unless a pattern places it.
fn scope_section<'a>(
    source: PadSketchSource,
    inks: ScopeInks,
    i18n: &I18n,
) -> Option<Element<'a, PanelMsg>> {
    let instance = source.instance?;
    let from = t!(
        i18n,
        ids::PROPERTIES_FP_PAD_SCOPE_FROM,
        "source" => source_text(source, i18n),
        "instance" => instance_text(Some(instance), i18n)
    );
    let title = t!(i18n, ids::PROPERTIES_FP_PAD_SCOPE_TITLE);
    let mut section: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .push(caption(title.into(), inks.label))
        .push(caption(from.into(), inks.value))
        .push(cells(
            t!(i18n, ids::PROPERTIES_FP_PAD_SCOPE_ATTRIBUTE).into(),
            t!(i18n, ids::PROPERTIES_FP_PAD_SCOPE_SCOPE).into(),
            inks.label,
            inks.label,
        ));
    for (attr, scope) in scope_rows(instance) {
        let ink = if scope == Scope::ThisPad {
            inks.this_pad
        } else {
            inks.value
        };
        section = section.push(cells(
            t!(i18n, attribute_label(attr)).into(),
            t!(i18n, scope_label(scope)).into(),
            inks.label,
            ink,
        ));
    }
    Some(section.boxed())
}

/// One caption line of the section.
fn caption<'a>(content: Label<'a>, ink: Color) -> Element<'a, PanelMsg> {
    container(text(content).size(TS.caption).color(ink))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// One table row: the attribute in `label_ink`, its scope in `scope_ink`.
fn cells<'a>(
    attribute: Label<'a>,
    scope: Label<'a>,
    label_ink: Color,
    scope_ink: Color,
) -> Element<'a, PanelMsg> {
    let cell = |content: Label<'a>, ink: Color, portion: u16| {
        text(content)
            .size(TS.caption)
            .color(ink)
            .width(Length::FillPortion(portion))
            .boxed()
    };
    let row: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .push(cell(attribute, label_ink, ATTRIBUTE_PORTION))
        .push(cell(scope, scope_ink, SCOPE_PORTION));
    container(row)
        .padding([SP.xs / 2.0, SP.m])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;
    use crate::library::editor::footprint::InstanceOverrides;

    fn plain(s: String) -> String {
        s.replace(['\u{2068}', '\u{2069}'], "")
    }

    fn linear(index: u32, overrides: InstanceOverrides) -> PadInstance {
        PadInstance {
            kind: PadPatternKind::Linear,
            index,
            row: None,
            overrides,
        }
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
            overrides: InstanceOverrides::default(),
        };
        assert_eq!(
            plain(instance_text(Some(linear), &i18n)),
            "Linear pattern, instance 3"
        );
        let grid = PadInstance {
            kind: PadPatternKind::Grid,
            index: 2,
            row: Some(4),
            overrides: InstanceOverrides::default(),
        };
        assert_eq!(
            plain(instance_text(Some(grid), &i18n)),
            "Grid pattern, instance 2 · 4"
        );
    }

    /// Mockup row 192, item D1: the designator follows the numbering, every
    /// other attribute the pattern's source, until this pad overrides it.
    #[test]
    fn each_attribute_reads_its_scope() {
        let shared = scope_rows(linear(2, InstanceOverrides::default()));
        assert_eq!(shared.len(), PadAttribute::ALL.len());
        assert_eq!(shared[0], (PadAttribute::Designator, Scope::Numbering));
        assert!(shared[1..].iter().all(|(_, scope)| *scope == Scope::Source));

        let overrides = InstanceOverrides::default()
            .with(PadAttribute::Paste, true)
            .with(PadAttribute::Designator, true);
        let held: Vec<PadAttribute> = scope_rows(linear(2, overrides))
            .into_iter()
            .filter(|(_, scope)| *scope == Scope::ThisPad)
            .map(|(attr, _)| attr)
            .collect();
        assert_eq!(held, [PadAttribute::Designator, PadAttribute::Paste]);
    }

    /// Mockup row 192, item D1: the table opens with the mockup's eight
    /// rows in its order.
    #[test]
    fn the_table_opens_with_the_mockup_rows_in_order() {
        let i18n = I18n::en_us();
        let labels: Vec<String> = PadAttribute::ALL[..8]
            .iter()
            .map(|attr| plain(t!(i18n, attribute_label(*attr)).into_string()))
            .collect();
        assert_eq!(
            labels,
            [
                "Designator",
                "Paste",
                "Locked",
                "Electrical Type",
                "Template",
                "Pad Features",
                "Testpoint",
                "Solder mask",
            ]
        );
    }

    fn laid_out(source: PadSketchSource) -> Vec<String> {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let palette = PanelPalette::from_tokens(&tokens);
        let inks = ScopeInks {
            label: palette.muted,
            value: palette.primary,
            this_pad: resolve(&tokens).accent_text,
        };
        let i18n = I18n::en_us();
        let Some(section) = scope_section(source, inks, &i18n) else {
            return Vec::new();
        };
        crate::layout_probe::lay_out_texts(section, iced::Size::new(260.0, 600.0))
            .into_iter()
            .map(|t| plain(t.text))
            .collect()
    }

    /// Mockup row 125: a pad no pattern places has no extra block under its
    /// header (the Source / Instance / Size rows are gone).
    #[test]
    fn a_single_pad_shows_no_driven_by_block() {
        let texts = laid_out(PadSketchSource {
            edges: 0,
            instance: None,
        });
        assert!(texts.is_empty(), "{texts:?}");
    }

    /// Mockup row 192, item D1: a patterned pad lists where it comes from
    /// and each attribute with its scope; an override reads "This pad".
    #[test]
    fn a_patterned_pad_shows_the_attribute_scope_table() {
        let overrides = InstanceOverrides::default().with(PadAttribute::Paste, true);
        let texts = laid_out(PadSketchSource {
            edges: 4,
            instance: Some(linear(3, overrides)),
        });
        for shown in [
            "Pad attributes ↔ sketch",
            "From sketch: Profile of 4 edges · Linear pattern, instance 3",
            "Attribute",
            "Scope",
            "Designator",
            "Numbering",
            "Electrical Type",
            "Pattern source",
            "Paste",
            "This pad",
            "Solder mask",
            "Shape",
            "Thermal relief",
        ] {
            assert!(texts.iter().any(|t| t == shown), "{shown:?}: {texts:?}");
        }
        assert!(!texts.iter().any(|t| t == "Size"), "{texts:?}");
    }
}
