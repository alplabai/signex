use crate::transmission_line_calculator::{AbcdMatrix, Complex, SParameterMatrix, TwoPortError};
use serde::{Deserialize, Serialize};

/// Couples a physical ABCD matrix with the reference impedances at its outer ports.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TwoPortNetwork {
    pub abcd: AbcdMatrix,
    pub port_1_reference_impedance: Complex,
    pub port_2_reference_impedance: Complex,
}

impl TwoPortNetwork {
    /// Creates a two-port network from an ABCD matrix and its outer port references.
    pub const fn from_abcd(
        abcd: AbcdMatrix,
        port_1_reference_impedance: Complex,
        port_2_reference_impedance: Complex,
    ) -> Self {
        Self {
            abcd,
            port_1_reference_impedance,
            port_2_reference_impedance,
        }
    }

    /// Creates a two-port network from power-wave S-parameters.
    pub fn from_s_parameters(
        s_parameters: SParameterMatrix,
        port_1_reference_impedance: Complex,
        port_2_reference_impedance: Complex,
    ) -> Result<Self, TwoPortError> {
        Ok(Self::from_abcd(
            s_parameters.to_abcd(port_1_reference_impedance, port_2_reference_impedance)?,
            port_1_reference_impedance,
            port_2_reference_impedance,
        ))
    }

    /// Creates an ideal through network for the supplied reference impedance.
    pub const fn through(reference_impedance: Complex) -> Self {
        Self::from_abcd(
            AbcdMatrix::identity(),
            reference_impedance,
            reference_impedance,
        )
    }

    /// Cascades this source-side network with the supplied load-side network.
    ///
    /// ABCD matrices represent physical voltage and current, so differing
    /// internal wave-reference impedances do not require an extra mismatch
    /// matrix. The resulting network keeps only the two outer references.
    pub fn cascade(self, load_side: Self) -> Self {
        Self::from_abcd(
            self.abcd.cascade(load_side.abcd),
            self.port_1_reference_impedance,
            load_side.port_2_reference_impedance,
        )
    }

    /// Returns the power-wave S-parameters at the network's outer references.
    pub fn s_parameters(self) -> Result<SParameterMatrix, TwoPortError> {
        self.abcd.to_s_parameters(
            self.port_1_reference_impedance,
            self.port_2_reference_impedance,
        )
    }

    /// Transforms a load impedance to the network's input impedance.
    pub fn input_impedance(self, load_impedance: Complex) -> Result<Complex, TwoPortError> {
        self.abcd.input_impedance(load_impedance)
    }
}
