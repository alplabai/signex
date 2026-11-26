//! Mockup row 80: Apply is drawn on the primary fill (the dark ink in the
//! light theme, not the accent), and "Footprint preview" heads its column
//! as an uppercase section label.

use std::path::PathBuf;

use iced::{Color, Size};
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::{apply_button, preview};
use crate::app::Message;
use crate::app::assign_footprints::{
    AssignFootprintsMsg, AssignFootprintsState, FootprintOffer, FootprintPart, PartAccess,
};
use crate::layout_probe::{draw_pixels, lay_out_texts, root_size};
use crate::styles::tokens;

const WINDOW: Size = Size::new(200.0, 60.0);

fn state() -> AssignFootprintsState {
    let offer = |binding: &str| FootprintOffer {
        binding: binding.to_string(),
        label: format!("FP {binding}"),
    };
    let mut state = AssignFootprintsState::reading(1, 0);
    state.show(vec![FootprintPart {
        sheet: PathBuf::from("a.snxsch"),
        sheet_label: "a.snxsch".to_string(),
        symbol_id: Uuid::now_v7(),
        units: Vec::new(),
        designator: "R1".to_string(),
        value: "10k".to_string(),
        row_label: Some("ROW-1".to_string()),
        row_key: Some((Uuid::from_u128(1), Uuid::from_u128(1))),
        current: "FP 0805".to_string(),
        current_binding: Some("0805".to_string()),
        offers: vec![offer("0805"), offer("0603")],
        access: PartAccess::Editable,
    }]);
    state
}

fn rgb8(c: Color) -> [u8; 3] {
    let [r, g, b, _] = c.into_rgba8();
    [r, g, b]
}

fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2)
}

#[test]
fn apply_is_drawn_on_the_primary_fill_in_both_themes() {
    let i18n = I18n::en_us();
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let theme = theme_tokens(id);
        let r = tokens::resolve(&theme);
        let apply = || {
            apply_button(
                &i18n,
                Some(Message::from(AssignFootprintsMsg::ApplyRequested)),
                &theme,
            )
        };
        let size = root_size(apply(), WINDOW);
        let pixels = draw_pixels(apply(), WINDOW);
        // Inside the button's side padding, clear of its label and corners.
        let (x, y) = (
            (crate::styles::Spacing::DEFAULT.l / 2.0) as usize,
            (size.height / 2.0) as usize,
        );
        let i = (y * WINDOW.width as usize + x) * 4;
        let got = [pixels[i], pixels[i + 1], pixels[i + 2]];
        assert!(
            near(got, rgb8(r.primary)),
            "{id:?}: Apply fill {got:?}, want the primary {:?}",
            rgb8(r.primary)
        );
        assert!(
            !near(got, rgb8(tokens::accent_fill(&theme))),
            "{id:?}: Apply is still on the accent fill"
        );
    }
}

#[test]
fn footprint_preview_heads_its_column_as_an_uppercase_label() {
    let i18n = I18n::en_us();
    let theme = theme_tokens(ThemeId::GraphiteLight);
    let state = state();
    let texts: Vec<String> = lay_out_texts(
        preview::column(&state, &i18n, &theme),
        Size::new(800.0, 900.0),
    )
    .into_iter()
    .map(|t| t.text)
    .collect();
    assert!(texts.iter().any(|t| t == "FOOTPRINT PREVIEW"), "{texts:#?}");
    assert!(
        !texts.iter().any(|t| t == "Footprint preview"),
        "{texts:#?}"
    );
}
