//! #2662, mockup row 32 "Per-channel variant state": a part on a sheet
//! repeated as CH1 to CH3 has one matrix row, "mixed" where its channels
//! disagree, that opens into one row per channel's copy. A copy is set on
//! its own; every channel's copy is set at once as one undo step.

use std::sync::Arc;

use iced::Size;
use signex_output::{ChannelFit, RowChannel, VariantMatrix, VariantRow};
use signex_types::pcb::SymbolLink;
use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use super::manager::{VariantCell, VariantManagerMsg};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};
use crate::layout_probe::lay_out_texts;

const SHEET_SYMBOL: u128 = 0x2662_0000;

fn unit() -> Uuid {
    Uuid::from_u128(0x2662_00ff)
}

/// The part every channel's copy of R5 shares.
fn r5() -> SymbolLink {
    SymbolLink::new(
        InstancePath::from_chain(vec![Uuid::from_u128(SHEET_SYMBOL)]),
        [unit()],
    )
}

/// Channel `n`'s copy of R5, fitted in Base and Lite as its sheet says.
fn copy(n: u128) -> VariantRow {
    let path = InstancePath::from_chain(vec![
        Uuid::from_u128(SHEET_SYMBOL),
        Uuid::from_u128(SHEET_SYMBOL + n),
    ]);
    VariantRow {
        designator: format!("CH{n}_R5"),
        link: Some(SymbolLink::new(path, [unit()])),
        value: "47k".into(),
        source_value: "47k".into(),
        footprint: "R_0603".into(),
        fitted: vec![true, true],
        parameters: Vec::new(),
        overridden: vec![false, false],
        channel: Some(RowChannel {
            name: format!("CH{n}"),
            part: r5(),
            designator: "R5".into(),
        }),
    }
}

/// A project with Lite whose Variant Manager is open, its parts read as
/// R5 on a sheet repeated as CH1 to CH3. The folder lives as long as the
/// guard.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    open_with(
        &mut app.document_state,
        &dir.path().join("a.snxsch"),
        SchematicSheet::default(),
    );
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".to_string()];
    let id = app.document_state.projects[0].id;
    let task = send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Variants,
        },
    );
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the task settles");
    let read = VariantMatrix {
        variants: vec!["Lite".into()],
        rows: (1..=3).map(copy).collect(),
    };
    let tab = app.document_view.project_tabs.get_mut(&id).expect("a tab");
    tab.variant_parts = Some(Arc::new(Ok(read)));
    (app, id, dir)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(msg)))
}

fn manage(app: &mut Signex, id: ProjectId, msg: VariantManagerMsg) {
    let _ = send(
        app,
        ProjectTabMsg::VariantManager {
            project_id: id,
            msg,
        },
    );
}

/// The read with the settings laid over it, as the views show it.
fn shown(app: &Signex, id: ProjectId) -> VariantMatrix {
    let read = app.document_view.project_tabs[&id]
        .variant_parts
        .as_deref()
        .expect("the parts were read")
        .as_ref()
        .expect("the parts were read without a problem");
    read.with_settings(&app.document_state.projects[0].data.variant_settings)
}

/// Every text the tab draws, isolation marks dropped.
fn texts(app: &Signex, id: ProjectId) -> Vec<String> {
    lay_out_texts(app.view_variant_manager_tab(id), Size::new(1600.0, 900.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn one_channels_copy_is_set_on_its_own() {
    let (mut app, id, _dir) = app();
    manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: VariantCell {
                variant: Some("Lite".into()),
                designator: "CH2_R5".into(),
            },
            fitted: Some(false),
        },
    );
    let parts = &app.document_state.projects[0].data.variant_settings.parts;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].part, copy(2).link, "keyed by the channel's copy");
    assert_eq!(
        shown(&app, id).channel_fit(&r5(), 1),
        ChannelFit::Mixed {
            not_fitted: 1,
            of: 3
        }
    );
}

#[test]
fn every_channels_copy_is_set_at_once_as_one_undo_step() {
    let (mut app, id, _dir) = app();
    manage(
        &mut app,
        id,
        VariantManagerMsg::ChannelPartFitPicked {
            variant: "Lite".into(),
            part: r5(),
            fitted: Some(false),
        },
    );
    let settings = &app.document_state.projects[0].data.variant_settings;
    assert_eq!(settings.parts.len(), 3);
    assert_eq!(shown(&app, id).channel_fit(&r5(), 1), ChannelFit::NotFitted);
    assert_eq!(shown(&app, id).channel_fit(&r5(), 0), ChannelFit::Fitted);

    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .variant_settings
            .is_empty(),
        "one step takes every channel's copy back"
    );
}

#[test]
fn the_part_row_reads_mixed_and_opens_into_its_copies() {
    let (mut app, id, _dir) = app();
    manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: VariantCell {
                variant: Some("Lite".into()),
                designator: "CH2_R5".into(),
            },
            fitted: Some(false),
        },
    );
    let folded = texts(&app, id);
    assert!(folded.iter().any(|t| t == "R5"), "{folded:?}");
    assert!(folded.iter().any(|t| t == "3 channels"), "{folded:?}");
    assert!(
        folded.iter().any(|t| t == "mixed · 1 of 3 not fitted"),
        "{folded:?}"
    );
    assert!(!folded.iter().any(|t| t == "CH1_R5"), "closed: {folded:?}");

    manage(
        &mut app,
        id,
        VariantManagerMsg::ChannelPartToggled { part: r5() },
    );
    let open = texts(&app, id);
    for copy in ["CH1_R5", "CH2_R5", "CH3_R5"] {
        assert!(open.iter().any(|t| t == copy), "{copy} in {open:?}");
    }

    manage(
        &mut app,
        id,
        VariantManagerMsg::ChannelPartToggled { part: r5() },
    );
    assert!(!texts(&app, id).iter().any(|t| t == "CH1_R5"));
}
