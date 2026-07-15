use crate::{CustomInterpolation, ScalarUnit, TransformerModel};

use super::reference_link::ReferenceLink;
use crate::tool::equations::FormulaReference;

pub(crate) const DEFAULT_S_PARAMETER_TEXT: &str =
    "# MHz S MA R 50\n1000 0.1 0 2.0 0 0.02 0 0.1 180";
pub(crate) const ONLINE_SMITH_CHART_BASE_URL: &str = "https://onlinesmithchart.com/";
pub(crate) const SHORTED_STUB_WARNING: &str = "A shorted shunt stub starts at zero impedance when its length is zero. Confirm that this topology is intended before continuing.";
pub(crate) const FREQUENCY_UNITS: [ScalarUnit; 5] = [
    ScalarUnit::Hertz,
    ScalarUnit::KiloHertz,
    ScalarUnit::MegaHertz,
    ScalarUnit::GigaHertz,
    ScalarUnit::TeraHertz,
];
pub(crate) const CUSTOM_INTERPOLATIONS: [CustomInterpolation; 2] = [
    CustomInterpolation::Linear,
    CustomInterpolation::SampleAndHold,
];
pub(crate) const TRANSFORMER_MODELS: [TransformerModel; 2] =
    [TransformerModel::CoupledInductor, TransformerModel::Ideal];
pub(crate) const TUTORIAL_REFERENCE_LINKS: [ReferenceLink; 5] = [
    ReferenceLink::TutorialS1p,
    ReferenceLink::TutorialS2p,
    ReferenceLink::TutorialNoise,
    ReferenceLink::TutorialStability,
    ReferenceLink::StabilityReference,
];
pub(crate) const FORMULA_REFERENCE_LINKS: [ReferenceLink; 4] = [
    ReferenceLink::GainDesignReference,
    ReferenceLink::NoiseDesignReference,
    ReferenceLink::NoiseFormulaReference,
    ReferenceLink::StabilityReference,
];
pub(crate) const FORMULA_REFERENCES: [FormulaReference; 7] = [
    FormulaReference {
        item: "Transformer",
        latex: r"L_m = k\sqrt{L_1L_2}",
        equation: "Lm = k * sqrt(L1 * L2); equivalent model uses (L1 - Lm), Lm, and (L2 - Lm).",
        notes: "Simple three-inductor equivalent model used by the coupled-transformer row.",
    },
    FormulaReference {
        item: "Transmission Lines",
        latex: r"Z_{in}=Z_0\frac{Z_L+jZ_0\tan(\beta l)}{Z_0+jZ_L\tan(\beta l)},\quad \beta=\frac{2\pi}{\lambda}",
        equation: "Zin = Z0 * (ZL + j Z0 tan(beta*l)) / (Z0 + j ZL tan(beta*l)); beta = 2*pi/lambda.",
        notes: "Matches the distributed line impedance transform used by line and length-unit rows.",
    },
    FormulaReference {
        item: "Stub",
        latex: r"Z=-j\frac{Z_0}{\tan(\beta l)}",
        equation: "Z = -j Z0 / tan(beta*l).",
        notes: "Adds the open-stub impedance in parallel with the current network.",
    },
    FormulaReference {
        item: "Shorted Stub",
        latex: r"Z=-j\frac{Z_0}{\tan(\beta l+\pi/2)},\quad \cot(x+\pi/2)=-\tan(x)",
        equation: "Z = -j Z0 / tan(beta*l + pi/2); cot(x + pi/2) = -tan(x).",
        notes: "Models a short-circuited shunt stub using the cotangent form.",
    },
    FormulaReference {
        item: ".s2p gain equations",
        latex: r"G_{TU}=G_S\left|S_{21}\right|^2G_L,\quad Z=Z_0\frac{1+\Gamma}{1-\Gamma}",
        equation: "G_TU = G_S * |S21|^2 * G_L; gain-circle centers/radii are solved in the reflection plane.",
        notes: "Cs and Rl are reflection coefficients; translate with Z = Z0 * (1 + Gamma) / (1 - Gamma).",
    },
    FormulaReference {
        item: "Noise Figure Circles",
        latex: r"N=\frac{F-F_{min}}{4R_n/Z_0}\left|1+\Gamma_{opt}\right|^2,\quad C_F=\frac{\Gamma_{opt}}{N+1}",
        equation: "N = (F - Fmin) / (4 Rn / Z0) * |1 + Gamma_opt|^2; center = Gamma_opt / (N + 1).",
        notes: "The radius uses sqrt(N * (N + 1 - |Gamma_opt|^2)) / (N + 1).",
    },
    FormulaReference {
        item: "Stability Circles",
        latex: r"\Delta=S_{11}S_{22}-S_{12}S_{21}",
        equation: "Delta = S11*S22 - S12*S21; source/load centers and radii are derived from S-parameters.",
        notes: "Defines the source and load stability-circle overlay.",
    },
];
