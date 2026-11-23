//! Mockup row 139, decided 2026-11-22: a measurement with a prefix-taking
//! unit has one control, and picking a prefix or a unit keeps the other and
//! the typed number.

use signex_library::{ParamKind, ParamSlot, ParamValue, ParameterTemplate};
use signex_types::quantity::SiPrefix;

use super::*;

const KEY: &str = "param_1";

fn measured(value: f64, unit: &str) -> ParamValue {
    ParamValue::Measurement {
        value,
        unit: unit.into(),
    }
}

/// A custom row holding 4.7 kΩ, with its menu open.
fn app_with_kilo_ohm_row() -> (Signex, PathBuf) {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    for msg in [
        BrowserEditMsg::AddParam,
        BrowserEditMsg::SetParamValue {
            key: KEY.into(),
            value: "4.7".into(),
        },
        BrowserEditMsg::SetParamUnit {
            key: KEY.into(),
            unit: "ohm".into(),
        },
        BrowserEditMsg::ParamPrefixPicked {
            key: KEY.into(),
            prefix: SiPrefix::Kilo,
        },
        BrowserEditMsg::UnitMenuOpened { key: KEY.into() },
    ] {
        let _ = app.update(edit(&root, msg));
    }
    assert_eq!(draft_param(&app, &root, KEY), Some(measured(4700.0, "ohm")));
    (app, root)
}

fn buffered_number(app: &Signex, root: &Path) -> Option<String> {
    let modal = app.library.library_browsers[root].edit_modal.as_ref()?;
    modal.param_buf.get(KEY).map(|(number, _)| number.clone())
}

#[test]
fn picking_a_prefix_keeps_the_unit_and_number_and_closes_the_menu() {
    let (mut app, root) = app_with_kilo_ohm_row();
    let picked = BrowserEditMsg::ParamPrefixPicked {
        key: KEY.into(),
        prefix: SiPrefix::Mega,
    };
    let _ = app.update(edit(&root, picked));
    assert_eq!(open_unit_menu(&app, &root), None);
    assert_eq!(buffered_number(&app, &root).as_deref(), Some("4.7"));
    assert_eq!(
        draft_param(&app, &root, KEY),
        Some(measured(4_700_000.0, "ohm"))
    );
}

#[test]
fn picking_a_unit_keeps_the_prefix_and_number_and_closes_the_menu() {
    let (mut app, root) = app_with_kilo_ohm_row();
    let picked = BrowserEditMsg::SetParamUnit {
        key: KEY.into(),
        unit: "V".into(),
    };
    let _ = app.update(edit(&root, picked));
    assert_eq!(open_unit_menu(&app, &root), None);
    assert_eq!(buffered_number(&app, &root).as_deref(), Some("4.7"));
    assert_eq!(draft_param(&app, &root, KEY), Some(measured(4700.0, "V")));
}

/// Esc closes the joined menu first, as it does the plain unit menu.
#[test]
fn escape_closes_the_measurement_menu_before_the_dialog() {
    let (mut app, root) = app_with_kilo_ohm_row();
    let _ = app.handle_escape_pressed(None);
    assert_eq!(open_unit_menu(&app, &root), None);
    assert!(app.library.library_browsers[&root].edit_modal.is_some());
}

/// What the Parameters table shows for the row, from the real dialog view.
fn dialog_texts(app: &Signex, root: &Path) -> Vec<String> {
    let modal = app.library.library_browsers[root]
        .edit_modal
        .as_ref()
        .expect("the dialog is open");
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let i18n = signex_i18n::I18n::en_us_ref();
    let context = crate::library::edit_row_modal::EditRowContext::resolve(
        &app.library,
        root,
        modal,
        Vec::new(),
    );
    let view = crate::library::edit_row_modal::view(root, modal, &tokens, i18n, false, context);
    crate::layout_probe::lay_out_texts(view, iced::Size::new(1200.0, 900.0))
        .into_iter()
        .map(|t| t.text)
        .collect()
}

/// A custom row used to show `kΩ` and a second `Ω` control.
#[test]
fn a_custom_row_with_a_prefix_unit_shows_one_face() {
    let (mut app, root) = app_with_kilo_ohm_row();
    let _ = app.update(edit(&root, BrowserEditMsg::UnitMenuDismissed));
    let texts = dialog_texts(&app, &root);
    let count = |wanted: &str| texts.iter().filter(|t| *t == wanted).count();
    assert_eq!(count("kΩ"), 1, "{texts:?}");
    assert_eq!(count("Ω"), 0, "{texts:?}");
}

/// A template row fixes its unit, so a unit pick on it is still refused.
#[test]
fn a_template_row_still_refuses_a_unit_pick() {
    let (mut app, root, row_id) = app_with_browser();
    let template = ParameterTemplate {
        class: "generic".into(),
        description: String::new(),
        required_params: vec![ParamSlot {
            name: "value".into(),
            kind: ParamKind::Measurement,
            unit: Some("ohm".into()),
            allow_zero: false,
            allow_negative: false,
            display_plus_minus: false,
        }],
        optional_params: Vec::new(),
    };
    let library_id = app.library.library_at(&root).expect("library").library_id;
    std::sync::Arc::make_mut(&mut app.library.template_registry)
        .insert_for_library(library_id, template);
    open_modal(&mut app, &root, row_id);
    add_slot(&mut app, &root, "value");
    let picked = BrowserEditMsg::SetParamUnit {
        key: "value".into(),
        unit: "V".into(),
    };
    let _ = app.update(edit(&root, picked));
    let modal = app.library.library_browsers[&root].edit_modal.as_ref();
    let error = modal.and_then(|m| m.error.as_deref()).unwrap_or_default();
    assert!(error.contains("fixes its unit"), "{error}");
}

/// A row with an unknown unit shows it as written.
#[test]
fn an_unknown_unit_is_shown_as_written() {
    let (mut app, root, row_id) = app_with_browser();
    open_modal(&mut app, &root, row_id);
    let _ = app.update(edit(&root, BrowserEditMsg::AddParam));
    let typed = BrowserEditMsg::SetParamUnit {
        key: KEY.into(),
        unit: "furlong".into(),
    };
    let _ = app.update(edit(&root, typed));
    let texts = dialog_texts(&app, &root);
    assert!(texts.iter().any(|t| t == "furlong"), "{texts:?}");
}

/// A unit that lacks the current prefix reads the number in its base unit.
#[test]
fn a_unit_without_the_current_prefix_keeps_the_number_in_its_base() {
    let (mut app, root) = app_with_kilo_ohm_row();
    let picked = BrowserEditMsg::SetParamUnit {
        key: KEY.into(),
        unit: "F".into(),
    };
    let _ = app.update(edit(&root, picked));
    assert_eq!(buffered_number(&app, &root).as_deref(), Some("4.7"));
    assert_eq!(draft_param(&app, &root, KEY), Some(measured(4.7, "F")));

    let picked = BrowserEditMsg::SetParamUnit {
        key: KEY.into(),
        unit: "V".into(),
    };
    let _ = app.update(edit(&root, picked));
    assert_eq!(draft_param(&app, &root, KEY), Some(measured(4.7, "V")));
}

/// Going through "no unit" does not bring the old prefix back either.
#[test]
fn a_unit_picked_after_no_unit_keeps_the_number_it_shows() {
    let (mut app, root) = app_with_kilo_ohm_row();
    for unit in ["", "V"] {
        let picked = BrowserEditMsg::SetParamUnit {
            key: KEY.into(),
            unit: unit.into(),
        };
        let _ = app.update(edit(&root, picked));
    }
    assert_eq!(buffered_number(&app, &root).as_deref(), Some("4.7"));
    assert_eq!(draft_param(&app, &root, KEY), Some(measured(4.7, "V")));
}
