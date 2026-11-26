//! The canvas marks of a part the active variant changes (mockup rows 208
//! and 209): a small `alt` tag by the designator of a part with an
//! alternate, and a `Δ` by the value of a part with a value of its own
//! (the value itself is drawn by `runtime::variant_values`). Hovering a mark
//! names the variant, what it sets and the Base value. A layer of plain
//! widgets over the canvas, placed through the live camera; Base marks
//! nothing.

use std::collections::HashSet;

use iced::widget::{Column, Space, Stack, column, container};
use iced::{Element, Length, Padding, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb::SymbolLink;
use signex_types::project::{
    MPN_PARAMETER, PlacedPart, VALUE_PARAMETER, VariantSettings, is_base_variant,
};
use signex_types::schematic::{InstancePath, SchematicSheet};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;
use uuid::Uuid;

use crate::app::{Message, Signex};
use crate::styles::{PillTone, Spacing, TypeScale, pill, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of a mark, in px.
const MARK_H: f32 = 14.0;

/// What a mark says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkKind {
    /// The part uses the alternate numbered `mpn`; `base_mpn` is its own.
    Alternate { mpn: String, base_mpn: String },
    /// The part's value in the variant, and its Base value.
    Value { value: String, base: String },
}

/// One mark on the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantMark {
    pub symbol: Uuid,
    /// Where it is drawn, in sheet mm: by the designator or the value.
    pub at: (f64, f64),
    pub kind: MarkKind,
}

/// The marks of `sheet` shown at `placement` in `variant`. A part is marked
/// once, on its first unit; an alternate outranks a value of its own.
pub fn variant_marks(
    sheet: &SchematicSheet,
    placement: &InstancePath,
    variant: &str,
    settings: &VariantSettings,
) -> Vec<VariantMark> {
    if is_base_variant(variant) || settings.parts.is_empty() {
        return Vec::new();
    }
    let mut marked: HashSet<&str> = HashSet::new();
    let mut marks = Vec::new();
    for symbol in sheet
        .symbols
        .iter()
        .filter(|symbol| signex_output::bom::is_part_symbol(symbol))
    {
        let designator = symbol.designator_in(placement).trim();
        if designator.is_empty() || marked.contains(designator) {
            continue;
        }
        let link = SymbolLink::new(placement.clone(), [symbol.uuid]);
        let Some(own) = settings.part(Some(variant), PlacedPart::new(designator, Some(&link)))
        else {
            continue;
        };
        let text_at = |text: &Option<signex_types::schematic::TextProp>| {
            text.as_ref()
                .map_or((symbol.position.x, symbol.position.y), |t| {
                    (t.position.x, t.position.y)
                })
        };
        let field = |name: &str| {
            symbol
                .fields
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.trim().to_string())
                .unwrap_or_default()
        };
        let mark = if let Some(mpn) = own.parameters.get(MPN_PARAMETER) {
            VariantMark {
                symbol: symbol.uuid,
                at: text_at(&symbol.ref_text),
                kind: MarkKind::Alternate {
                    mpn: mpn.clone(),
                    base_mpn: field(MPN_PARAMETER),
                },
            }
        } else if let Some(value) = own.parameters.get(VALUE_PARAMETER) {
            VariantMark {
                symbol: symbol.uuid,
                at: text_at(&symbol.val_text),
                kind: MarkKind::Value {
                    value: value.clone(),
                    base: symbol.value.clone(),
                },
            }
        } else {
            continue;
        };
        marked.insert(designator);
        marks.push(mark);
    }
    marks
}

impl Signex {
    /// The marks layer over the schematic canvas of `window_id`; an empty
    /// layer when the sheet is in no project, Base is active or no part is
    /// marked.
    pub(crate) fn variant_marks_layer(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let empty = || Space::new().boxed();
        let Some(path) = self
            .document_state
            .sheet_path_for_window(window_id, &self.ui_state)
        else {
            return empty();
        };
        let (Some(project), Some(engine)) = (
            self.document_state.document_project(path),
            self.document_state.engines.get(path),
        ) else {
            return empty();
        };
        let variant = project.data.active_variant_name();
        let placement = self
            .shown_placement(path)
            .unwrap_or_else(InstancePath::root);
        let marks = variant_marks(
            engine.document(),
            &placement,
            variant,
            &project.data.variant_settings,
        );
        if marks.is_empty() {
            return empty();
        }
        let camera = self
            .interaction_state
            .canvas_for_window(window_id)
            .live_camera();
        marks_layer(
            &marks,
            camera,
            variant,
            &self.document_state.panel_ctx.tokens,
            &self.ui_state.i18n,
        )
    }
}

/// `marks` placed through `camera` (`(offset_x, offset_y, scale)`); a mark
/// off the top or left edge is not drawn.
pub(super) fn marks_layer<'a>(
    marks: &[VariantMark],
    camera: (f32, f32, f32),
    variant: &str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let (offset_x, offset_y, scale) = camera;
    let mut layer: Stack<Element<'a, Message>> =
        Stack::new().width(Length::Fill).height(Length::Fill);
    for mark in marks {
        let x = mark.at.0 as f32 * scale + offset_x + SP.xs;
        let y = mark.at.1 as f32 * scale + offset_y - MARK_H - SP.xs;
        if !(x.is_finite() && y.is_finite()) || x < 0.0 || y < 0.0 {
            continue;
        }
        let placed = container(mark_view(mark, variant, tokens, i18n))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: y,
                left: x,
                ..Padding::ZERO
            });
        layer = layer.push(placed.boxed());
    }
    layer.boxed()
}

/// One mark with its hover card.
fn mark_view<'a>(
    mark: &VariantMark,
    variant: &str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let (face, tone, lines): (_, _, Vec<iced::widget::Text<'a, iced::Theme>>) = match &mark.kind {
        MarkKind::Alternate { mpn, base_mpn } => (
            t!(i18n, ids::VARIANT_MARK_ALT),
            PillTone::Info,
            vec![
                text(t!(i18n, ids::VARIANT_MARK_ALT_HEAD, "variant" => variant)),
                text(UserText::new(mpn.clone())),
                text(t!(i18n, ids::VARIANT_MARK_BASE, "value" => base_mpn.as_str())),
            ],
        ),
        MarkKind::Value { value, base } => (
            t!(i18n, ids::VARIANT_MARK_VALUE),
            PillTone::Accent,
            vec![
                text(t!(
                    i18n,
                    ids::VARIANT_MARK_VALUE_HEAD,
                    "variant" => variant,
                    "value" => value.as_str()
                )),
                text(t!(i18n, ids::VARIANT_MARK_BASE, "value" => base.as_str())),
                text(t!(i18n, ids::VARIANT_MARK_VALUE_HINT, "variant" => variant)),
            ],
        ),
    };
    let tag = container(
        text(face)
            .size(TS.caption)
            .font(crate::styles::medium_font()),
    )
    .height(Length::Fixed(MARK_H))
    .padding([0.0, SP.xs])
    .center_y(Length::Fixed(MARK_H))
    .style(pill(tokens, tone));
    let ink = ti(tokens.text);
    let card: Column<Element<'a, Message>> = column(
        lines
            .into_iter()
            .map(|line| line.size(TS.caption).color(ink).boxed()),
    )
    .spacing(SP.xs / 2.0);
    let tip = container(card)
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens));
    signex_widgets::tooltip(tag, tip, iced::widget::tooltip::Position::Bottom)
        .gap(SP.xs)
        .boxed()
}
