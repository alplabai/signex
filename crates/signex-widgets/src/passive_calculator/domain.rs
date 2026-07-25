use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Resistor,
    Capacitor,
    Inductor,
}

impl ComponentKind {
    pub const ALL: [Self; 3] = [Self::Resistor, Self::Capacitor, Self::Inductor];

    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Resistor => "R",
            Self::Capacitor => "C",
            Self::Inductor => "L",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Resistor => 0,
            Self::Capacitor => 1,
            Self::Inductor => 2,
        }
    }

    pub const fn quantity_name(self) -> &'static str {
        match self {
            Self::Resistor => "Resistance",
            Self::Capacitor => "Capacitance",
            Self::Inductor => "Inductance",
        }
    }

    pub const fn component_name(self) -> &'static str {
        match self {
            Self::Resistor => "Resistor",
            Self::Capacitor => "Capacitor",
            Self::Inductor => "Inductor",
        }
    }

    pub const fn unit_symbol(self) -> &'static str {
        match self {
            Self::Resistor => "Ω",
            Self::Capacitor => "F",
            Self::Inductor => "H",
        }
    }
}

impl fmt::Display for ComponentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.component_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ESeries {
    E3,
    E6,
    E12,
    E24,
    E48,
    E96,
    E192,
}

impl ESeries {
    pub const ALL: [Self; 7] = [
        Self::E3,
        Self::E6,
        Self::E12,
        Self::E24,
        Self::E48,
        Self::E96,
        Self::E192,
    ];

    pub const fn values(self) -> &'static [u16] {
        match self {
            Self::E3 => &E3_VALUES,
            Self::E6 => &E6_VALUES,
            Self::E12 => &E12_VALUES,
            Self::E24 => &E24_VALUES,
            Self::E48 => &E48_VALUES,
            Self::E96 => &E96_VALUES,
            Self::E192 => &E192_VALUES,
        }
    }

    pub const fn decimal_places(self) -> u8 {
        match self {
            Self::E3 | Self::E6 | Self::E12 | Self::E24 => 1,
            Self::E48 | Self::E96 | Self::E192 => 2,
        }
    }

    pub fn preferred_numbers(self) -> impl Iterator<Item = PreferredNumber> {
        let decimal_places = self.decimal_places();
        self.values()
            .iter()
            .copied()
            .map(move |significand| PreferredNumber {
                significand,
                decimal_places,
            })
    }
}

impl fmt::Display for ESeries {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::E3 => "E3",
            Self::E6 => "E6",
            Self::E12 => "E12",
            Self::E24 => "E24",
            Self::E48 => "E48",
            Self::E96 => "E96",
            Self::E192 => "E192",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreferredNumber {
    pub significand: u16,
    pub decimal_places: u8,
}

impl PreferredNumber {
    pub fn normalized(self) -> f64 {
        f64::from(self.significand) / 10_f64.powi(i32::from(self.decimal_places))
    }
}

impl fmt::Display for PreferredNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.decimal_places == 0 {
            return write!(formatter, "{}", self.significand);
        }

        let decimal_places = usize::from(self.decimal_places);
        let digits = format!("{:0width$}", self.significand, width = decimal_places + 1);
        let separator_index = digits.len() - decimal_places;
        write!(
            formatter,
            "{}.{}",
            &digits[..separator_index],
            &digits[separator_index..]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreferredComponent {
    pub number: PreferredNumber,
    /// Base-10 exponent of the normalized number. `4.7 kΩ` is 4.7 × 10³,
    /// therefore its decade is 3.
    pub decade: i8,
}

impl PreferredComponent {
    pub fn value(self) -> f64 {
        self.number.normalized() * 10_f64.powi(i32::from(self.decade))
    }

    pub fn multiplier_exponent(self) -> i8 {
        self.decade - self.number.decimal_places as i8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SiPrefix {
    Pico,
    Nano,
    Micro,
    Milli,
    None,
    Kilo,
    Mega,
    Giga,
}

impl SiPrefix {
    pub const RESISTOR: [Self; 5] = [Self::Milli, Self::None, Self::Kilo, Self::Mega, Self::Giga];
    pub const REACTIVE: [Self; 8] = [
        Self::Pico,
        Self::Nano,
        Self::Micro,
        Self::Milli,
        Self::None,
        Self::Kilo,
        Self::Mega,
        Self::Giga,
    ];

    pub const fn for_kind(kind: ComponentKind) -> &'static [Self] {
        match kind {
            ComponentKind::Resistor => &Self::RESISTOR,
            ComponentKind::Capacitor | ComponentKind::Inductor => &Self::REACTIVE,
        }
    }

    pub const fn exponent(self) -> i8 {
        match self {
            Self::Pico => -12,
            Self::Nano => -9,
            Self::Micro => -6,
            Self::Milli => -3,
            Self::None => 0,
            Self::Kilo => 3,
            Self::Mega => 6,
            Self::Giga => 9,
        }
    }

    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Pico => "p",
            Self::Nano => "n",
            Self::Micro => "µ",
            Self::Milli => "m",
            Self::None => "",
            Self::Kilo => "k",
            Self::Mega => "M",
            Self::Giga => "G",
        }
    }

    pub fn multiplier(self) -> f64 {
        10_f64.powi(i32::from(self.exponent()))
    }

    pub fn unit(self, kind: ComponentKind) -> String {
        format!("{}{}", self.symbol(), kind.unit_symbol())
    }
}

impl fmt::Display for SiPrefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pico => "p",
            Self::Nano => "n",
            Self::Micro => "µ",
            Self::Milli => "m",
            Self::None => "—",
            Self::Kilo => "k",
            Self::Mega => "M",
            Self::Giga => "G",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tolerance {
    Percent20,
    Percent10,
    Percent5,
    Percent2,
    Percent1,
    Percent0_5,
    Percent0_25,
    Percent0_1,
    Percent0_05,
    Percent0_02,
    Percent0_01,
}

impl Tolerance {
    pub const ALL: [Self; 11] = [
        Self::Percent20,
        Self::Percent10,
        Self::Percent5,
        Self::Percent2,
        Self::Percent1,
        Self::Percent0_5,
        Self::Percent0_25,
        Self::Percent0_1,
        Self::Percent0_05,
        Self::Percent0_02,
        Self::Percent0_01,
    ];

    pub const fn fraction(self) -> f64 {
        match self {
            Self::Percent20 => 0.20,
            Self::Percent10 => 0.10,
            Self::Percent5 => 0.05,
            Self::Percent2 => 0.02,
            Self::Percent1 => 0.01,
            Self::Percent0_5 => 0.005,
            Self::Percent0_25 => 0.0025,
            Self::Percent0_1 => 0.001,
            Self::Percent0_05 => 0.0005,
            Self::Percent0_02 => 0.0002,
            Self::Percent0_01 => 0.0001,
        }
    }

    pub const fn percent_label(self) -> &'static str {
        match self {
            Self::Percent20 => "±20%",
            Self::Percent10 => "±10%",
            Self::Percent5 => "±5%",
            Self::Percent2 => "±2%",
            Self::Percent1 => "±1%",
            Self::Percent0_5 => "±0.5%",
            Self::Percent0_25 => "±0.25%",
            Self::Percent0_1 => "±0.1%",
            Self::Percent0_05 => "±0.05%",
            Self::Percent0_02 => "±0.02%",
            Self::Percent0_01 => "±0.01%",
        }
    }
}

impl fmt::Display for Tolerance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.percent_label())
    }
}

const E3_VALUES: [u16; 3] = [10, 22, 47];
const E6_VALUES: [u16; 6] = [10, 15, 22, 33, 47, 68];
const E12_VALUES: [u16; 12] = [10, 12, 15, 18, 22, 27, 33, 39, 47, 56, 68, 82];
const E24_VALUES: [u16; 24] = [
    10, 11, 12, 13, 15, 16, 18, 20, 22, 24, 27, 30, 33, 36, 39, 43, 47, 51, 56, 62, 68, 75, 82, 91,
];
const E48_VALUES: [u16; 48] = [
    100, 105, 110, 115, 121, 127, 133, 140, 147, 154, 162, 169, 178, 187, 196, 205, 215, 226, 237,
    249, 261, 274, 287, 301, 316, 332, 348, 365, 383, 402, 422, 442, 464, 487, 511, 536, 562, 590,
    619, 649, 681, 715, 750, 787, 825, 866, 909, 953,
];
const E96_VALUES: [u16; 96] = [
    100, 102, 105, 107, 110, 113, 115, 118, 121, 124, 127, 130, 133, 137, 140, 143, 147, 150, 154,
    158, 162, 165, 169, 174, 178, 182, 187, 191, 196, 200, 205, 210, 215, 221, 226, 232, 237, 243,
    249, 255, 261, 267, 274, 280, 287, 294, 301, 309, 316, 324, 332, 340, 348, 357, 365, 374, 383,
    392, 402, 412, 422, 432, 442, 453, 464, 475, 487, 499, 511, 523, 536, 549, 562, 576, 590, 604,
    619, 634, 649, 665, 681, 698, 715, 732, 750, 768, 787, 806, 825, 845, 866, 887, 909, 931, 953,
    976,
];
const E192_VALUES: [u16; 192] = [
    100, 101, 102, 104, 105, 106, 107, 109, 110, 111, 113, 114, 115, 117, 118, 120, 121, 123, 124,
    126, 127, 129, 130, 132, 133, 135, 137, 138, 140, 142, 143, 145, 147, 149, 150, 152, 154, 156,
    158, 160, 162, 164, 165, 167, 169, 172, 174, 176, 178, 180, 182, 184, 187, 189, 191, 193, 196,
    198, 200, 203, 205, 208, 210, 213, 215, 218, 221, 223, 226, 229, 232, 234, 237, 240, 243, 246,
    249, 252, 255, 258, 261, 264, 267, 271, 274, 277, 280, 284, 287, 291, 294, 298, 301, 305, 309,
    312, 316, 320, 324, 328, 332, 336, 340, 344, 348, 352, 357, 361, 365, 370, 374, 379, 383, 388,
    392, 397, 402, 407, 412, 417, 422, 427, 432, 437, 442, 448, 453, 459, 464, 470, 475, 481, 487,
    493, 499, 505, 511, 517, 523, 530, 536, 542, 549, 556, 562, 569, 576, 583, 590, 597, 604, 612,
    619, 626, 634, 642, 649, 657, 665, 673, 681, 690, 698, 706, 715, 723, 732, 741, 750, 759, 768,
    777, 787, 796, 806, 816, 825, 835, 845, 856, 866, 876, 887, 898, 909, 920, 931, 942, 953, 965,
    976, 988,
];
