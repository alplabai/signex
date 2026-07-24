/// Selects the frequency-domain result displayed or exported by the tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultDiagramKind {
    ImpedanceMagnitude,
    S11Db,
    S21Db,
}

impl ResultDiagramKind {
    /// Returns the user-facing diagram title.
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::ImpedanceMagnitude => "|Z| [Ω] / Frequency [MHz]",
            Self::S11Db => "|S11| [dB] / Frequency [MHz]",
            Self::S21Db => "|S21| [dB] / Frequency [MHz]",
        }
    }

    /// Returns the label used for exported diagram values.
    pub(crate) const fn value_label(self) -> &'static str {
        match self {
            Self::ImpedanceMagnitude => "|Z| [Ω]",
            Self::S11Db => "|S11| [dB]",
            Self::S21Db => "|S21| [dB]",
        }
    }

    /// Returns the default file name for this export kind.
    pub(crate) const fn file_name(self) -> &'static str {
        match self {
            Self::ImpedanceMagnitude => "smith_chart_impedance.csv",
            Self::S11Db => "smith_chart_s11.csv",
            Self::S21Db => "smith_chart_s21.csv",
        }
    }
}
