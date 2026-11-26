//! #2658, mockup row 154: a tree row of the Compiled Design panel opens its
//! sheet on a click and its source sheet on a Ctrl+click.

use iced::{Event, Size, keyboard, mouse};

use super::{CompiledPanelView, CompiledRow, CompiledRowKind, CompiledStatus};
use crate::app::CompiledDesignMsg;
use crate::layout_probe::{lay_out_texts, messages_for_events};
use crate::panels::{PanelContext, PanelMsg};

const WINDOW: Size = Size::new(320.0, 400.0);
/// The row index the CH2 row carries.
const CH2: usize = 4;

/// A compiled panel with one channel row, CH2.
fn context() -> PanelContext {
    let (app, _task) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.compiled_design.panel = CompiledPanelView {
        status: CompiledStatus::Ready,
        rows: vec![CompiledRow {
            index: CH2,
            depth: 1,
            name: "CH2".to_string(),
            caption: None,
            kind: CompiledRowKind::Channel,
            parts: 9,
            nets: 5,
            is_stale: false,
            not_fitted_in: None,
            is_selected: false,
            has_children: false,
            is_expanded: false,
            part_rows: Vec::new(),
            net_rows: Vec::new(),
        }],
        ..CompiledPanelView::default()
    };
    ctx
}

/// The row messages a click on CH2 sends with `held` down.
fn click_ch2(held: keyboard::Modifiers) -> Vec<CompiledDesignMsg> {
    let ctx = context();
    // A user name is laid out between isolation marks.
    let at = lay_out_texts(super::view(&ctx), WINDOW)
        .into_iter()
        .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "CH2")
        .expect("the CH2 row")
        .bounds
        .center();
    let events = [
        Event::Keyboard(keyboard::Event::ModifiersChanged(held)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ];
    messages_for_events(super::view(&ctx), WINDOW, at, &events)
        .into_iter()
        .filter_map(|m| match m {
            PanelMsg::CompiledDesign(m) => Some(m),
            _ => None,
        })
        .collect()
}

#[test]
fn a_click_on_a_row_opens_it() {
    let sent = click_ch2(keyboard::Modifiers::empty());
    assert!(
        matches!(sent.as_slice(), [CompiledDesignMsg::RowClicked(CH2)]),
        "{sent:?}"
    );
}

#[test]
fn a_ctrl_click_on_a_row_opens_its_source_sheet() {
    let sent = click_ch2(keyboard::Modifiers::COMMAND);
    assert!(
        matches!(sent.as_slice(), [CompiledDesignMsg::RowSourceClicked(CH2)]),
        "{sent:?}"
    );
}

/// #2662, mockup row 154: a channel row the active variant leaves out
/// carries the grey "Not fitted (Lite)" tag beside its name, in Light and
/// Dark; a fitted channel carries none.
#[test]
fn a_channel_left_out_of_the_variant_carries_the_not_fitted_tag() {
    use signex_types::theme::{ThemeId, theme_tokens};

    let tag = "Not fitted (Lite)";
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let mut ctx = context();
        ctx.tokens = theme_tokens(theme);
        ctx.theme_id = theme;
        let fitted: Vec<String> = lay_out_texts(super::view(&ctx), WINDOW)
            .into_iter()
            .map(|t| t.text)
            .collect();
        let tagged = fitted.iter().any(|t| t.contains("Not fitted"));
        assert!(!tagged, "{fitted:?}");

        ctx.compiled_design.panel.rows[0].not_fitted_in = Some("Lite".to_string());
        let shown: Vec<String> = lay_out_texts(super::view(&ctx), WINDOW)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert!(shown.iter().any(|t| t == tag), "{theme:?}: {shown:?}");
    }
}
