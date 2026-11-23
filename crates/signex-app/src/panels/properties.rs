//! Properties panel view; the pre-placement form lives in `pre_placement`.

use super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::column;
use signex_i18n::{ids, t};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub use crate::app::core::drawing_field::DrawingFieldId;

impl DrawingFieldId {
    /// A Y coordinate on the sheet: shown and typed from the page origin.
    pub fn is_page_y(self) -> bool {
        matches!(
            self,
            Self::LineStartY
                | Self::LineEndY
                | Self::RectStartY
                | Self::CircleCenterY
                | Self::ArcCenterY
        )
    }

    /// The edit this box makes with `value`.
    pub fn edit(self, value: f64) -> crate::app::contracts::DrawingFieldEdit {
        use crate::app::contracts::DrawingFieldEdit as E;
        match self {
            Self::LineStartX => E::LineStartX(value),
            Self::LineStartY => E::LineStartY(value),
            Self::LineEndX => E::LineEndX(value),
            Self::LineEndY => E::LineEndY(value),
            Self::RectStartX => E::RectStartX(value),
            Self::RectStartY => E::RectStartY(value),
            Self::RectWidth => E::RectWidthMm(value),
            Self::RectHeight => E::RectHeightMm(value),
            Self::CircleCenterX => E::CircleCenterX(value),
            Self::CircleCenterY => E::CircleCenterY(value),
            Self::CircleRadius => E::CircleRadius(value),
            Self::ArcCenterX => E::ArcCenterX(value),
            Self::ArcCenterY => E::ArcCenterY(value),
            Self::ArcRadius => E::ArcRadius(value),
            Self::ArcStartAngle => E::ArcStartAngle(value),
            Self::ArcEndAngle => E::ArcEndAngle(value),
            Self::LineWidth
            | Self::RectBorder
            | Self::CircleBorder
            | Self::ArcWidth
            | Self::PolyBorder => E::Width(value),
        }
    }

    /// Whether `text` reads as a value this box can store.
    pub fn reads(self, text: &str) -> bool {
        signex_types::numeric::parse_finite_f64(text).is_ok_and(|v| self.edit(v).is_valid())
    }

    /// Whether the box shows `text` as invalid: not a value it can store and
    /// not the start of one (empty, a lone `-`).
    pub fn refuses(self, text: &str) -> bool {
        crate::numeric_field::text_refused(text, self.reads(text))
    }
}

// ─── Properties Panel (matched to Altium Designer) ───────────

/// Width of the property-row label column: the fixed grid column, so a label
/// such as "Designator" is never clipped by a narrow dock.
pub const PROPERTY_LABEL_WIDTH: f32 = 92.0;
/// Property-row label column: fixed, so a wide dock gives its extra width to
/// the fields (#2347).
pub const PROPERTY_LABEL_COLUMN: Length = Length::Fixed(PROPERTY_LABEL_WIDTH);
pub const PROPERTY_CONTROL_PORTION: u16 = 5;
pub const PROPERTY_ROW_PAD_X: u16 = 6;
/// Gap between a property row's label and its field.
pub const PROPERTY_ROW_GAP: f32 = 6.0;

/// The tool's form, veiled and deaf while its pause belongs to another window
/// (mockup row 157).
fn read_only_while_paused_elsewhere<'a>(
    ctx: &PanelContext,
    form: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let panel = crate::styles::ti(ctx.tokens.panel_bg);
    let is_editable = !ctx.annotation.is_tool_paused_elsewhere;
    match signex_widgets::enable_section::dependents_veil(panel, is_editable) {
        Some(veil) => signex_widgets::inert::Inert::new(form, veil).into(),
        None => form,
    }
}

/// The pre-placement form Properties shows: none once its tool has ended,
/// though the values stay for the next placement (mockup row 157).
fn shown_placement_form(ctx: &PanelContext) -> Option<&PrePlacementData> {
    ctx.pre_placement
        .as_ref()
        .filter(|_| !ctx.annotation.is_placement_form_idle)
}

pub fn view_properties<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        ..
    } = palette;

    // A Component editor tab or a Library Browser row (mockup row 127) takes
    // precedence over the schematic / pre-placement / symbol-editor
    // branches, so the panel stays on the library row while its tab is
    // active.
    if let Some(sheet) = ctx.component_sheet.as_ref() {
        return super::view_component_sheet(sheet, &ctx.tokens, &ctx.i18n);
    }
    // A channel tab of a repeated sheet shows its channel, read-only (#1279).
    if let Some(channel) = super::element_properties::channel_view_properties(ctx) {
        return channel;
    }
    if let Some(detail) = ctx.library_row_detail.as_ref() {
        return view_library_row_properties(detail, &ctx.tokens, &ctx.i18n);
    }

    // Symbol-editor tab takes precedence — when the user is editing a
    // `.snxsym` the right-dock Properties panel shows symbol/pin
    // properties driven by `panel_ctx.symbol_editor`. Matches Altium's
    // SchLib editor flow where the same Properties panel switches mode
    // based on selection. (#62 / v0.9 phase 1)
    if let Some(sym) = ctx.symbol_editor.as_ref() {
        return view_symbol_editor_properties(
            sym,
            &ctx.collapsed_sections,
            ctx.numeric_draft.as_ref(),
            border_c,
            &ctx.tokens,
            &ctx.i18n,
        );
    }

    // v0.14.2 — Footprint-editor tab. Properties panel switches body
    // based on (mode × selection): Pads-mode pad selected → pad
    // properties; Sketch-mode entity selected → sketch entity
    // properties; nothing selected → footprint summary + solve stats.
    if let Some(fp) = ctx.footprint_editor.as_ref() {
        return view_footprint_editor_properties(
            fp,
            palette,
            &ctx.tokens,
            &ctx.collapsed_sections,
            ctx.unit,
            &ctx.i18n,
        );
    }

    if !ctx.has_schematic
        && let Some(board) = ctx.pcb_board.as_ref()
    {
        return super::pcb_board_properties::view(ctx, board);
    }
    if !ctx.has_schematic {
        // Don't mislead the user into thinking nothing is loaded when
        // they've just switched to a PCB tab — distinguish "no project
        // yet" from "project open, but the active tab isn't a
        // schematic". GitHub issue #51.
        let hint = if ctx.has_pcb {
            ids::PANELS_PROPERTIES_NEEDS_SCHEMATIC
        } else {
            ids::PANELS_PROPERTIES_OPEN_PROJECT
        };
        return container(
            column![
                text(t!(ctx.i18n, ids::PANELS_KIND_PROPERTIES))
                    .size(TS.label)
                    .color(primary),
                Space::new().height(12.0),
                text(t!(ctx.i18n, hint)).size(TS.body).color(muted),
            ]
            .spacing(SP.s)
            .padding(SP.m),
        )
        .width(Length::Fill)
        .boxed();
    }

    // An armed dimension tool shows its values all through placing, paused
    // or not (mockup row 157).
    if let Some(form) = ctx.annotation.dimension_tool.as_deref() {
        let form = super::element_properties::view_dimension_tool_properties(ctx, form);
        return read_only_while_paused_elsewhere(ctx, form);
    }

    // Tab pauses placement; the form edits what the next object is placed
    // with (#2532).
    if let Some(pp) = shown_placement_form(ctx) {
        return read_only_while_paused_elsewhere(ctx, super::pre_placement::view(pp, ctx));
    }

    // #1280: the harness definitions view, while it is open.
    if ctx.harness.definitions_open {
        return super::view_harness_definitions(ctx, muted, primary);
    }

    // ── Context-aware: if something is selected, show element properties (Altium style) ──
    if ctx.selection_count == 1 && !ctx.selection_info.is_empty() {
        return view_selected_element_properties(ctx, muted, primary, border_c);
    }
    if ctx.selection_count > 1 {
        return super::multi_selection::view_multi_selection(ctx, muted, primary, border_c);
    }

    // ── Nothing selected: show Document Options (Altium default) ──
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    // ── Header: the title, then the sheet's file name (#2531) ──
    let mut header = row![
        text(t!(ctx.i18n, ids::PANELS_PROPERTIES_DOCUMENT_OPTIONS))
            .size(TS.title)
            .color(primary),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(name) = ctx
        .title_block_sheet
        .as_deref()
        .and_then(std::path::Path::file_name)
    {
        header = header.push(
            text(signex_i18n::UserText::new(name.to_string_lossy()))
                .size(TS.caption)
                .color(muted)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .boxed(),
        );
    }
    col = col.push(
        container(header)
            .padding([10.0, 12.0])
            .width(Length::Fill)
            .boxed(),
    );

    // ── General | Parameters: a segmented switch ──
    let tab = ctx.properties_tab;
    let r = crate::styles::tokens::resolve(&ctx.tokens);
    let tab_style = r.segmented(TS.body).fill_width(true);
    let tabs: [(usize, signex_widgets::Label<'a>); 2] = [
        (0, t!(ctx.i18n, ids::PANELS_PROPERTIES_TAB_GENERAL).into()),
        (
            1,
            t!(ctx.i18n, ids::PANELS_PROPERTIES_TAB_PARAMETERS).into(),
        ),
    ];
    col = col.push(
        container(signex_widgets::segmented(
            tabs,
            tab.min(1),
            tab_style,
            PanelMsg::PropertiesTab,
        ))
        .padding([SP.xs, 12.0])
        .boxed(),
    );
    // ── Tab content ──
    if tab == 0 {
        col = col.push(view_properties_general(ctx, primary).boxed());
    } else {
        col = col.push(view_properties_parameters(ctx, primary).boxed());
    }

    // ── Status: Nothing selected ──
    col = col.push(Space::new().height(8.0).boxed());
    col = col.push(
        container(
            text(t!(ctx.i18n, ids::PANELS_PROPERTIES_NOTHING_SELECTED))
                .size(TS.caption)
                .color(muted),
        )
        .padding([6.0, 12.0])
        .width(Length::Fill)
        .boxed(),
    );

    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;
