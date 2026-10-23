//! The PCB trace calculator dialog body: editable text fields, unit
//! pickers and the two result panels. The maths lives in
//! [`super::domain`]; this file only parses text, calls it and draws.

use std::fmt;
use std::time::{Duration, Instant};

use iced::animation::Easing;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Animation, Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use super::diagram::{self, DiagramCache, width_labels};
use super::domain::{
    AmbientUnit, CalculationError, InputField, LengthUnit, RiseUnit, ThicknessUnit,
    TraceCalculation, TraceInput, WidthUnit, calculate,
};
use super::results;
use crate::theme_ext;

const BODY_SIZE: f32 = 11.0;
const CAPTION_SIZE: f32 = 10.0;
const CONTENT_PADDING: u16 = 12;
const SECTION_SPACING: f32 = 14.0;
const ROW_SPACING: f32 = 8.0;
const LABEL_WIDTH: f32 = 120.0;
const UNIT_PICKER_WIDTH: f32 = 90.0;
const STATIC_BOX_RADIUS: f32 = 2.0;
const BUTTON_RADIUS: f32 = 4.0;
const CONTROL_PADDING: [u16; 2] = [4, 6];

/// The curves already carry about 10 % current derating.
const DERATING_NOTE: &str = "The curves include about 10 % derating; consider a further 15 % for boards up to 0.8 mm thick or copper of 108 \u{b5}m and above.";

/// How long the values and the status text take to fade in after the
/// result state changes.
const FADE_DURATION: Duration = Duration::from_millis(150);

const DEFAULT_CURRENT: &str = "5";
const DEFAULT_THICKNESS: &str = "1";
const DEFAULT_RISE: &str = "20";
const DEFAULT_AMBIENT: &str = "20";
const DEFAULT_LENGTH: &str = "50";

/// Why the result panels cannot be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceCalculatorError {
    /// A field's text is not a number.
    InvalidNumber(InputField),
    /// The numbers parsed but the domain rejected them.
    Calculation(CalculationError),
}

impl fmt::Display for TraceCalculatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNumber(field) => write!(f, "Enter a numeric value for {field}."),
            Self::Calculation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TraceCalculatorError {}

impl From<CalculationError> for TraceCalculatorError {
    fn from(error: CalculationError) -> Self {
        Self::Calculation(error)
    }
}

#[derive(Debug, Clone)]
pub enum TraceCalculatorMessage {
    CurrentChanged(String),
    ThicknessChanged(String),
    ThicknessUnitChanged(ThicknessUnit),
    RiseChanged(String),
    RiseUnitChanged(RiseUnit),
    AmbientChanged(String),
    AmbientUnitChanged(AmbientUnit),
    LengthChanged(String),
    LengthUnitChanged(LengthUnit),
    InnerWidthUnitChanged(WidthUnit),
    OuterWidthUnitChanged(WidthUnit),
    ResetPressed,
    /// A display frame while the fade runs; carries the frame time.
    FrameArrived(Instant),
}

#[derive(Debug, Clone)]
pub struct TraceCalculatorControl {
    pub current: String,
    pub thickness: String,
    pub thickness_unit: ThicknessUnit,
    pub rise: String,
    pub rise_unit: RiseUnit,
    pub ambient: String,
    pub ambient_unit: AmbientUnit,
    pub length: String,
    pub length_unit: LengthUnit,
    pub inner_width_unit: WidthUnit,
    pub outer_width_unit: WidthUnit,
    /// The failure last handed to the caller by [`Self::update`], so a
    /// persisting error is reported once and not on every keystroke.
    reported: Option<TraceCalculatorError>,
    /// Geometry cache of the explanatory diagram.
    diagram_cache: DiagramCache,
    /// What the result area last showed: whether it had values, and its
    /// status text. A change restarts the fade.
    shown: (bool, String),
    fade: Animation<f32>,
    /// Time of the latest edit or frame; the fade is read at this instant.
    now: Instant,
}

impl Default for TraceCalculatorControl {
    fn default() -> Self {
        Self {
            current: DEFAULT_CURRENT.to_string(),
            thickness: DEFAULT_THICKNESS.to_string(),
            thickness_unit: ThicknessUnit::OuncePerSquareFoot,
            rise: DEFAULT_RISE.to_string(),
            rise_unit: RiseUnit::Celsius,
            ambient: DEFAULT_AMBIENT.to_string(),
            ambient_unit: AmbientUnit::Celsius,
            length: DEFAULT_LENGTH.to_string(),
            length_unit: LengthUnit::Millimeter,
            inner_width_unit: WidthUnit::Millimeter,
            outer_width_unit: WidthUnit::Millimeter,
            reported: None,
            diagram_cache: DiagramCache::default(),
            shown: (true, String::new()),
            fade: Animation::new(1.0),
            now: Instant::now(),
        }
    }
}

impl TraceCalculatorControl {
    /// Apply an edit. Returns the failure this edit newly produced, if
    /// any: the dialog already shows it, and the caller is expected to
    /// route it to the application log as well.
    ///
    /// A failure is returned once and then remembered, whatever valid or
    /// invalid states follow, until a different failure occurs or
    /// [`Self::forget_reported_failure`] is called. Typing `1`, clearing
    /// the field, typing `1` again would otherwise log the same line on
    /// every other keystroke.
    pub fn update(&mut self, message: TraceCalculatorMessage) -> Option<TraceCalculatorError> {
        match message {
            TraceCalculatorMessage::CurrentChanged(value) => self.current = value,
            TraceCalculatorMessage::ThicknessChanged(value) => self.thickness = value,
            TraceCalculatorMessage::ThicknessUnitChanged(unit) => self.thickness_unit = unit,
            TraceCalculatorMessage::RiseChanged(value) => self.rise = value,
            TraceCalculatorMessage::RiseUnitChanged(unit) => self.rise_unit = unit,
            TraceCalculatorMessage::AmbientChanged(value) => self.ambient = value,
            TraceCalculatorMessage::AmbientUnitChanged(unit) => self.ambient_unit = unit,
            TraceCalculatorMessage::LengthChanged(value) => self.length = value,
            TraceCalculatorMessage::LengthUnitChanged(unit) => self.length_unit = unit,
            TraceCalculatorMessage::InnerWidthUnitChanged(unit) => self.inner_width_unit = unit,
            TraceCalculatorMessage::OuterWidthUnitChanged(unit) => self.outer_width_unit = unit,
            TraceCalculatorMessage::ResetPressed => *self = Self::default(),
            TraceCalculatorMessage::FrameArrived(at) => {
                self.now = at;
                return None;
            }
        }
        let outcome = self.calculation();
        self.restart_fade_if_the_result_state_changed(&outcome);
        let failure = outcome.err()?;
        if self.reported.as_ref() == Some(&failure) {
            return None;
        }
        self.reported = Some(failure.clone());
        Some(failure)
    }

    /// Start a new reporting session: the next failure is returned again
    /// even if it equals the last one. Called when the dialog is opened.
    pub fn forget_reported_failure(&mut self) {
        self.reported = None;
    }

    /// Whether the fade is still running, so a host knows to deliver
    /// [`TraceCalculatorMessage::FrameArrived`] and otherwise not to.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.fade.is_animating(self.now)
    }

    fn restart_fade_if_the_result_state_changed(
        &mut self,
        outcome: &Result<TraceCalculation, TraceCalculatorError>,
    ) {
        let status =
            results::results_model(outcome, self.inner_width_unit, self.outer_width_unit).status;
        let shown = (outcome.is_ok(), status.text);
        if shown == self.shown {
            return;
        }
        self.shown = shown;
        self.now = Instant::now();
        self.fade = Animation::new(0.0)
            .easing(Easing::EaseOut)
            .duration(FADE_DURATION)
            .go(1.0, self.now);
    }

    /// Parse the fields, convert to canonical units and run the domain.
    ///
    /// # Errors
    ///
    /// A field that is not a number, or whatever the domain rejects.
    pub fn calculation(&self) -> Result<TraceCalculation, TraceCalculatorError> {
        let current = parse_field(InputField::Current, &self.current)?;
        let thickness = parse_field(InputField::CopperThickness, &self.thickness)?;
        let rise = parse_field(InputField::TemperatureRise, &self.rise)?;
        let ambient = parse_field(InputField::AmbientTemperature, &self.ambient)?;
        let length = parse_field(InputField::TraceLength, &self.length)?;
        Ok(calculate(TraceInput {
            current_a: current,
            copper_thickness_m: self.thickness_unit.to_metres(thickness),
            temperature_rise_c: self.rise_unit.to_celsius_degrees(rise),
            ambient_c: self.ambient_unit.to_celsius(ambient),
            length_m: self.length_unit.to_metres(length),
        })?)
    }

    pub fn view<'a>(&'a self, tokens: &'a ThemeTokens) -> Element<'a, TraceCalculatorMessage> {
        let secondary = theme_ext::text_secondary(tokens);
        let header = row![
            column![
                text("IPC-2221 conductor sizing")
                    .size(BODY_SIZE)
                    .color(secondary),
                text("IPC-2221 curve fit; IPC-2152 supersedes it for current capacity.")
                    .size(CAPTION_SIZE)
                    .color(secondary),
                text(DERATING_NOTE).size(CAPTION_SIZE).color(secondary),
            ]
            .spacing(2)
            .width(Length::Fill),
            button(text("Reset").size(BODY_SIZE))
                .padding(CONTROL_PADDING)
                .on_press(TraceCalculatorMessage::ResetPressed)
                .style(move |_theme: &Theme, status| button_style(tokens, status)),
        ]
        .align_y(Alignment::Start);

        let outcome = self.calculation();
        let labels = width_labels(
            outcome.as_ref().ok(),
            self.outer_width_unit,
            self.inner_width_unit,
        );
        let diagram = diagram::view(&self.diagram_cache, labels, tokens);
        let model = results::results_model(&outcome, self.inner_width_unit, self.outer_width_unit);
        let alpha = self.fade.interpolate_with(|value| value, self.now);
        let results = results::view(model, alpha, tokens);

        let content = column![header, self.inputs_view(tokens), diagram, results]
            .spacing(SECTION_SPACING)
            .padding(CONTENT_PADDING);
        scrollable(content).height(Length::Shrink).into()
    }

    fn inputs_view<'a>(&'a self, tokens: &'a ThemeTokens) -> Element<'a, TraceCalculatorMessage> {
        use TraceCalculatorMessage as M;
        column![
            input_row(
                "Current",
                &self.current,
                static_unit_box("A", tokens),
                M::CurrentChanged,
                tokens,
            ),
            input_row(
                "Copper thickness",
                &self.thickness,
                unit_picker(
                    ThicknessUnit::ALL,
                    self.thickness_unit,
                    M::ThicknessUnitChanged
                ),
                M::ThicknessChanged,
                tokens,
            ),
            input_row(
                "Temperature rise",
                &self.rise,
                unit_picker(RiseUnit::ALL, self.rise_unit, M::RiseUnitChanged),
                M::RiseChanged,
                tokens,
            ),
            input_row(
                "Ambient temperature",
                &self.ambient,
                unit_picker(AmbientUnit::ALL, self.ambient_unit, M::AmbientUnitChanged),
                M::AmbientChanged,
                tokens,
            ),
            input_row(
                "Trace length",
                &self.length,
                unit_picker(LengthUnit::ALL, self.length_unit, M::LengthUnitChanged),
                M::LengthChanged,
                tokens,
            ),
        ]
        .spacing(ROW_SPACING)
        .into()
    }
}

fn input_row<'a>(
    label: &'static str,
    value: &'a str,
    unit: impl Into<Element<'a, TraceCalculatorMessage>>,
    on_input: fn(String) -> TraceCalculatorMessage,
    tokens: &'a ThemeTokens,
) -> Element<'a, TraceCalculatorMessage> {
    row![
        text(label)
            .size(BODY_SIZE)
            .color(theme_ext::text_primary(tokens))
            .width(LABEL_WIDTH),
        text_input("", value)
            .on_input(on_input)
            .size(BODY_SIZE)
            .padding(CONTROL_PADDING)
            .width(Length::Fill),
        unit.into(),
    ]
    .align_y(Alignment::Center)
    .spacing(ROW_SPACING)
    .into()
}

/// A unit that cannot be changed (the current is always in amperes),
/// drawn with the size and border of the pickers so the unit column lines
/// up.
fn static_unit_box<'a>(
    label: &'static str,
    tokens: &'a ThemeTokens,
) -> Element<'a, TraceCalculatorMessage> {
    container(
        text(label)
            .size(BODY_SIZE)
            .color(theme_ext::text_secondary(tokens)),
    )
    .padding(CONTROL_PADDING)
    .width(UNIT_PICKER_WIDTH)
    .style(move |_theme| container::Style {
        border: Border {
            width: 1.0,
            radius: STATIC_BOX_RADIUS.into(),
            color: theme_ext::border_color(tokens),
        },
        ..container::Style::default()
    })
    .into()
}

pub(super) fn unit_picker<'a, U, const N: usize>(
    units: [U; N],
    selected: U,
    on_change: fn(U) -> TraceCalculatorMessage,
) -> Element<'a, TraceCalculatorMessage>
where
    U: Copy + PartialEq + fmt::Display + 'a,
{
    pick_list(units, Some(selected), on_change)
        .text_size(BODY_SIZE)
        .padding(CONTROL_PADDING)
        .width(UNIT_PICKER_WIDTH)
        .into()
}

fn button_style(tokens: &ThemeTokens, status: button::Status) -> button::Style {
    let background = if status == button::Status::Hovered {
        theme_ext::hover_color(tokens)
    } else {
        theme_ext::to_color(&tokens.panel_bg)
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: theme_ext::text_primary(tokens),
        border: Border {
            width: 1.0,
            radius: BUTTON_RADIUS.into(),
            color: theme_ext::border_color(tokens),
        },
        ..button::Style::default()
    }
}

/// Read a field. The text is trimmed, and one comma is accepted as the
/// decimal separator. Input that could be read two ways is refused rather
/// than guessed: several commas, a comma next to a point, and a comma
/// followed by exactly three digits (`1,000` is a thousand to some and one
/// to others).
fn parse_field(field: InputField, text: &str) -> Result<f64, TraceCalculatorError> {
    let invalid = || TraceCalculatorError::InvalidNumber(field);
    let trimmed = text.trim();
    if is_ambiguous_comma(trimmed) {
        return Err(invalid());
    }
    trimmed
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| invalid())
}

fn is_ambiguous_comma(text: &str) -> bool {
    match text.matches(',').count() {
        0 => false,
        1 => {
            let (whole, fraction) = text.split_once(',').unwrap_or((text, ""));
            text.contains('.')
                || (fraction.len() == 3
                    && fraction.bytes().all(|b| b.is_ascii_digit())
                    && !whole.is_empty()
                    && whole.bytes().all(|b| b.is_ascii_digit()))
        }
        _ => true,
    }
}

/// About three significant figures in plain notation, scientific outside
/// 1e-4..1e6 and for a non-finite value.
pub fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if !value.is_finite() {
        return value.to_string();
    }
    let exponent = value.abs().log10().floor() as i32;
    if !(-4..=5).contains(&exponent) {
        return format!("{value:.3e}");
    }
    let decimals = (2 - exponent).clamp(0, 8) as usize;
    format!("{value:.decimals$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_calculates_cleanly_and_draws() {
        let control = TraceCalculatorControl::default();
        let calculation = control.calculation().expect("defaults are valid");
        assert!(calculation.warnings.is_empty());
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
        let _: Element<'_, TraceCalculatorMessage> = control.view(&tokens);
    }

    #[test]
    fn a_non_numeric_field_names_itself() {
        let mut control = TraceCalculatorControl::default();
        let failure = control.update(TraceCalculatorMessage::CurrentChanged("5 amps".into()));
        assert_eq!(
            failure,
            Some(TraceCalculatorError::InvalidNumber(InputField::Current))
        );
        assert_eq!(
            failure.expect("failure").to_string(),
            "Enter a numeric value for current."
        );
    }

    #[test]
    fn an_empty_field_is_not_a_number() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::LengthChanged(String::new()));
        assert_eq!(
            control.calculation(),
            Err(TraceCalculatorError::InvalidNumber(InputField::TraceLength))
        );
    }

    #[test]
    fn ambiguous_comma_input_is_refused_not_guessed() {
        for text in ["1,000", "1,2,3", "1,000.5", "1.000,5", "12,345"] {
            let mut control = TraceCalculatorControl::default();
            control.update(TraceCalculatorMessage::CurrentChanged(text.into()));
            assert_eq!(
                control.calculation(),
                Err(TraceCalculatorError::InvalidNumber(InputField::Current)),
                "{text:?} must not parse"
            );
        }
        for text in ["1,5", "0,25", "1,50", "1,5000", ",5"] {
            let mut control = TraceCalculatorControl::default();
            control.update(TraceCalculatorMessage::CurrentChanged(text.into()));
            assert!(control.calculation().is_ok(), "{text:?} is a decimal comma");
        }
    }

    #[test]
    fn a_comma_decimal_separator_and_padding_are_accepted() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::CurrentChanged("  2,5 ".into()));
        assert!(control.calculation().is_ok());
    }

    #[test]
    fn a_valid_edit_reports_no_failure() {
        let mut control = TraceCalculatorControl::default();
        assert_eq!(
            control.update(TraceCalculatorMessage::CurrentChanged("8".into())),
            None
        );
    }

    #[test]
    fn a_domain_rejection_is_surfaced_as_a_failure() {
        let mut control = TraceCalculatorControl::default();
        let failure = control.update(TraceCalculatorMessage::CurrentChanged("-1".into()));
        assert_eq!(
            failure,
            Some(TraceCalculatorError::Calculation(
                CalculationError::NonPositive(InputField::Current)
            ))
        );
    }

    #[test]
    fn a_failure_is_reported_once_even_across_recoveries() {
        let mut control = TraceCalculatorControl::default();
        let first = control.update(TraceCalculatorMessage::CurrentChanged("x".into()));
        let repeat = control.update(TraceCalculatorMessage::CurrentChanged("xy".into()));
        let recovered = control.update(TraceCalculatorMessage::CurrentChanged("5".into()));
        let same_again = control.update(TraceCalculatorMessage::CurrentChanged("y".into()));

        assert!(first.is_some());
        assert!(repeat.is_none(), "same failure must not be re-reported");
        assert!(recovered.is_none());
        assert!(
            same_again.is_none(),
            "the same failure after a recovery is not new"
        );
    }

    #[test]
    fn a_different_failure_is_reported_and_so_is_the_first_after_forgetting() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::CurrentChanged("x".into()));
        control.update(TraceCalculatorMessage::CurrentChanged("5".into()));
        let other = control.update(TraceCalculatorMessage::LengthChanged("y".into()));
        assert_eq!(
            other,
            Some(TraceCalculatorError::InvalidNumber(InputField::TraceLength))
        );

        control.forget_reported_failure();
        let after_reopen = control.update(TraceCalculatorMessage::LengthChanged("z".into()));
        assert!(after_reopen.is_some(), "a new session reports again");
    }

    #[test]
    fn replaying_valid_and_empty_on_one_field_reports_exactly_once() {
        let mut control = TraceCalculatorControl::default();
        let mut reported = 0;
        for text in ["1", "", "1", "", "1", ""] {
            let failure = control.update(TraceCalculatorMessage::ThicknessChanged(text.into()));
            reported += usize::from(failure.is_some());
        }
        assert_eq!(reported, 1);
    }

    #[test]
    fn a_result_state_change_starts_the_fade_and_a_frame_past_it_ends_it() {
        let mut control = TraceCalculatorControl::default();
        assert!(!control.is_animating(), "idle on open: no frame loop");

        control.update(TraceCalculatorMessage::CurrentChanged("x".into()));
        assert!(control.is_animating());

        let later = Instant::now() + FADE_DURATION * 2;
        control.update(TraceCalculatorMessage::FrameArrived(later));
        assert!(
            !control.is_animating(),
            "the fade ends, so the host can stop ticking"
        );
    }

    #[test]
    fn typing_another_valid_value_does_not_restart_the_fade() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::CurrentChanged("6".into()));
        assert!(!control.is_animating());
    }

    #[test]
    fn switching_the_thickness_unit_changes_the_width_not_the_area() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::ThicknessChanged("35".into()));
        control.update(TraceCalculatorMessage::ThicknessUnitChanged(
            ThicknessUnit::Micrometer,
        ));
        let micrometers = control.calculation().expect("valid");

        control.update(TraceCalculatorMessage::ThicknessChanged("1.378".into()));
        control.update(TraceCalculatorMessage::ThicknessUnitChanged(
            ThicknessUnit::Mil,
        ));
        let mils = control.calculation().expect("valid");

        assert_eq!(micrometers.inner.area_mil2, mils.inner.area_mil2);
        let ratio = micrometers.inner.width_m / mils.inner.width_m;
        assert!(
            (ratio - 1.0).abs() < 1.0e-3,
            "35 um is about 1.378 mil: {ratio}"
        );
    }

    #[test]
    fn a_unit_switch_keeps_the_typed_text() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::LengthUnitChanged(LengthUnit::Inch));
        assert_eq!(control.length, "50");
        assert_eq!(control.length_unit, LengthUnit::Inch);
    }

    #[test]
    fn reset_restores_the_defaults_and_forgets_the_reported_failure() {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::CurrentChanged("x".into()));
        control.update(TraceCalculatorMessage::ResetPressed);
        assert_eq!(control.current, DEFAULT_CURRENT);
        let failure = control.update(TraceCalculatorMessage::CurrentChanged("x".into()));
        assert!(failure.is_some(), "the same failure is new after a reset");
    }

    #[test]
    fn format_number_handles_zero_and_the_plain_range() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(5.0), "5.00");
        assert_eq!(format_number(256.27), "256");
        assert_eq!(format_number(0.0123), "0.0123");
        assert_eq!(format_number(123_456.0), "123456");
        assert_eq!(format_number(-4.5678), "-4.57");
    }

    #[test]
    fn format_number_switches_to_scientific_outside_the_plain_range() {
        assert_eq!(format_number(1.0e-5), "1.000e-5");
        assert_eq!(format_number(1.0e6), "1.000e6");
        assert_eq!(format_number(0.000_012_34), "1.234e-5");
        assert_eq!(format_number(0.000_123_4), "0.000123");
    }

    #[test]
    fn format_number_does_not_panic_on_non_finite_values() {
        assert_eq!(format_number(f64::INFINITY), "inf");
        assert_eq!(format_number(f64::NAN), "NaN");
    }
}
