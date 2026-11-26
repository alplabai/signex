//! The block diagram tab's view (mockup row 171): the shared document-tab
//! shell with the toolbar, the canvas and Properties side by side, and the
//! footer counting blocks and connections with the mockup's hint. The view
//! switch puts the Power tree and its rails there instead; the export
//! popover sits over the canvas.

mod export;
mod power_tree;
mod properties;
mod toolbar;

use std::path::PathBuf;

use iced::widget::{container, row, stack};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer};

use super::canvas::{CanvasLook, diagram_canvas};
use super::state::{BlockDiagramEditor, DiagramView};
use super::{BlockDiagramMsg, TabEvent};
use crate::app::{Message, Signex};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The width of Properties, in body-text ems.
const SIDE_EMS: f32 = 22.0;
/// The canvas's click slop, in body-text ems.
const SLOP_EMS: f32 = 0.4;

/// What every part of the view reads.
#[derive(Clone, Copy)]
pub(super) struct Look<'a> {
    pub tokens: &'a ThemeTokens,
    pub i18n: &'a I18n,
}

impl Look<'_> {
    pub(super) fn t(&self, id: MessageId) -> Localized {
        t!(self.i18n, id)
    }
}

/// The Variant field (mockup row 171): the project's variants, Base first,
/// and the one the Power tree reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct VariantField {
    pub names: Vec<String>,
    pub shown: String,
}

impl Signex {
    /// The Variant field of the diagram at `path`: the variant picked, else
    /// the project's active one, else Base.
    pub(crate) fn power_variant_field(&self, path: &std::path::Path) -> VariantField {
        let base = signex_types::project::BASE_VARIANT.to_string();
        let project = super::project_holding(&self.document_state, path);
        let mut names = vec![base.clone()];
        if let Some(project) = project {
            names.extend(project.data.variant_definitions.iter().cloned());
        }
        let picked = (self.document_state.block_diagrams.editors.get(path))
            .and_then(|editor| editor.power.variant.clone());
        let active = project.and_then(|p| p.data.active_variant.clone());
        let shown = picked
            .or(active)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(base);
        VariantField { names, shown }
    }

    /// The tab of the diagram at `path`.
    pub(crate) fn view_block_diagram_tab(&self, path: PathBuf) -> Element<'_, Message> {
        let look = Look {
            tokens: &self.document_state.panel_ctx.tokens,
            i18n: &self.ui_state.i18n,
        };
        let Some(editor) = self.document_state.block_diagrams.editors.get(&path) else {
            return container(signex_widgets::text(look.t(ids::BLOCK_DIAGRAM_NOT_OPEN)))
                .padding(SP.l)
                .boxed();
        };
        let variant = self.power_variant_field(&path);
        tab(look, editor, variant)
            .map(move |event| {
                Message::BlockDiagram(BlockDiagramMsg::Tab {
                    path: path.clone(),
                    event,
                })
            })
            .boxed()
    }
}

/// The canvas colours from the theme tokens.
pub(super) fn canvas_look(look: Look<'_>, show_ground: bool) -> CanvasLook {
    let tokens = look.tokens;
    CanvasLook {
        background: ti(tokens.paper),
        block_fill: ti(tokens.panel_bg),
        edge: ti(tokens.border),
        text: ti(tokens.text),
        text_secondary: ti(tokens.text_secondary),
        accent: ti(tokens.accent),
        power: ti(tokens.error),
        unlinked: ti(tokens.warning),
        text_size: TS.body,
        slop: TS.body * SLOP_EMS,
        show_ground,
    }
}

/// The whole tab.
fn tab<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
    variant: VariantField,
) -> Element<'a, TabEvent> {
    let (main, side) = match editor.view {
        DiagramView::Blocks => (
            diagram_canvas(editor, canvas_look(look, editor.show_ground)),
            properties::view(look, editor),
        ),
        DiagramView::PowerTree => (
            power_tree::main(look, editor),
            power_tree::side(look, editor),
        ),
    };
    // The export popover sits over the canvas's top right (mockup row 171).
    let main = match &editor.export {
        Some(draft) => stack![
            main,
            container(export::view(look, editor, draft))
                .padding(SP.s)
                .width(Length::Fill)
                .align_x(Alignment::End),
        ]
        .boxed(),
        None => main,
    };
    let body = row![
        container(main)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        container(side)
            .width(Length::Fixed(TS.body * SIDE_EMS))
            .height(Length::Fill)
            .boxed(),
    ]
    .height(Length::Fill);
    DocTabShell::new(
        body.boxed(),
        DocTabShellStyle::from_tokens(look.tokens, TS.body),
    )
    .toolbar(toolbar::view(look, editor, variant))
    .footer(footer(look, editor))
    .view()
}

/// "4 blocks · 4 connections", and how the canvas is used.
fn footer<'a>(look: Look<'a>, editor: &BlockDiagramEditor) -> Footer<'a> {
    if editor.view == DiagramView::PowerTree {
        let note = look.t(ids::BLOCK_DIAGRAM_POWER_FOOTER_NOTE);
        return Footer::new(power_tree::status(look, editor)).note(note);
    }
    let diagram = &editor.diagram;
    let status = t!(
        look.i18n,
        ids::BLOCK_DIAGRAM_FOOTER,
        "blocks" => diagram.blocks.len(),
        "connections" => diagram.connections.len()
    );
    let note = if editor.is_read_only() {
        look.t(ids::BLOCK_DIAGRAM_FOOTER_READ_ONLY)
    } else {
        look.t(ids::BLOCK_DIAGRAM_FOOTER_NOTE)
    };
    Footer::new(status).note(note)
}

/// A caption over a Properties section or a toolbar group.
pub(super) fn caption<'a>(look: Look<'a>, id: MessageId) -> Element<'a, TabEvent> {
    signex_widgets::text(look.t(id))
        .size(TS.section)
        .color(resolve(look.tokens).ink3)
        .boxed()
}
