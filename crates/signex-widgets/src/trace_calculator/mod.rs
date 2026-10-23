//! IPC-2221 PCB trace width calculator: an iced-free domain plus the
//! dialog body that drives it.

pub mod control;
pub mod diagram;
pub mod domain;
pub mod results;

pub use control::{
    TraceCalculatorControl, TraceCalculatorError, TraceCalculatorMessage, format_number,
};
pub use domain::{
    AmbientUnit, CHART_RANGE, CalculationError, ChartRange, InputField, LayerChart, LayerKind,
    LayerResult, LengthUnit, RangeWarning, RiseUnit, ThicknessUnit, TraceCalculation, TraceInput,
    WidthUnit, calculate,
};
