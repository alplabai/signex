//! #2662, mockup row 151: a channel tab's Properties have a Variant section
//! with the active variant, the channels it leaves out, how this channel
//! fits and the parts the channels fit differently ("mixed · 1 of 3").

use std::path::PathBuf;

use iced::Size;
use signex_types::pcb::SymbolLink;
use signex_types::project::PartRef;
use signex_types::schematic::{ChannelOccurrence, InstancePath};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use crate::layout_probe::lay_out_texts;
use crate::panels::PanelContext;
use crate::panels::compiled_design::{
    ChannelPartFit, ChannelStripView, ChannelTabView, ChannelVariantSummary, MixedPart,
};

/// Channel `i`'s copy of the part `designator`, fitted or not.
fn copy(i: u32, designator: &str, is_fitted: bool) -> ChannelPartFit {
    let path = InstancePath::from_chain(vec![
        Uuid::from_u128(0x2662_0000),
        Uuid::from_u128(0x2662_0000 + u128::from(i)),
    ]);
    let unit = Uuid::from_u128(if designator == "R5" { 5 } else { 6 });
    ChannelPartFit {
        part: PartRef::new(
            format!("CH{i}_{designator}"),
            Some(SymbolLink::new(path, [unit])),
        ),
        is_fitted,
    }
}

/// CH1 fits R5 and C6, CH2 leaves out C6, CH3 leaves out both; CH2 is
/// shown and `variant` is active.
fn strip(variant: Option<&str>, active: Option<u32>) -> ChannelStripView {
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
        active,
        designator_map: Vec::new(),
        variant: variant.map(str::to_string),
        is_fit_locked: false,
    }
}

fn context(theme: ThemeId, strip: ChannelStripView) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.tokens = theme_tokens(theme);
    ctx.theme_id = theme;
    ctx.compiled_design.strip = Some(strip);
    ctx
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    let view = super::channel_view_properties(ctx).expect("a channel is shown");
    lay_out_texts(view, Size::new(360.0, 900.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_summary_names_the_channels_left_out_and_the_mixed_parts() {
    let summary = strip(Some("Lite"), Some(2))
        .variant_summary()
        .expect("CH2 is shown");
    assert_eq!(
        summary,
        ChannelVariantSummary {
            left_out: vec!["CH3".into()],
            not_fitted: 1,
            parts: 2,
            mixed: vec![
                MixedPart {
                    designator: "CH2_C6".into(),
                    not_fitted: 2,
                    of: 3,
                },
                MixedPart {
                    designator: "CH2_R5".into(),
                    not_fitted: 1,
                    of: 3,
                },
            ],
        }
    );
    let editor = strip(Some("Lite"), None).variant_summary();
    assert!(editor.is_none(), "Editor shows no channel");
}

#[test]
fn a_channel_tab_shows_its_variant_state_in_both_themes() {
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let shown = texts(&context(theme, strip(Some("Lite"), Some(2))));
        for words in [
            "VARIANT", // a section heading is drawn in capitals, as DESIGNATOR MAP
            "Active",
            "Lite",
            "In Lite, CH3 is Not fitted. Edit in the Variant Manager tab.",
            "This channel",
            "mixed · 1 of 2 not fitted",
            "CH2_R5",
            "mixed · 1 of 3 not fitted",
            "CH2_C6",
            "mixed · 2 of 3 not fitted",
        ] {
            assert!(
                shown.iter().any(|t| t == words),
                "{theme:?}: {words} in {shown:?}"
            );
        }
    }
}

#[test]
fn base_and_a_channel_before_a_compile_say_so() {
    let base = texts(&context(ThemeId::GraphiteLight, strip(None, Some(1))));
    assert!(base.iter().any(|t| t == "Base"), "{base:?}");
    let says_base = base.iter().any(|t| t.starts_with("Base fits every"));
    assert!(says_base, "{base:?}");
    assert!(!base.iter().any(|t| t.starts_with("mixed")), "{base:?}");

    let mut before = strip(Some("Lite"), Some(1));
    for tab in &mut before.tabs {
        tab.fits.clear();
    }
    let shown = texts(&context(ThemeId::GraphiteLight, before));
    assert!(
        shown
            .iter()
            .any(|t| t == "Compile the project to see how each channel fits in Lite."),
        "{shown:?}"
    );
}
