use std::fmt;

use super::domain::{ComponentKind, PreferredComponent, Tolerance};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemperatureCoefficient {
    Ppm1,
    Ppm2,
    Ppm5,
    Ppm10,
    Ppm15,
    Ppm25,
    Ppm50,
    Ppm100,
    Ppm250,
    Other,
}

impl TemperatureCoefficient {
    pub const ALL: [Self; 10] = [
        Self::Ppm1,
        Self::Ppm2,
        Self::Ppm5,
        Self::Ppm10,
        Self::Ppm15,
        Self::Ppm25,
        Self::Ppm50,
        Self::Ppm100,
        Self::Ppm250,
        Self::Other,
    ];

    const fn code(self) -> char {
        match self {
            Self::Ppm1 => 'K',
            Self::Ppm2 => 'L',
            Self::Ppm5 => 'M',
            Self::Ppm10 => 'N',
            Self::Ppm15 => 'P',
            Self::Ppm25 => 'Q',
            Self::Ppm50 => 'R',
            Self::Ppm100 => 'S',
            Self::Ppm250 => 'U',
            Self::Other => 'Z',
        }
    }

    const fn ppm_per_kelvin(self) -> Option<u16> {
        match self {
            Self::Ppm1 => Some(1),
            Self::Ppm2 => Some(2),
            Self::Ppm5 => Some(5),
            Self::Ppm10 => Some(10),
            Self::Ppm15 => Some(15),
            Self::Ppm25 => Some(25),
            Self::Ppm50 => Some(50),
            Self::Ppm100 => Some(100),
            Self::Ppm250 => Some(250),
            Self::Other => None,
        }
    }
}

impl fmt::Display for TemperatureCoefficient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ppm_per_kelvin() {
            Some(value) => write!(formatter, "{value} ppm/K"),
            None => formatter.write_str("Other"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RatedPower {
    Watts0_05,
    Watts0_063,
    Watts0_1,
    Watts0_125,
    Watts0_25,
    Watts0_5,
    Watts0_6,
    Watts0_63,
    Watts1,
    Watts2,
    Watts3,
    Watts5,
    Watts10,
}

impl RatedPower {
    pub const ALL: [Self; 13] = [
        Self::Watts0_05,
        Self::Watts0_063,
        Self::Watts0_1,
        Self::Watts0_125,
        Self::Watts0_25,
        Self::Watts0_5,
        Self::Watts0_6,
        Self::Watts0_63,
        Self::Watts1,
        Self::Watts2,
        Self::Watts3,
        Self::Watts5,
        Self::Watts10,
    ];

    const fn milliwatts(self) -> u16 {
        match self {
            Self::Watts0_05 => 50,
            Self::Watts0_063 => 63,
            Self::Watts0_1 => 100,
            Self::Watts0_125 => 125,
            Self::Watts0_25 => 250,
            Self::Watts0_5 => 500,
            Self::Watts0_6 => 600,
            Self::Watts0_63 => 630,
            Self::Watts1 => 1_000,
            Self::Watts2 => 2_000,
            Self::Watts3 => 3_000,
            Self::Watts5 => 5_000,
            Self::Watts10 => 10_000,
        }
    }

    fn decimal_watts(self) -> String {
        let milliwatts = self.milliwatts();
        let whole = milliwatts / 1_000;
        let remainder = milliwatts % 1_000;
        if remainder == 0 {
            return whole.to_string();
        }
        let fraction = format!("{remainder:03}").trim_end_matches('0').to_string();
        format!("{whole}.{fraction}")
    }

    fn code(self) -> String {
        self.decimal_watts().replace('.', "W")
    }
}

impl fmt::Display for RatedPower {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} W", self.decimal_watts())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RkmCode {
    value_code: String,
    tolerance_code: char,
    temperature_coefficient: Option<TemperatureCoefficient>,
    rated_power: Option<RatedPower>,
}

impl RkmCode {
    pub fn for_component(
        kind: ComponentKind,
        component: PreferredComponent,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            value_code: value_code(kind, component),
            tolerance_code: tolerance_code(tolerance),
            temperature_coefficient: None,
            rated_power: None,
        }
    }

    pub fn value_code(&self) -> &str {
        &self.value_code
    }

    pub fn with_temperature_coefficient(
        mut self,
        temperature_coefficient: TemperatureCoefficient,
    ) -> Self {
        self.temperature_coefficient = Some(temperature_coefficient);
        self
    }

    pub fn with_rated_power(mut self, rated_power: RatedPower) -> Self {
        self.rated_power = Some(rated_power);
        self
    }
}

impl fmt::Display for RkmCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.value_code, self.tolerance_code)?;
        if let Some(temperature_coefficient) = self.temperature_coefficient {
            write!(formatter, "{}", temperature_coefficient.code())?;
        }
        if let Some(rated_power) = self.rated_power {
            write!(formatter, " {}", rated_power.code())?;
        }
        Ok(())
    }
}

fn value_code(kind: ComponentKind, component: PreferredComponent) -> String {
    let value_decade = component.number.significand.to_string().len() as i16 - 1
        + i16::from(component.decade)
        - i16::from(component.number.decimal_places);
    let (unit_exponent, separator) = match kind {
        ComponentKind::Resistor => resistance_scale(value_decade),
        ComponentKind::Capacitor => capacitance_scale(value_decade),
        ComponentKind::Inductor => {
            let (microhenry_exponent, separator) = resistance_scale(value_decade + 6);
            (microhenry_exponent - 6, separator)
        }
    };
    format_scaled_value(component, unit_exponent, separator)
}

fn format_scaled_value(
    component: PreferredComponent,
    unit_exponent: i16,
    separator: char,
) -> String {
    let digits = component.number.significand.to_string();
    let power =
        i16::from(component.decade) - i16::from(component.number.decimal_places) - unit_exponent;
    if power >= 0 {
        let mut result = digits;
        result.extend(std::iter::repeat_n('0', power as usize));
        result.push(separator);
        return result;
    }

    let separator_index = digits.len() as i16 + power;
    if separator_index > 0 {
        let separator_index = separator_index as usize;
        return format!(
            "{}{separator}{}",
            &digits[..separator_index],
            &digits[separator_index..]
        );
    }

    let mut result = separator.to_string();
    result.extend(std::iter::repeat_n('0', -separator_index as usize));
    result.push_str(&digits);
    result
}

fn resistance_scale(decade: i16) -> (i16, char) {
    match decade {
        12.. => (12, 'T'),
        9..=11 => (9, 'G'),
        6..=8 => (6, 'M'),
        3..=5 => (3, 'K'),
        -1..=2 => (0, 'R'),
        ..=-2 => (-3, 'L'),
    }
}

fn capacitance_scale(decade: i16) -> (i16, char) {
    match decade {
        0.. => (0, 'F'),
        -3..=-1 => (-3, 'm'),
        -6..=-4 => (-6, 'u'),
        -9..=-7 => (-9, 'n'),
        ..=-10 => (-12, 'p'),
    }
}

const fn tolerance_code(tolerance: Tolerance) -> char {
    match tolerance {
        Tolerance::Percent20 => 'M',
        Tolerance::Percent10 => 'K',
        Tolerance::Percent5 => 'J',
        Tolerance::Percent2 => 'G',
        Tolerance::Percent1 => 'F',
        Tolerance::Percent0_5 => 'D',
        Tolerance::Percent0_25 => 'C',
        Tolerance::Percent0_1 => 'B',
        Tolerance::Percent0_05 => 'W',
        Tolerance::Percent0_02 => 'P',
        Tolerance::Percent0_01 => 'L',
    }
}
