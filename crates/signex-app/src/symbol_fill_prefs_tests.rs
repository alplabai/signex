use super::*;
use signex_types::theme::{ThemeId, canvas_colors};

fn round_trip(pref: SymbolBodyFill) -> SymbolBodyFill {
    let mut prefs = serde_json::Map::new();
    pref.write_into(&mut prefs);
    SymbolBodyFill::from_prefs(Some(&serde_json::Value::Object(prefs)))
}

#[test]
fn the_preference_round_trips_through_prefs_json() {
    let custom = SymbolBodyFill {
        enabled: true,
        color: Some(Color::new(0x12, 0x34, 0x56, 0xFF)),
    };
    let off = SymbolBodyFill {
        enabled: false,
        color: None,
    };
    for pref in [SymbolBodyFill::default(), custom, off] {
        assert_eq!(round_trip(pref), pref);
    }
}

#[test]
fn a_missing_or_broken_value_reads_as_on_in_the_theme_tint() {
    assert_eq!(SymbolBodyFill::from_prefs(None), SymbolBodyFill::default());
    let mut prefs = serde_json::Map::new();
    prefs.insert(ENABLED_KEY.to_string(), serde_json::json!("yes"));
    prefs.insert(COLOR_KEY.to_string(), serde_json::json!("#12"));
    let broken = serde_json::Value::Object(prefs);
    assert_eq!(
        SymbolBodyFill::from_prefs(Some(&broken)),
        SymbolBodyFill {
            enabled: true,
            color: None,
        }
    );
}

#[test]
fn going_back_to_the_theme_colour_removes_the_saved_colour() {
    let mut prefs = serde_json::Map::new();
    SymbolBodyFill {
        enabled: true,
        color: Some(Color::new(1, 2, 3, 255)),
    }
    .write_into(&mut prefs);
    SymbolBodyFill::default().write_into(&mut prefs);
    assert!(!prefs.contains_key(COLOR_KEY));
}

#[test]
fn the_tint_drawn_is_the_users_colour_the_themes_or_none_when_off() {
    let theme = canvas_colors(ThemeId::GraphiteLight);
    let user = Color::new(0x12, 0x34, 0x56, 0xFF);
    assert_eq!(
        SymbolBodyFill::default().apply(theme).body_tint,
        theme.body_tint
    );
    let custom = SymbolBodyFill {
        enabled: true,
        color: Some(user),
    };
    assert_eq!(custom.apply(theme).body_tint, Some(user));
    let off = SymbolBodyFill {
        enabled: false,
        ..custom
    };
    assert_eq!(off.apply(theme).body_tint, None);
}
