//! The Graphite bar glyphs draw the schematic metaphor of their tool,
//! checked against the mock-up's Active Bar paths.

use super::glyphs::PowerGlyph;
use super::{ActiveBarAction, ActiveBarMenu, ActiveBarMsg, bar_items};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar::{ActiveBarIcon, ActiveBarItem};

const POWER: &str = include_str!("../../assets/icons/active_bar/graphite/power.svg");
const POWER_RAIL: &str = include_str!("../../assets/icons/active_bar/graphite/power_rail.svg");
const WIRE: &str = include_str!("../../assets/icons/active_bar/graphite/wire.svg");

/// An axis-aligned stroke of a glyph: `(x0, y0, x1, y1)`.
type Seg = (f32, f32, f32, f32);

/// The axis-aligned strokes of every `d="…"` in `svg`. Understands the
/// `M h H v V` commands the bar glyphs that matter here are drawn with.
fn strokes(svg: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    for d in svg.split("d=\"").skip(1) {
        let d = d.split('"').next().unwrap_or("");
        let mut tokens: Vec<String> = Vec::new();
        let mut cur = String::new();
        for ch in d.chars() {
            if ch.is_ascii_alphabetic() {
                if !cur.trim().is_empty() {
                    tokens.push(cur.trim().to_string());
                }
                tokens.push(ch.to_string());
                cur.clear();
            } else if ch == ' ' || ch == ',' {
                if !cur.trim().is_empty() {
                    tokens.push(cur.trim().to_string());
                }
                cur.clear();
            } else {
                cur.push(ch);
            }
        }
        if !cur.trim().is_empty() {
            tokens.push(cur.trim().to_string());
        }
        let num = |s: &String| s.parse::<f32>().expect("numeric path argument");
        let (mut x, mut y) = (0.0_f32, 0.0_f32);
        let mut i = 0;
        while i < tokens.len() {
            let cmd = tokens[i].as_str();
            match cmd {
                "M" => {
                    x = num(&tokens[i + 1]);
                    y = num(&tokens[i + 2]);
                    i += 3;
                }
                "h" | "H" | "v" | "V" => {
                    let a = num(&tokens[i + 1]);
                    let (nx, ny) = match cmd {
                        "h" => (x + a, y),
                        "H" => (a, y),
                        "v" => (x, y + a),
                        _ => (x, a),
                    };
                    out.push((x, y, nx, ny));
                    (x, y) = (nx, ny);
                    i += 2;
                }
                other => panic!("unexpected path command {other} in {d}"),
            }
        }
    }
    out
}

fn horizontals(segs: &[Seg]) -> Vec<(f32, f32, f32)> {
    let mut h: Vec<(f32, f32, f32)> = segs
        .iter()
        .filter(|s| s.1 == s.3 && s.0 != s.2)
        .map(|s| (s.1, s.0.min(s.2), s.0.max(s.2)))
        .collect();
    h.sort_by(|a, b| a.0.total_cmp(&b.0));
    h
}

fn verticals(segs: &[Seg]) -> Vec<(f32, f32, f32)> {
    segs.iter()
        .filter(|s| s.0 == s.2 && s.1 != s.3)
        .map(|s| (s.0, s.1.min(s.3), s.1.max(s.3)))
        .collect()
}

/// The maintainer saw the IEC standby symbol on the power button. The
/// schematic GND glyph is a stem on top and three centred bars that
/// shrink downwards (mock path `M8 2V8M3 8h10M5 11h6M7 14h2`).
#[test]
fn power_glyph_is_ground_with_the_stem_on_top_and_three_shrinking_bars() {
    // `strokes` rejects the standby symbol's arc command outright.
    let segs = strokes(POWER);
    let bars = horizontals(&segs);
    let stems = verticals(&segs);
    assert_eq!(bars.len(), 3, "three ground bars: {bars:?}");
    assert_eq!(stems.len(), 1, "one stem: {stems:?}");
    let (sx, s_top, s_bottom) = stems[0];
    assert!(s_bottom <= bars[0].0, "the stem sits above every bar");
    assert!(s_top < bars[0].0);
    for pair in bars.windows(2) {
        let (w0, w1) = (pair[0].2 - pair[0].1, pair[1].2 - pair[1].1);
        assert!(
            pair[0].0 < pair[1].0 && w1 < w0,
            "bars shrink downwards: {bars:?}"
        );
    }
    for (_, l, r) in &bars {
        assert_eq!((l + r) / 2.0, sx, "bars centred on the stem");
    }
}

/// The rail variant (VCC and the +/- rails) is the ground glyph flipped:
/// one bar on top of the stem.
#[test]
fn power_rail_glyph_is_a_bar_over_a_stem() {
    let segs = strokes(POWER_RAIL);
    let bars = horizontals(&segs);
    let stems = verticals(&segs);
    assert_eq!(bars.len(), 1, "{bars:?}");
    assert_eq!(stems.len(), 1, "{stems:?}");
    assert_eq!(stems[0].1, bars[0].0, "the stem hangs from the bar");
    assert_eq!((bars[0].1 + bars[0].2) / 2.0, stems[0].0);
}

/// The wire glyph was a square pulse (two verticals, back to the start
/// height): a waveform, not a wire. The mock draws one routed corner
/// pair between two different heights (`M2 12h5V4h7`).
#[test]
fn wire_glyph_is_one_routed_jog_not_a_pulse() {
    let segs = strokes(WIRE);
    assert_eq!(verticals(&segs).len(), 1, "{segs:?}");
    let (first, last) = (segs.first().unwrap(), segs.last().unwrap());
    assert_ne!(first.1, last.3, "the wire ends at another height");
}

#[test]
fn power_glyph_follows_the_port_family() {
    use ActiveBarAction::*;
    assert_eq!(PowerGlyph::of(None), PowerGlyph::Ground);
    for a in [PlacePowerGND, PlacePowerSignalGND, PlacePowerEarth] {
        assert_eq!(PowerGlyph::of(Some(&a)), PowerGlyph::Ground, "{a:?}");
    }
    for a in [
        PlacePowerVCC,
        PlacePowerPlus12,
        PlacePowerPlus5,
        PlacePowerMinus5,
        PlacePowerArrow,
        PlacePowerWave,
        PlacePowerBar,
        PlacePowerCircle,
    ] {
        assert_eq!(PowerGlyph::of(Some(&a)), PowerGlyph::Rail, "{a:?}");
    }
}

/// The handle the Graphite bar draws on its power button after `last`.
fn graphite_power_icon(last: Option<ActiveBarAction>) -> iced::widget::svg::Handle {
    let tid = ThemeId::GraphiteLight;
    let tokens = signex_types::theme::theme_tokens(tid);
    let mut last_tool = std::collections::HashMap::new();
    if let Some(a) = last {
        last_tool.insert("power".to_string(), a);
    }
    let items = bar_items(
        crate::app::Tool::Select,
        crate::app::DrawMode::Ortho90,
        &last_tool,
        &tokens,
        tid,
        false,
        false,
    );
    items
        .into_iter()
        .find_map(|item| match item {
            ActiveBarItem::Button(b)
                if matches!(
                    b.on_right_press,
                    Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power))
                ) =>
            {
                match b.icon {
                    ActiveBarIcon::Svg(h) => Some(h),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect("the bar has a power button")
}

#[test]
fn graphite_power_button_shows_the_last_used_port_family() {
    let tid = ThemeId::GraphiteLight;
    assert_eq!(
        graphite_power_icon(None).id(),
        crate::icons::icon_power(tid).id()
    );
    assert_eq!(
        graphite_power_icon(Some(ActiveBarAction::PlacePowerVCC)).id(),
        crate::icons::icon_power_rail(tid).id()
    );
    assert_eq!(
        graphite_power_icon(Some(ActiveBarAction::PlacePowerEarth)).id(),
        crate::icons::icon_power(tid).id()
    );
}
