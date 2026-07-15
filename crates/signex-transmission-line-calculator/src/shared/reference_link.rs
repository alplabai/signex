#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceLink {
    AuthorHome,
    OnlineSmithChart,
    OnlineCircuitSolver,
    Trgmc,
    Github,
    TutorialS1p,
    TutorialS2p,
    TutorialNoise,
    TutorialStability,
    StabilityReference,
    GainDesignReference,
    NoiseDesignReference,
    NoiseFormulaReference,
}

impl ReferenceLink {
    pub fn label(self) -> &'static str {
        match self {
            Self::AuthorHome => "Author home",
            Self::OnlineSmithChart => "OnlineSmithChart",
            Self::OnlineCircuitSolver => "OnlineCircuitSolver",
            Self::Trgmc => "TRGMC",
            Self::Github => "GitHub source",
            Self::TutorialS1p => "S1P tutorial",
            Self::TutorialS2p => "S2P tutorial",
            Self::TutorialNoise => "Noise tutorial",
            Self::TutorialStability => "Stability tutorial",
            Self::StabilityReference => "Stability reference",
            Self::GainDesignReference => "Gain design reference",
            Self::NoiseDesignReference => "Noise design reference",
            Self::NoiseFormulaReference => "Noise formula reference",
        }
    }

    pub fn url(self) -> &'static str {
        match self {
            Self::AuthorHome => "https://www.will-kelsey.com",
            Self::OnlineSmithChart => "https://onlinesmithchart.com",
            Self::OnlineCircuitSolver => "https://onlinecircuitsolver.com",
            Self::Trgmc => "https://trgmc.net",
            Self::Github => "https://github.com/28raining/smith-chart",
            Self::TutorialS1p => {
                "https://github.com/28raining/smith-chart/blob/main/tutorials/s1p.md"
            }
            Self::TutorialS2p => {
                "https://github.com/28raining/smith-chart/blob/main/tutorials/s2p.md"
            }
            Self::TutorialNoise => {
                "https://github.com/28raining/smith-chart/blob/main/tutorials/noise.md"
            }
            Self::TutorialStability => {
                "https://github.com/28raining/smith-chart/blob/main/tutorials/stability.md"
            }
            Self::StabilityReference => {
                "https://www.allaboutcircuits.com/technical-articles/learn-about-unconditional-stability-and-potential-instability-in-rf-amplifier-design/"
            }
            Self::GainDesignReference => {
                "https://www.allaboutcircuits.com/technical-articles/designing-a-unilateral-rf-amplifier-for-a-specified-gain"
            }
            Self::NoiseDesignReference => {
                "https://www.allaboutcircuits.com/technical-articles/learn-about-designing-unilateral-low-noise-amplifiers/"
            }
            Self::NoiseFormulaReference => {
                "https://homepages.uc.edu/~ferendam/Courses/EE_611/Amplifier/NFC.html"
            }
        }
    }
}
