//! #2751, #2752, #2753: the Properties rows at the narrow (220 px) and the
//! default (300 px) dock: segment labels never overlap, a paired value stays
//! readable, and a read-only value is sunken.

use iced::{Background, Element, Size, Theme, Widget as _};
use signex_i18n::UserText;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::{Label, text};

use super::rows::{field_insets, labelled_row, mono, pair_row, segmented_style, unit_input};
use crate::layout_probe::{find, lay_out_inputs, lay_out_texts, natural};
use crate::panels::PanelMsg;
use crate::panels::form_look::{self, FormLook};
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;

/// Fixture text, not UI text.
fn fixture_label(text: &str) -> Label<'static> {
    UserText::new(text).into()
}

const DOCKS: [f32; 2] = [220.0, 300.0];
const FORMATS: [&str; 3] = ["Template", "Standard", "Custom"];
const VALUE: &str = "88.900";

fn format_row<'a>() -> Element<'a, PanelMsg> {
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let options = [
        (0_u8, fixture_label(FORMATS[0])),
        (1, fixture_label(FORMATS[1])),
        (2, fixture_label(FORMATS[2])),
    ];
    labelled_row(
        fixture_label("Format"),
        signex_widgets::segmented(options, 0, segmented_style(&tokens), |_| PanelMsg::Noop),
    )
}

#[test]
fn segment_labels_never_overlap_and_stay_whole_at_220_and_300_px() {
    for dock in DOCKS {
        let texts = lay_out_texts(format_row(), Size::new(dock, 400.0));
        let at = FORMATS.map(|label| find(&texts, label).bounds);
        for (i, a) in at.iter().enumerate() {
            let whole = natural::<PanelMsg>(
                text(fixture_label(FORMATS[i])).size(TS.caption).boxed(),
                FORMATS[i],
            );
            assert!(
                a.width + 1.0 >= whole.width,
                "{} is cut at {dock} px: {a:?} vs {whole:?}",
                FORMATS[i]
            );
            assert!(a.x + a.width <= dock + 0.5, "past the dock: {a:?}");
            for b in &at[i + 1..] {
                assert!(!a.intersects(b), "overlap at {dock} px: {a:?} {b:?}");
            }
        }
    }
}

fn pair_view<'a>() -> Element<'a, PanelMsg> {
    let field = |axis: &'static str| {
        unit_input(
            Some(fixture_label(axis)),
            VALUE.to_string(),
            Some(fixture_label("mm")),
            |_| PanelMsg::Noop,
            PanelMsg::Noop,
        )
    };
    pair_row(fixture_label("Position"), field("X"), field("Y"))
}

#[test]
fn a_paired_value_fits_side_by_side_at_300_px_and_stacks_at_220_px() {
    let value = natural::<PanelMsg>(
        text(fixture_label(VALUE))
            .size(TS.body)
            .font(mono())
            .boxed(),
        VALUE,
    );
    let (left, right) = field_insets(true, true);
    for dock in DOCKS {
        let inputs = lay_out_inputs(pair_view(), Size::new(dock, 400.0));
        assert_eq!(inputs.len(), 2, "{inputs:?}");
        for input in &inputs {
            let room = input.bounds.width - left - right;
            assert!(
                room >= value.width,
                "{room} px for a {} px value at {dock} px",
                value.width
            );
        }
        let (x, y) = (inputs[0].bounds, inputs[1].bounds);
        assert!(!x.intersects(&y), "{x:?} {y:?}");
        if dock == 300.0 {
            assert_eq!(x.y, y.y, "side by side at the default dock");
        } else {
            assert!(y.y >= x.y + x.height, "stacked at 220 px: {x:?} {y:?}");
        }
    }
}

/// Mockup row 43: a seven-character value keeps its last digit beside the
/// unit, with X and Y still side by side at the 300 px dock.
#[test]
fn a_seven_character_value_fits_side_by_side_at_300_px() {
    const LONG: &str = "110.940";
    let field = |axis: &'static str| {
        unit_input(
            Some(fixture_label(axis)),
            LONG.to_string(),
            Some(fixture_label("mm")),
            |_| PanelMsg::Noop,
            PanelMsg::Noop,
        )
    };
    let view = pair_row(fixture_label("Start"), field("X"), field("Y"));
    let inputs = lay_out_inputs(view, Size::new(300.0, 400.0));
    assert_eq!(inputs.len(), 2, "{inputs:?}");
    assert_eq!(inputs[0].bounds.y, inputs[1].bounds.y, "side by side");
    let value = natural::<PanelMsg>(
        text(fixture_label(LONG)).size(TS.body).font(mono()).boxed(),
        LONG,
    );
    let (left, right) = field_insets(true, true);
    for input in &inputs {
        let room = input.bounds.width - left - right;
        assert!(room >= value.width, "{room} px for {} px", value.width);
    }
}

#[test]
fn a_read_only_value_is_sunken_with_no_field_border() {
    let theme = Theme::Dark;
    let style = form_look::read_only_value(&theme);
    assert_eq!(
        style.background,
        Some(Background::Color(FormLook::of(&theme).sunken))
    );
    assert_eq!(style.border.width, 0.0);
}
