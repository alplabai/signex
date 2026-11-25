//! The Slot header of a selected slot edge (mockup row 179, DECISIONS
//! 2026-11-24): it heads the edge's own Entity section, and a non-slot edge
//! keeps its entity header.

use std::path::PathBuf;

use iced::Widget as _;
use iced::widget::Column;
use signex_library::{Footprint, FootprintFile};
use signex_types::sketch::SketchHost;

use super::super::body::Ctx;
use super::view_sketch_entity;
use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
use crate::layout_probe::{LaidOutText, lay_out_texts};
use crate::panels::{
    BoardSketchProps, FootprintEditorPanelContext, FootprintModeKind, FootprintSketchEntitySummary,
    PanelPalette,
};

const PATH: &str = "slot-header.snxfpt";
const DOCK: iced::Size = iced::Size::new(300.0, 4000.0);

fn app() -> Signex {
    let path = PathBuf::from(PATH);
    let file = FootprintFile::from_footprint(Footprint::empty("board"));
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "board".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = 0;
    app.refresh_panel_ctx();
    app
}

/// A board sketch with a line selected; `slot` is its slot's size.
fn line_selected(app: &Signex, slot: Option<(f64, f64)>) -> FootprintEditorPanelContext {
    let mut fp = app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_deref()
        .cloned()
        .expect("the footprint editor panel context is built");
    fp.mode_kind = FootprintModeKind::Sketch;
    fp.sketch_host = SketchHost::Board;
    fp.selected_sketch_entity = Some(FootprintSketchEntitySummary {
        kind_label: "Line",
        position_mm: None,
        attached_constraint_count: 2,
        construction: false,
        dof_state: None,
    });
    let id = signex_sketch::id::SketchEntityId::new();
    fp.selected_sketch_entity_id = Some(id);
    fp.board = Some(BoardSketchProps {
        selected: Some(id),
        is_edge: true,
        selected_slot: slot,
        ..BoardSketchProps::default()
    });
    fp
}

fn texts(app: &Signex, fp: &FootprintEditorPanelContext) -> Vec<LaidOutText> {
    let ctx = &app.document_state.panel_ctx;
    let i18n = signex_i18n::I18n::en_us();
    let cx = Ctx {
        fp,
        palette: PanelPalette::from_tokens(&ctx.tokens),
        tokens: &ctx.tokens,
        collapsed: &ctx.collapsed_sections,
        unit: signex_types::coord::Unit::Mm,
        i18n: &i18n,
    };
    let entity = fp.selected_sketch_entity.as_ref().expect("a line");
    let col = view_sketch_entity(Column::new(), cx, entity);
    lay_out_texts(col.boxed(), DOCK)
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

fn has(texts: &[LaidOutText], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

#[test]
fn a_slot_edge_heads_with_the_slot_and_its_size() {
    let app = app();
    let fp = line_selected(&app, Some((12.0, 3.0)));
    let texts = texts(&app, &fp);
    let first: Vec<&str> = texts.iter().take(2).map(|t| t.text.as_str()).collect();
    assert_eq!(first, ["Slot", "12.00 × 3.00 mm"], "{texts:#?}");
    assert!(!has(&texts, "Sketch entity · Line"), "{texts:#?}");
    // The selection is still the edge: its own Entity section follows.
    assert!(has(&texts, "ENTITY"), "{texts:#?}");
}

#[test]
fn a_non_slot_edge_has_no_slot_header() {
    let app = app();
    let fp = line_selected(&app, None);
    let texts = texts(&app, &fp);
    assert_eq!(
        texts.first().map(|t| t.text.as_str()),
        Some("Sketch entity · Line"),
        "{texts:#?}"
    );
    assert!(!has(&texts, "Slot"), "{texts:#?}");
}

fn position(texts: &[LaidOutText], label: &str) -> usize {
    texts
        .iter()
        .position(|t| t.text == label)
        .unwrap_or_else(|| panic!("{label} is shown: {texts:#?}"))
}

/// Mockup row 179 (m179-slot): on a board sketch the Sketch DOF section
/// sits right under the selection's header, above its Entity section.
#[test]
fn a_board_selection_has_the_sketch_dof_section_under_its_header() {
    let app = app();
    for slot in [Some((12.0, 3.0)), None] {
        let fp = line_selected(&app, slot);
        let texts = texts(&app, &fp);
        let header = match slot {
            Some(_) => position(&texts, "Slot"),
            None => position(&texts, "Sketch entity · Line"),
        };
        let dof = position(&texts, "SKETCH DOF");
        assert!(header < dof, "{texts:#?}");
        assert!(dof < position(&texts, "ENTITY"), "{texts:#?}");
    }
}
