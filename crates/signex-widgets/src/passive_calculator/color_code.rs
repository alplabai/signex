use iced::Color;

use super::domain::{ComponentKind, PreferredComponent, Tolerance};
use super::rkm_code::TemperatureCoefficient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BandColor {
    Pink,
    Silver,
    Gold,
    Black,
    Brown,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    Grey,
    White,
}

impl BandColor {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pink => "Pink",
            Self::Silver => "Silver",
            Self::Gold => "Gold",
            Self::Black => "Black",
            Self::Brown => "Brown",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Violet => "Violet",
            Self::Grey => "Grey",
            Self::White => "White",
        }
    }

    /// Non-normative RAL associations from the Wikipedia electronic
    /// color-code table. Gold and silver have no RAL entry there.
    pub const fn ral(self) -> Option<u16> {
        match self {
            Self::Pink => Some(3015),
            Self::Black => Some(9005),
            Self::Brown => Some(8003),
            Self::Red => Some(3000),
            Self::Orange => Some(2003),
            Self::Yellow => Some(1021),
            Self::Green => Some(6018),
            Self::Blue => Some(5015),
            Self::Violet => Some(4005),
            Self::Grey => Some(7000),
            Self::White => Some(1013),
            Self::Silver | Self::Gold => None,
        }
    }

    /// Screen approximation of the associated RAL color.
    pub const fn color(self) -> Color {
        match self {
            Self::Pink => Color::from_rgb8(0xD8, 0xA0, 0xA6),
            Self::Silver => Color::from_rgb8(0xC0, 0xC0, 0xC0),
            Self::Gold => Color::from_rgb8(0xD4, 0xAF, 0x37),
            Self::Black => Color::from_rgb8(0x0A, 0x0A, 0x0D),
            Self::Brown => Color::from_rgb8(0x7A, 0x3B, 0x2E),
            Self::Red => Color::from_rgb8(0xAF, 0x2B, 0x1E),
            Self::Orange => Color::from_rgb8(0xF6, 0x78, 0x28),
            Self::Yellow => Color::from_rgb8(0xF3, 0xDA, 0x0B),
            Self::Green => Color::from_rgb8(0x57, 0xA6, 0x39),
            Self::Blue => Color::from_rgb8(0x28, 0x74, 0xB2),
            Self::Violet => Color::from_rgb8(0x83, 0x63, 0x9D),
            Self::Grey => Color::from_rgb8(0x7E, 0x8B, 0x92),
            Self::White => Color::from_rgb8(0xE9, 0xE5, 0xCE),
        }
    }

    pub const fn for_digit(digit: u8) -> Option<Self> {
        match digit {
            0 => Some(Self::Black),
            1 => Some(Self::Brown),
            2 => Some(Self::Red),
            3 => Some(Self::Orange),
            4 => Some(Self::Yellow),
            5 => Some(Self::Green),
            6 => Some(Self::Blue),
            7 => Some(Self::Violet),
            8 => Some(Self::Grey),
            9 => Some(Self::White),
            _ => None,
        }
    }

    pub const fn for_multiplier(exponent: i8) -> Option<Self> {
        match exponent {
            -3 => Some(Self::Pink),
            -2 => Some(Self::Silver),
            -1 => Some(Self::Gold),
            0 => Some(Self::Black),
            1 => Some(Self::Brown),
            2 => Some(Self::Red),
            3 => Some(Self::Orange),
            4 => Some(Self::Yellow),
            5 => Some(Self::Green),
            6 => Some(Self::Blue),
            7 => Some(Self::Violet),
            8 => Some(Self::Grey),
            9 => Some(Self::White),
            _ => None,
        }
    }

    pub const fn for_tolerance(tolerance: Tolerance) -> Option<Self> {
        match tolerance {
            Tolerance::Percent20 => None,
            Tolerance::Percent10 => Some(Self::Silver),
            Tolerance::Percent5 => Some(Self::Gold),
            Tolerance::Percent2 => Some(Self::Red),
            Tolerance::Percent1 => Some(Self::Brown),
            Tolerance::Percent0_5 => Some(Self::Green),
            Tolerance::Percent0_25 => Some(Self::Blue),
            Tolerance::Percent0_1 => Some(Self::Violet),
            Tolerance::Percent0_05 => Some(Self::Orange),
            Tolerance::Percent0_02 => Some(Self::Yellow),
            Tolerance::Percent0_01 => Some(Self::Grey),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResistorColorCode {
    pub bands: Vec<BandColor>,
    pub tolerance_has_no_band: bool,
    pub convention_label: &'static str,
    pub tolerance_note: Option<&'static str>,
    pub temperature_coefficient_note: Option<&'static str>,
}

/// Color-code result for any supported passive component. The legacy
/// `ResistorColorCode` name remains available for API compatibility.
pub type ComponentColorCode = ResistorColorCode;

impl ResistorColorCode {
    pub fn for_component(component: PreferredComponent, tolerance: Tolerance) -> Option<Self> {
        Self::for_kind(ComponentKind::Resistor, component, tolerance)
    }

    pub fn for_kind(
        kind: ComponentKind,
        component: PreferredComponent,
        tolerance: Tolerance,
    ) -> Option<Self> {
        Self::representations_for_kind(kind, component, tolerance)
            .into_iter()
            .next()
    }

    pub fn representations_for_kind(
        kind: ComponentKind,
        component: PreferredComponent,
        tolerance: Tolerance,
    ) -> Vec<Self> {
        Self::representations_for_kind_with_temperature_coefficient(
            kind, component, tolerance, None,
        )
    }

    pub fn representations_for_kind_with_temperature_coefficient(
        kind: ComponentKind,
        component: PreferredComponent,
        tolerance: Tolerance,
        temperature_coefficient: Option<TemperatureCoefficient>,
    ) -> Vec<Self> {
        let (decade_offset, convention_label) = match kind {
            ComponentKind::Resistor => (0, "IEC 60062, Ω"),
            ComponentKind::Capacitor => (12, "capacitor code, pF"),
            ComponentKind::Inductor => (6, "manufacturer convention, µH"),
        };
        let Some(encoded_decade) = component.decade.checked_add(decade_offset) else {
            return Vec::new();
        };
        let encoded_component = PreferredComponent {
            decade: encoded_decade,
            ..component
        };
        let tolerance_color = match kind {
            ComponentKind::Capacitor => capacitor_tolerance_color(tolerance),
            ComponentKind::Resistor | ComponentKind::Inductor => {
                BandColor::for_tolerance(tolerance)
            }
        };
        let tolerance_note = (kind == ComponentKind::Capacitor
            && tolerance != Tolerance::Percent20
            && tolerance_color.is_none())
        .then_some("selected tolerance has no capacitor color band");
        let temperature_coefficient_color = if kind == ComponentKind::Resistor {
            temperature_coefficient.and_then(temperature_coefficient_color)
        } else {
            None
        };
        let temperature_coefficient_note = (kind == ComponentKind::Resistor
            && temperature_coefficient.is_some()
            && temperature_coefficient_color.is_none())
        .then_some("selected TCR has no standardized color band");

        let mut representations = Vec::new();
        let primary_digit_count = encoded_component
            .number
            .significand
            .to_string()
            .len()
            .clamp(2, 3);
        for significant_digit_count in [primary_digit_count, 5 - primary_digit_count] {
            if let Some((digits, multiplier_exponent)) = adjusted_significand(
                encoded_component.number.significand,
                encoded_component.multiplier_exponent(),
                significant_digit_count,
            ) && let Some(mut bands) = digits
                .bytes()
                .map(|digit| BandColor::for_digit(digit - b'0'))
                .collect::<Option<Vec<_>>>()
                .and_then(|mut bands| {
                    bands.push(BandColor::for_multiplier(multiplier_exponent)?);
                    Some(bands)
                })
            {
                if let Some(color) = tolerance_color {
                    bands.push(color);
                }
                if let Some(color) = temperature_coefficient_color {
                    bands.push(color);
                }
                representations.push(Self {
                    bands,
                    tolerance_has_no_band: tolerance_color.is_none(),
                    convention_label,
                    tolerance_note,
                    temperature_coefficient_note,
                });
            }
        }

        representations
    }

    pub fn accessible_label(&self) -> String {
        let mut labels = self
            .bands
            .iter()
            .map(|band| band.name())
            .collect::<Vec<_>>();
        if let Some(note) = self.tolerance_note {
            labels.push(note);
        } else if self.tolerance_has_no_band {
            labels.push("no tolerance band");
        }
        if let Some(note) = self.temperature_coefficient_note {
            labels.push(note);
        }
        format!("{} ({})", labels.join(" – "), self.convention_label)
    }
}

fn adjusted_significand(
    mut significand: u16,
    mut multiplier_exponent: i8,
    desired_digits: usize,
) -> Option<(String, i8)> {
    let mut digits = significand.to_string();
    while digits.len() < desired_digits {
        significand = significand.checked_mul(10)?;
        multiplier_exponent = multiplier_exponent.checked_sub(1)?;
        digits = significand.to_string();
    }
    while digits.len() > desired_digits {
        if !significand.is_multiple_of(10) {
            return None;
        }
        significand /= 10;
        multiplier_exponent = multiplier_exponent.checked_add(1)?;
        digits = significand.to_string();
    }
    (digits.len() == desired_digits).then_some((digits, multiplier_exponent))
}

const fn capacitor_tolerance_color(tolerance: Tolerance) -> Option<BandColor> {
    match tolerance {
        Tolerance::Percent20 => None,
        Tolerance::Percent10 => Some(BandColor::Silver),
        Tolerance::Percent5 => Some(BandColor::Gold),
        Tolerance::Percent2 => Some(BandColor::Red),
        Tolerance::Percent1 => Some(BandColor::Brown),
        Tolerance::Percent0_5 => Some(BandColor::Green),
        Tolerance::Percent0_25
        | Tolerance::Percent0_1
        | Tolerance::Percent0_05
        | Tolerance::Percent0_02
        | Tolerance::Percent0_01 => None,
    }
}

const fn temperature_coefficient_color(
    temperature_coefficient: TemperatureCoefficient,
) -> Option<BandColor> {
    match temperature_coefficient {
        TemperatureCoefficient::Ppm1 => Some(BandColor::Grey),
        TemperatureCoefficient::Ppm2 => None,
        TemperatureCoefficient::Ppm5 => Some(BandColor::Violet),
        TemperatureCoefficient::Ppm10 => Some(BandColor::Blue),
        TemperatureCoefficient::Ppm15 => Some(BandColor::Orange),
        TemperatureCoefficient::Ppm25 => Some(BandColor::Yellow),
        TemperatureCoefficient::Ppm50 => Some(BandColor::Red),
        TemperatureCoefficient::Ppm100 => Some(BandColor::Brown),
        TemperatureCoefficient::Ppm250 => Some(BandColor::Black),
        TemperatureCoefficient::Other => None,
    }
}
