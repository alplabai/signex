use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GerberDisplayUnit {
    Inches,
    Mils,
    Millimetres,
}

impl GerberDisplayUnit {
    pub(super) const ALL: [Self; 3] = [Self::Inches, Self::Mils, Self::Millimetres];

    pub fn next(self) -> Self {
        match self {
            Self::Millimetres => Self::Mils,
            Self::Mils => Self::Inches,
            Self::Inches => Self::Millimetres,
        }
    }

    pub(super) fn value_from_millimetres(self, value: f64) -> f64 {
        match self {
            Self::Inches => value / 25.4,
            Self::Mils => value / 0.0254,
            Self::Millimetres => value,
        }
    }

    pub(super) fn decimal_places(self) -> usize {
        match self {
            Self::Inches | Self::Millimetres => 4,
            Self::Mils => 2,
        }
    }

    pub(super) fn suffix(self) -> &'static str {
        match self {
            Self::Inches => "in",
            Self::Mils => "mils",
            Self::Millimetres => "mm",
        }
    }

    pub(super) fn format_value(self, millimetres: f64, decimal_separator: &str) -> String {
        let value = self.value_from_millimetres(millimetres);
        let decimals = self.decimal_places();
        format!("{value:.decimals$}").replace('.', decimal_separator)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GerberCrosshairMode {
    None,
    #[default]
    Short,
    Full,
}

impl GerberCrosshairMode {
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Short,
            Self::Short => Self::Full,
            Self::Full => Self::None,
        }
    }
}

impl fmt::Display for GerberCrosshairMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::None => "none",
            Self::Short => "short",
            Self::Full => "full window",
        })
    }
}

impl fmt::Display for GerberDisplayUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inches => "Inches",
            Self::Mils => "Mils",
            Self::Millimetres => "Millimetres",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum GerberPageSize {
    #[default]
    FullSize,
    A4,
    A3,
    A2,
    A,
    B,
    C,
}

impl GerberPageSize {
    pub(super) const ALL: [Self; 7] = [
        Self::FullSize,
        Self::A4,
        Self::A3,
        Self::A2,
        Self::A,
        Self::B,
        Self::C,
    ];

    pub(super) fn dimensions_millimetres(self) -> Option<(f64, f64)> {
        match self {
            Self::FullSize => None,
            Self::A4 => Some((297.0, 210.0)),
            Self::A3 => Some((420.0, 297.0)),
            Self::A2 => Some((594.0, 420.0)),
            Self::A => Some((279.4, 215.9)),
            Self::B => Some((431.8, 279.4)),
            Self::C => Some((558.8, 431.8)),
        }
    }
}

impl fmt::Display for GerberPageSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FullSize => "Full size",
            Self::A4 => "A4",
            Self::A3 => "A3",
            Self::A2 => "A2",
            Self::A => "ANSI A",
            Self::B => "ANSI B",
            Self::C => "ANSI C",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GerberPrintLayout {
    pub page_size: GerberPageSize,
    pub bounds: Bounds,
}
