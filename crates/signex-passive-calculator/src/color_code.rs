use iced::Color;

use crate::domain::{ComponentKind, PreferredComponent, Tolerance};

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
        let (decade_offset, convention_label) = match kind {
            ComponentKind::Resistor => (0, "IEC 60062, Ω"),
            ComponentKind::Capacitor => (12, "capacitor code, pF"),
            ComponentKind::Inductor => (6, "manufacturer convention, µH"),
        };
        let encoded_component = PreferredComponent {
            decade: component.decade.checked_add(decade_offset)?,
            ..component
        };
        let digits = encoded_component.number.significand.to_string();
        let expected_digits = usize::from(component.number.decimal_places) + 1;
        if digits.len() != expected_digits {
            return None;
        }

        let mut bands = digits
            .bytes()
            .map(|digit| BandColor::for_digit(digit - b'0'))
            .collect::<Option<Vec<_>>>()?;
        bands.push(BandColor::for_multiplier(
            encoded_component.multiplier_exponent(),
        )?);

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
        if let Some(color) = tolerance_color {
            bands.push(color);
        }

        Some(Self {
            bands,
            tolerance_has_no_band: tolerance_color.is_none(),
            convention_label,
            tolerance_note,
        })
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
        format!("{} ({})", labels.join(" – "), self.convention_label)
    }
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
