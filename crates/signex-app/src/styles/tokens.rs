//! Theme-derived colours for chrome surfaces.
//!
//! These replace hard-coded literals that only read on dark themes
//! (`Color::from_rgba(1, 1, 1, 0.04)` and friends). Every colour is
//! derived from existing [`ThemeTokens`] — no new token fields, no
//! theme-file change — so custom themes get the right behaviour for free.
//!
//! The key trick is that translucent "state" fills are tinted with
//! `tokens.text`: text is light on dark themes and dark on light themes,
//! so the same fill lightens a dark panel and darkens a light one.

use iced::Color;
use signex_types::theme::ThemeTokens;

use super::ti;

/// Strength of a translucent state fill, as a step of alpha over
/// `tokens.text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Resting control fill (replaces white at 0.03 / 0.04).
    Faint,
    /// Pointer-over fill (replaces white at 0.06).
    Hover,
    /// Pressed / active fill (replaces white at 0.10).
    Pressed,
}

impl Overlay {
    /// Alpha applied to `tokens.text` for this step.
    pub const fn alpha(self) -> f32 {
        match self {
            Overlay::Faint => 0.04,
            Overlay::Hover => 0.06,
            Overlay::Pressed => 0.10,
        }
    }
}

/// Translucent state fill for `step`: `tokens.text` at the step's alpha.
/// Draw it over a surface; it lightens dark themes and darkens light ones.
pub fn overlay(tokens: &ThemeTokens, step: Overlay) -> Color {
    Color {
        a: step.alpha(),
        ..ti(tokens.text)
    }
}

/// Pointer-over fill — shorthand for `overlay(tokens, Overlay::Hover)`.
pub fn hover_overlay(tokens: &ThemeTokens) -> Color {
    overlay(tokens, Overlay::Hover)
}

/// Translucent selected-row / selection tint: the accent at 40 % alpha
/// (replaces `rgba(0.4, 0.6, 1.0, 0.4)`).
pub fn selection_tint(tokens: &ThemeTokens) -> Color {
    Color {
        a: 0.4,
        ..ti(tokens.accent)
    }
}

/// Opaque accent colour (replaces `rgb(0.00, 0.47, 0.84)`).
pub fn accent(tokens: &ThemeTokens) -> Color {
    ti(tokens.accent)
}

/// Text / icon colour for anything painted on [`accent_fill`]: always
/// white, because the fill is darkened until white reaches 4.5:1.
pub fn on_accent(_tokens: &ThemeTokens) -> Color {
    Color::WHITE
}

/// Contrast below which a label counts as lost on its fill. Every label
/// the dark built-ins show sits above it except two that were already
/// unreadable (Catppuccin Mocha "+ Add Filter", GitHub Dark focused ERC
/// detail); those are corrected, the rest keep their colours. The light
/// built-in's lost labels sit far under it.
pub const UNREADABLE_CONTRAST: f32 = 1.5;

/// `preferred`, unless it is lost on `fill` ([`UNREADABLE_CONTRAST`]), in
/// which case [`readable_on`] picks a label that reads. A label keeps its
/// theme colour wherever it is at least visible and is replaced only where
/// it vanishes.
pub fn keep_readable_on(preferred: Color, fill: Color) -> Color {
    if contrast_ratio(preferred, fill) >= UNREADABLE_CONTRAST {
        preferred
    } else {
        readable_on(fill)
    }
}

/// Fill for primary buttons and selected chips / segments: the theme
/// accent, darkened (OKLab lightness lowered, hue and chroma kept) just
/// enough that white text on it reaches [`MIN_TEXT_CONTRAST`]. An accent
/// that already passes is returned unchanged.
pub fn accent_fill(tokens: &ThemeTokens) -> Color {
    let accent = ti(tokens.accent);
    if contrast_ratio(Color::WHITE, accent) >= MIN_TEXT_CONTRAST {
        return accent;
    }
    let (l0, a, b) = to_oklab(accent);
    let mut fill = accent;
    // 0.005 steps; l reaches 0 (black, 21:1) within 200 iterations.
    for step in 1..=200u32 {
        let l = (l0 - 0.005 * step as f32).max(0.0);
        fill = from_oklab(l, a, b);
        if contrast_ratio(Color::WHITE, fill) >= MIN_TEXT_CONTRAST {
            break;
        }
    }
    fill
}

/// Hover shade of [`accent_fill`]: 8 % toward black, so white text only
/// gains contrast.
pub fn accent_fill_hover(tokens: &ThemeTokens) -> Color {
    composite_over(Color::from_rgba(0.0, 0.0, 0.0, 0.08), accent_fill(tokens))
}

/// Pressed shade of [`accent_fill`]: 16 % toward black.
pub fn accent_fill_pressed(tokens: &ThemeTokens) -> Color {
    composite_over(Color::from_rgba(0.0, 0.0, 0.0, 0.16), accent_fill(tokens))
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB colour to OKLab `(L, a, b)`.
fn to_oklab(c: Color) -> (f32, f32, f32) {
    let (r, g, b) = (
        srgb_to_linear(c.r),
        srgb_to_linear(c.g),
        srgb_to_linear(c.b),
    );
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

/// OKLab `(L, a, b)` to an opaque sRGB colour (out-of-gamut channels clamp).
fn from_oklab(l: f32, a: f32, b: f32) -> Color {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    Color::from_rgb(
        linear_to_srgb(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        linear_to_srgb(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_),
        linear_to_srgb(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
    )
}

/// Minimum WCAG contrast for normal-size text.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;

/// Text colour for an opaque fill `bg`. White is preferred whenever it
/// reaches [`MIN_TEXT_CONTRAST`] (the long-standing look on dark and
/// mid-tone fills); only when white fails does the higher-contrast of
/// black / white win.
pub fn readable_on(bg: Color) -> Color {
    let white = contrast_ratio(Color::WHITE, bg);
    if white >= MIN_TEXT_CONTRAST || white >= contrast_ratio(Color::BLACK, bg) {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

/// Destructive / error colour (replaces `rgb(0.85, 0.3, 0.3)`).
pub fn danger(tokens: &ThemeTokens) -> Color {
    ti(tokens.error)
}

/// Alpha of the backdrop behind an in-window modal and of the click-outside
/// layer under a menu.
pub const BACKDROP_ALPHA: f32 = 0.55;

/// Modal backdrop. Black at `alpha` on every theme — a scrim dims the
/// content behind it, which is theme-neutral by design.
pub fn scrim(alpha: f32) -> Color {
    Color::from_rgba(0.0, 0.0, 0.0, alpha)
}

/// Secondary / hint text.
pub fn muted_text(tokens: &ThemeTokens) -> Color {
    ti(tokens.text_secondary)
}

/// Text of a disabled control: secondary text at half strength.
pub fn disabled_text(tokens: &ThemeTokens) -> Color {
    let c = ti(tokens.text_secondary);
    Color { a: c.a * 0.5, ..c }
}

/// Opaque text-input background: the faint overlay composited over the
/// panel, so inputs sit slightly off the surface on any theme.
pub fn input_bg(tokens: &ThemeTokens) -> Color {
    composite_over(overlay(tokens, Overlay::Faint), ti(tokens.panel_bg))
}

/// Text-input border.
pub fn input_border(tokens: &ThemeTokens) -> Color {
    ti(tokens.border)
}

/// Alpha-composite `fg` over an opaque `bg` (result is opaque).
pub fn composite_over(fg: Color, bg: Color) -> Color {
    let mix = |f: f32, b: f32| f * fg.a + b * (1.0 - fg.a);
    Color::from_rgb(mix(fg.r, bg.r), mix(fg.g, bg.g), mix(fg.b, bg.b))
}

/// WCAG relative luminance (linearised sRGB).
pub fn relative_luminance(c: Color) -> f32 {
    let lin = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two opaque colours (1.0 ..= 21.0).
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Every chrome colour a surface can ask for, with the optional
/// [`ThemeTokens::extended`] entries filled in. A theme that leaves an
/// extended token unset gets the colour the app derived before those tokens
/// existed, so legacy themes look the same.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedTokens {
    pub raised: Color,
    pub field: Color,
    pub sunken: Color,
    pub line_soft: Color,
    pub line_strong: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent_text: Color,
    pub accent_soft: Color,
    pub accent_soft_ink: Color,
    pub primary: Color,
    pub primary_ink: Color,
    pub info: Color,
    pub ok_soft: Color,
    pub warn_soft: Color,
    pub err_soft: Color,
    pub info_soft: Color,
    /// Drop-shadow colour; `a` is the strength.
    pub shadow: Color,
    /// Modal scrim colour; `a` is the strength.
    pub scrim: Color,
}

/// Alpha of a soft status / accent tint derived from its base colour.
const SOFT_TINT_ALPHA: f32 = 0.15;

/// Alpha of the shadow under a floating surface when the theme sets none.
const FALLBACK_SHADOW_ALPHA: f32 = 0.35;

fn tint(c: Color, alpha: f32) -> Color {
    Color { a: alpha, ..c }
}

/// Resolve `tokens`: each extended token if set, else its derived fallback.
pub fn resolve(tokens: &ThemeTokens) -> ResolvedTokens {
    let e = &tokens.extended;
    let pick = |set: Option<signex_types::theme::Color>, fallback: Color| set.map_or(fallback, ti);
    let panel = ti(tokens.panel_bg);
    let accent = ti(tokens.accent);
    ResolvedTokens {
        raised: pick(e.raised, panel),
        field: pick(e.field, input_bg(tokens)),
        sunken: pick(
            e.sunken,
            composite_over(overlay(tokens, Overlay::Hover), panel),
        ),
        line_soft: pick(
            e.line_soft,
            composite_over(tint(ti(tokens.border), 0.5), panel),
        ),
        line_strong: pick(e.line_strong, ti(tokens.border)),
        ink3: pick(e.ink3, muted_text(tokens)),
        ink4: pick(e.ink4, disabled_text(tokens)),
        accent_text: pick(e.accent_text, accent),
        accent_soft: pick(e.accent_soft, tint(accent, SOFT_TINT_ALPHA)),
        accent_soft_ink: pick(e.accent_soft_ink, ti(tokens.text)),
        primary: pick(e.primary, accent_fill(tokens)),
        primary_ink: pick(e.primary_ink, on_accent(tokens)),
        info: pick(e.info, accent),
        ok_soft: pick(e.ok_soft, tint(ti(tokens.success), SOFT_TINT_ALPHA)),
        warn_soft: pick(e.warn_soft, tint(ti(tokens.warning), SOFT_TINT_ALPHA)),
        err_soft: pick(e.err_soft, tint(ti(tokens.error), SOFT_TINT_ALPHA)),
        info_soft: pick(e.info_soft, tint(accent, SOFT_TINT_ALPHA)),
        shadow: pick(e.shadow, scrim(FALLBACK_SHADOW_ALPHA)),
        scrim: pick(e.scrim, scrim(BACKDROP_ALPHA)),
    }
}
