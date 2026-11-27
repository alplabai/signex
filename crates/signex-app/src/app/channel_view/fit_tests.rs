//! #2662, mockup row 151: a channel card shows the channel's fit in the
//! active variant, greys a channel the variant leaves out and the lines of
//! the parts it leaves out, and a click on its fit leaves the whole channel
//! out (or lets the sheets decide again) as one undo step.

use std::path::PathBuf;

use iced::Size;
use signex_types::pcb::SymbolLink;
use signex_types::project::PartRef;
use signex_types::schematic::{ChannelOccurrence, InstancePath, SchematicSheet};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::fit;
use crate::app::CompiledDesignMsg;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::layout_probe::lay_out_texts;
use crate::panels::PanelContext;
use crate::panels::compiled_design::{ChannelPartFit, ChannelStripView, ChannelTabView};

/// Channel `i`'s copy of the part `designator` on the repeated sheet.
fn copy(i: u32, designator: &str, is_fitted: bool) -> ChannelPartFit {
    let path = InstancePath::from_chain(vec![
        Uuid::from_u128(0x2662_0000),
        Uuid::from_u128(0x2662_0000 + u128::from(i)),
    ]);
    let unit = Uuid::from_u128(if designator.ends_with("R5") { 5 } else { 6 });
    ChannelPartFit {
        part: PartRef::new(
            format!("CH{i}_{designator}"),
            Some(SymbolLink::new(path, [unit])),
        ),
        is_fitted,
    }
}

/// CH1 fits R5 and C6, CH2 leaves out C6, CH3 leaves out both; `variant`
/// is active.
fn strip(variant: Option<&str>) -> ChannelStripView {
    let tab = |i: u32, r5: bool, c6: bool| ChannelTabView {
        channel: ChannelOccurrence {
            name: format!("CH{i}"),
            index: i,
            number: i,
            of: 3,
        },
        parts: Some(2),
        nets: Some(3),
        designators: vec![format!("CH{i}_C6"), format!("CH{i}_R5")],
        miniature: None,
        fits: vec![copy(i, "C6", c6), copy(i, "R5", r5)],
    };
    ChannelStripView {
        sheet: PathBuf::from("buck_channel.snxsch"),
        file: "buck_channel.snxsch".into(),
        tabs: vec![
            tab(1, true, true),
            tab(2, true, false),
            tab(3, false, false),
        ],
        active: Some(1),
        designator_map: Vec::new(),
        variant: variant.map(str::to_string),
        is_fit_locked: false,
    }
}

fn context() -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.tokens = theme_tokens(ThemeId::GraphiteLight);
    ctx.theme_id = ThemeId::GraphiteLight;
    ctx
}

fn texts(ctx: &PanelContext, view: &ChannelStripView) -> Vec<String> {
    lay_out_texts(super::cards::view(ctx, view, 1), Size::new(1200.0, 700.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn each_card_says_how_its_channel_fits_in_the_active_variant() {
    let ctx = context();
    let shown = texts(&ctx, &strip(Some("Lite")));
    for words in ["Lite: Fitted", "Lite: 1 not fitted", "Lite: Not fitted"] {
        assert!(shown.iter().any(|t| t == words), "{words} in {shown:?}");
    }
    let base = texts(&ctx, &strip(None));
    assert!(
        !base.iter().any(|t| t.starts_with("Lite")),
        "Base is the sheets: {base:?}"
    );
}

#[test]
fn a_channel_left_out_entirely_reads_as_not_fitted_and_its_parts_greyed() {
    let ctx = context();
    let view = strip(Some("Lite"));
    let [ch1, ch2, ch3] = &view.tabs[..] else {
        panic!("three channels");
    };
    assert!(!ch1.is_not_fitted() && !ch2.is_not_fitted() && ch3.is_not_fitted());
    assert_eq!(ch2.not_fitted(), 1);
    assert!(ch2.is_part_not_fitted("CH2_C6"));
    assert!(!ch2.is_part_not_fitted("CH2_R5"));
    assert_ne!(
        fit::line_ink(&ctx, ch2, "CH2_C6"),
        fit::line_ink(&ctx, ch2, "CH2_R5"),
        "a part left out is greyed"
    );
    // The name of a channel left out entirely is greyed; a channel with
    // some parts left out keeps its ink.
    assert_ne!(fit::name_ink(&ctx, ch3), fit::name_ink(&ctx, ch1));
    assert_eq!(fit::name_ink(&ctx, ch2), fit::name_ink(&ctx, ch1));
    // A click leaves a channel out; on a channel left out entirely it lets
    // the sheets decide again.
    assert_eq!(fit::channel_click(ch1), Some(false));
    assert_eq!(fit::channel_click(ch2), Some(false));
    assert_eq!(fit::channel_click(ch3), None);
}

#[test]
fn a_cards_click_leaves_the_channel_out_as_one_undo_step() {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    open_with(
        &mut app.document_state,
        &dir.path().join("a.snxsch"),
        SchematicSheet::default(),
    );
    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string()];
    data.active_variant = Some("Lite".to_string());
    let id = app.document_state.projects[0].id;
    let parts: Vec<PartRef> = strip(Some("Lite")).tabs[0]
        .fits
        .iter()
        .map(|fit| fit.part.clone())
        .collect();
    let _ = app.update(crate::app::Message::from(
        CompiledDesignMsg::ChannelFitPicked {
            parts: parts.clone(),
            fitted: Some(false),
        },
    ));
    let settings = &app.document_state.projects[0].data.variant_settings;
    assert_eq!(settings.parts.len(), 2);
    for part in &parts {
        assert_eq!(
            settings.part(Some("Lite"), part.placed()).unwrap().fitted,
            Some(false)
        );
    }
    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .variant_settings
            .is_empty()
    );
}
