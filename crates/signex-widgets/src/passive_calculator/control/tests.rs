use super::*;

/// A control that has just produced a network for target `100`.
fn calculated() -> CalculatorControl {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::TargetChanged("100".into()));
    control.update(CalculatorMessage::Calculate);
    assert!(
        control.active_state().result.is_some(),
        "setup: Calculate must produce a network for target 100"
    );
    control
}

#[test]
fn switching_to_a_passive_tab_selects_that_kind_for_edits() {
    let mut control = CalculatorControl::default();
    control.update(CalculatorMessage::TabChanged(CalculatorTab::Passive(
        ComponentKind::Capacitor,
    )));
    assert_eq!(control.kind, ComponentKind::Capacitor);
    control.update(CalculatorMessage::TargetChanged("47".into()));
    assert_eq!(control.state(ComponentKind::Capacitor).target_input, "47");
    assert_eq!(control.state(ComponentKind::Resistor).target_input, "220");
}

// GH #599 — the result used to survive an edit that left the target
// unreadable, and the summary then rendered against `target = 0.0`:
// "Target" read 0 and all three deltas were measured against 0 while
// Nominal / Minimum / Maximum still showed the real network. No
// error text either, because TargetChanged clears validation_error.
#[test]
fn an_unreadable_target_edit_drops_the_result_it_was_solved_for() {
    let mut control = calculated();
    control.update(CalculatorMessage::TargetChanged("22o".into()));
    assert!(
        control.active_state().result.is_none(),
        "a result outlived the target it was solved for"
    );
}

#[test]
fn clearing_the_target_drops_the_result() {
    let mut control = calculated();
    control.update(CalculatorMessage::TargetChanged(String::new()));
    assert!(control.active_state().result.is_none());
}

// Mid-typing states are the same shape: `2.` is not yet a number.
#[test]
fn a_partial_target_drops_the_result() {
    let mut control = calculated();
    control.update(CalculatorMessage::TargetChanged("2.".into()));
    assert!(control.active_state().result.is_none());
}

// A readable edit must still clear the stale network — the siblings
// (PrefixChanged / SeriesChanged / MaxComponentsChanged) all do.
#[test]
fn a_readable_target_edit_also_drops_the_stale_result() {
    let mut control = calculated();
    control.update(CalculatorMessage::TargetChanged("220".into()));
    assert!(control.active_state().result.is_none());
    control.update(CalculatorMessage::Calculate);
    assert!(control.active_state().result.is_some());
}

// GH #599 — a missing target must never render as a real `0`.
#[test]
fn target_dependent_metrics_read_unavailable_without_a_target() {
    assert_eq!(
        format_target(None, ComponentKind::Resistor),
        TARGET_UNAVAILABLE
    );
    assert_eq!(format_optional_difference(120.0, None), TARGET_UNAVAILABLE);
    assert_eq!(
        format_optional_difference(100.0, Some(100.0)),
        format_difference(100.0, 100.0)
    );
}

// ---- Graphite layout (mockup row 17, #2262) ----

mod graphite_layout {
    use chrono::NaiveDate;
    use iced::{Color, Font, Theme};
    use signex_i18n::{I18n, ids, t};
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;
    use crate::checkbox;
    use crate::enable_section::EnableSectionStyle;
    use crate::passive_calculator::look::{CalculatorLook, CalculatorPalette};
    use crate::passive_calculator::production_date_code::{ProductionDateCycle, ProductionMonth};
    use crate::passive_calculator::rkm_encoder::RkmEncoderMessage;
    use crate::passive_calculator::widgets::{NARROW_WIDTH, TS};
    use crate::segmented::SegmentedStyle;

    fn palette() -> CalculatorPalette {
        let c = Color::BLACK;
        CalculatorPalette {
            ink: c,
            ink2: c,
            ink3: c,
            ink4: c,
            accent: c,
            accent_text: c,
            border: c,
            line_soft: c,
            line_strong: c,
            surface: c,
            panel: c,
            field: c,
            sunken: c,
            primary: c,
            primary_ink: c,
            hover: c,
            ok: c,
            ok_soft: c,
            warn: c,
            warn_soft: c,
            error: c,
        }
    }

    fn look() -> CalculatorLook<'static> {
        let c = Color::BLACK;
        CalculatorLook {
            palette: palette(),
            mono: Font::MONOSPACE,
            segmented: SegmentedStyle::new(c, c, c, c, c, TS.body),
            enable_section: EnableSectionStyle {
                header_ink: c,
                disabled_ink: c,
                panel: c,
                font: Font::DEFAULT,
                checkbox: Box::new(move |_: &Theme, _| checkbox::Style {
                    background: iced::Background::Color(c),
                    icon_color: c,
                    border: iced::Border::default(),
                    text_color: None,
                }),
                tip: Box::new(|_: &Theme| iced::widget::container::Style::default()),
            },
        }
    }

    fn encoder_message(control: &mut CalculatorControl, message: RkmEncoderMessage) {
        control.update(CalculatorMessage::RkmEncoder(message));
    }

    fn date(year: i32, month: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, 15).expect("a valid date")
    }

    #[test]
    fn the_production_date_is_off_by_default_and_gives_no_code() {
        let control = CalculatorControl::default();
        assert!(!control.rkm_encoder.is_production_date_enabled);
        assert!(control.rkm_encoder.production_date_code().is_none());
        assert_eq!(
            t!(I18n::en_us_ref(), ids::PASSIVE_CALCULATOR_DATE_OFF).into_string(),
            "Off"
        );
    }

    #[test]
    fn switching_the_production_date_off_keeps_the_picks_and_on_restores_them() {
        let mut control = CalculatorControl::default();
        encoder_message(&mut control, RkmEncoderMessage::ProductionDateToggled(true));
        encoder_message(
            &mut control,
            RkmEncoderMessage::ProductionDateCycleChanged(ProductionDateCycle::TenYear),
        );
        encoder_message(&mut control, RkmEncoderMessage::ProductionYearChanged(2024));
        encoder_message(
            &mut control,
            RkmEncoderMessage::ProductionMonthChanged(ProductionMonth::March),
        );
        let on = control.rkm_encoder.production_date_code();
        assert_eq!(on.map(|code| code.to_string()), Some("43".to_owned()));

        encoder_message(
            &mut control,
            RkmEncoderMessage::ProductionDateToggled(false),
        );
        let encoder = &control.rkm_encoder;
        assert!(
            encoder.production_date_code().is_none(),
            "off gives no code"
        );
        assert_eq!(
            encoder.production_date_cycle,
            Some(ProductionDateCycle::TenYear)
        );
        assert_eq!(encoder.production_year, Some(2024));
        assert_eq!(encoder.production_month, Some(ProductionMonth::March));

        encoder_message(&mut control, RkmEncoderMessage::ProductionDateToggled(true));
        assert_eq!(control.rkm_encoder.production_date_code(), on);
    }

    #[test]
    fn the_first_switch_on_prefills_today() {
        let mut encoder = RkmEncoder::default();
        encoder.set_production_date_enabled(true, date(2026, 11));
        assert!(encoder.is_production_date_enabled);
        assert_eq!(
            encoder.production_date_cycle,
            Some(ProductionDateCycle::TwentyYear)
        );
        assert_eq!(encoder.production_year, Some(2026));
        assert_eq!(encoder.production_month, Some(ProductionMonth::November));
        assert!(encoder.production_date_code().is_some());
    }

    #[test]
    fn a_later_switch_on_keeps_the_earlier_picks() {
        let mut encoder = RkmEncoder::default();
        encoder.set_production_date_enabled(true, date(2026, 11));
        encoder.update(RkmEncoderMessage::ProductionYearChanged(2001));
        encoder.set_production_date_enabled(false, date(2026, 11));
        encoder.set_production_date_enabled(true, date(2030, 1));
        assert_eq!(encoder.production_year, Some(2001));
        assert_eq!(encoder.production_month, Some(ProductionMonth::November));
    }

    #[test]
    fn the_optional_section_starts_open_and_folds() {
        let mut control = CalculatorControl::default();
        assert!(control.rkm_encoder.is_optional_expanded);
        encoder_message(&mut control, RkmEncoderMessage::OptionalSectionToggled);
        assert!(!control.rkm_encoder.is_optional_expanded);
        encoder_message(&mut control, RkmEncoderMessage::OptionalSectionToggled);
        assert!(control.rkm_encoder.is_optional_expanded);
    }

    /// The pill is ok while the nominal value is within the tolerance the
    /// series is solved at (E24: 5 %), warn past it; no pill without a
    /// target or against a zero or infinite one.
    #[test]
    fn the_status_pill_is_ok_within_the_series_tolerance() {
        assert_eq!(
            off_target(100.0, Some(100.0), ESeries::E24),
            Some((0.0, OffTargetTone::Ok))
        );
        let (percent, tone) = off_target(104.0, Some(100.0), ESeries::E24).expect("a pill");
        assert!((percent - 4.0).abs() < 1e-9);
        assert_eq!(tone, OffTargetTone::Ok);
        let (percent, tone) = off_target(94.0, Some(100.0), ESeries::E24).expect("a pill");
        assert!((percent - 6.0).abs() < 1e-9, "the distance is unsigned");
        assert_eq!(tone, OffTargetTone::Warn);
        assert_eq!(
            off_target(94.0, Some(100.0), ESeries::E3).map(|(_, tone)| tone),
            Some(OffTargetTone::Ok),
            "E3 parts are solved at 20 %"
        );
        assert_eq!(off_target(100.0, None, ESeries::E24), None);
        assert_eq!(off_target(5.0, Some(0.0), ESeries::E24), None);
        assert_eq!(
            off_target(f64::INFINITY, Some(f64::INFINITY), ESeries::E24),
            Some((0.0, OffTargetTone::Ok))
        );
    }

    #[test]
    fn a_target_error_is_a_catalog_message_not_english_state() {
        let mut control = CalculatorControl::default();
        control.update(CalculatorMessage::TargetChanged("abc".into()));
        control.update(CalculatorMessage::Calculate);
        assert_eq!(
            control.active_state().validation_error,
            Some(TargetError::NotANumber)
        );
        control.update(CalculatorMessage::TargetChanged("-2".into()));
        control.update(CalculatorMessage::Calculate);
        assert_eq!(
            control.active_state().validation_error,
            Some(TargetError::Negative)
        );
    }

    /// Every page builds, wide and narrow, with and without a result, on
    /// both Graphite themes.
    #[test]
    fn every_page_builds_wide_and_narrow() {
        let i18n = I18n::en_us_ref();
        for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(theme);
            let page_look = super::super::view::PageLook {
                palette: palette(),
                mono: Font::MONOSPACE,
            };
            let mut control = CalculatorControl::default();
            for width in [NARROW_WIDTH - 1.0, NARROW_WIDTH + 200.0] {
                let _ = control.network_page(i18n, &tokens, page_look, width);
                let _ = control
                    .rkm_encoder
                    .inputs(i18n, palette(), Font::MONOSPACE, width);
                let _ = control
                    .rkm_encoder
                    .output(i18n, palette(), Font::MONOSPACE, width);
            }
            control.update(CalculatorMessage::Calculate);
            for width in [NARROW_WIDTH - 1.0, NARROW_WIDTH + 200.0] {
                let _ = control.network_page(i18n, &tokens, page_look, width);
            }
            let _ = control.view(i18n, &tokens, look());
            control.update(CalculatorMessage::TabChanged(CalculatorTab::RkmEncoder));
            let _ = control.view(i18n, &tokens, look());
        }
    }

    /// The pages word every label from the catalog: no view file calls
    /// `raw`, and the one data helper and the one upper-casing helper are
    /// its only callers in the calculator.
    #[test]
    fn the_views_take_their_words_from_the_catalog() {
        let views = [
            ("control/view.rs", include_str!("view.rs")),
            ("control/result_view.rs", include_str!("result_view.rs")),
            (
                "rkm_encoder_view.rs",
                include_str!("../rkm_encoder_view.rs"),
            ),
            ("component_card.rs", include_str!("../component_card.rs")),
            ("color_code_view.rs", include_str!("../color_code_view.rs")),
            ("network_drawing.rs", include_str!("../network_drawing.rs")),
        ];
        for (name, source) in views {
            assert_eq!(raw_calls(source), 0, "{name} calls raw");
        }
        let helpers = include_str!("../widgets.rs");
        assert_eq!(raw_calls(helpers), 2);
        assert_eq!(helpers.matches("// raw:").count(), 2);
    }

    /// Calls of the `raw` escape hatch itself, not of `draw` or another name
    /// ending in it. (The needle has no parenthesis, so the raw-label
    /// ratchet does not count this helper as a call site.)
    fn raw_calls(source: &str) -> usize {
        source
            .match_indices("raw")
            .filter(|(at, _)| {
                let is_called = source[at + 3..].starts_with('(');
                let starts_name = source[..*at]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !(before.is_alphanumeric() || before == '_'));
                is_called && starts_name
            })
            .count()
    }
}
