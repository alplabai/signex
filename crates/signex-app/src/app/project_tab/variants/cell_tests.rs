//! Mockup rows 208 and 209 in the Variant Manager: a cell whose part uses
//! an alternate names its part number on the info tint, and a cell with a
//! value of its own shows it with "value"; hovering either says what the
//! variant sets. Graphite Light and Dark.

use iced::{Point, Size};
use signex_types::project::{PartRef, VALUE_PARAMETER};
use signex_types::theme::{ThemeId, theme_tokens};

use super::manager_tests::{app, open_manager};
use crate::app::project_tab::{alternate_parameters, variant_parameters_command};
use crate::layout_probe::{lay_out_texts, probe_points};

const WINDOW: Size = Size::new(1600.0, 900.0);

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn the_alternate_and_the_value_cells_name_what_lite_sets() {
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let (mut app, id, _dir) = app();
        open_manager(&mut app, id);
        app.document_view.panel_ctx.tokens = theme_tokens(theme);
        let r1 = [PartRef::new("R1", None)];
        let alternate = alternate_parameters(Some("RC0402FR-071KL".into()), None, None);
        let command = variant_parameters_command("Lite", &r1, alternate).expect("one part");
        assert_eq!(app.apply_project_section(id, &command), Ok(true));

        let texts = lay_out_texts(app.view_variant_manager_tab(id), WINDOW);
        let alt = texts
            .iter()
            .find(|t| t.text == "RC0402FR-071KL")
            .unwrap_or_else(|| panic!("{theme:?}: the alternate's number in {texts:?}"));
        let at = Point::new(alt.bounds.center_x(), alt.bounds.center_y());
        let probes = probe_points(app.view_variant_manager_tab(id), WINDOW, &[at]);
        let tips: Vec<String> = probes[0].tips.iter().map(|tip| plain(tip)).collect();
        assert!(
            tips.contains(&"Alternate part in Lite: RC0402FR-071KL".to_string()),
            "{theme:?}: {tips:?}"
        );
    }
}

/// A fitted part with a value of its own shows the value and "value", and
/// says so on hover.
#[test]
fn a_value_cell_shows_the_value_and_its_tooltip() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let r1 = [PartRef::new("R1", None)];
    let value = vec![(VALUE_PARAMETER.to_string(), Some("4k7".to_string()))];
    let command = variant_parameters_command("Lite", &r1, value).expect("one part");
    assert_eq!(app.apply_project_section(id, &command), Ok(true));
    let texts = lay_out_texts(app.view_variant_manager_tab(id), WINDOW);
    let shown = texts
        .iter()
        .find(|t| t.text == "4k7")
        .unwrap_or_else(|| panic!("the value in {texts:?}"));
    assert!(texts.iter().any(|t| t.text == "value"));
    let at = Point::new(shown.bounds.center_x(), shown.bounds.center_y());
    let probes = probe_points(app.view_variant_manager_tab(id), WINDOW, &[at]);
    let tips: Vec<String> = probes[0].tips.iter().map(|tip| plain(tip)).collect();
    assert!(
        tips.contains(&"Value override in Lite".to_string()),
        "{tips:?}"
    );
}
