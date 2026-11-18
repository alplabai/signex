//! Errors, and the guard every kernel call goes through.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Why a modelling or tessellation operation did not produce a result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McadError {
    /// The caller's input was refused before the kernel ran.
    #[error("{operation}: {reason}")]
    InvalidInput {
        operation: &'static str,
        reason: String,
    },
    /// The kernel reported that the operation failed.
    #[error("{operation} failed in the kernel: {message}")]
    Kernel {
        operation: &'static str,
        message: String,
    },
    /// The kernel stopped with a panic; it was contained here (native
    /// targets only; a `wasm32` build aborts on panic).
    #[error("{operation} stopped in the kernel: {message}")]
    KernelPanicked {
        operation: &'static str,
        message: String,
    },
    /// A face could not be turned into triangles.
    #[error("face {face} cannot be tessellated: {reason}")]
    Tessellation { face: usize, reason: String },
    /// A solid holds geometry the STEP writer cannot describe exactly.
    #[error("STEP export: {reason}")]
    StepExport { reason: String },
}

impl McadError {
    pub(crate) fn invalid(operation: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            operation,
            reason: reason.into(),
        }
    }

    /// Maps a kernel error of `operation` into this type.
    pub(crate) fn kernel(operation: &'static str) -> impl FnOnce(solidcore::SolidError) -> Self {
        move |error| Self::Kernel {
            operation,
            message: error.to_string(),
        }
    }
}

/// Runs one kernel call: a kernel error becomes [`McadError::Kernel`], a
/// panic becomes [`McadError::KernelPanicked`] on native targets (see
/// [`contained`]).
pub(crate) fn kernel_call<T>(
    operation: &'static str,
    call: impl FnOnce() -> solidcore::Result<T>,
) -> Result<T, McadError> {
    contained(operation, || call().map_err(McadError::kernel(operation)))
}

/// Runs `call` and turns a panic inside it into [`McadError::KernelPanicked`].
///
/// Native targets only: `catch_unwind` needs unwinding, and `wasm32` builds
/// abort on panic, so there a kernel panic still ends the program.
///
/// Every public operation builds its own kernel state, so nothing a panic
/// left half-done is reused afterwards.
pub(crate) fn contained<T>(
    operation: &'static str,
    call: impl FnOnce() -> Result<T, McadError>,
) -> Result<T, McadError> {
    catch_unwind(AssertUnwindSafe(call)).unwrap_or_else(|payload| {
        Err(McadError::KernelPanicked {
            operation,
            message: panic_message(payload.as_ref()),
        })
    })
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "no message".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kernel_error_is_a_value() {
        let result = kernel_call("cylinder", || {
            let mut kernel = solidcore::Kernel::default();
            solidcore::primitive::api_primitives::create_cylinder(
                &mut kernel,
                solidcore::Point3::new(0.0, 0.0, 0.0),
                -1.0,
                1.0,
            )
        });
        assert!(
            matches!(
                result,
                Err(McadError::Kernel {
                    operation: "cylinder",
                    ..
                })
            ),
            "{result:?}"
        );
    }

    #[test]
    fn a_kernel_panic_is_contained() {
        let result: Result<(), McadError> = kernel_call("panicking call", || {
            std::panic::panic_any("kernel invariant broken")
        });
        assert_eq!(
            result,
            Err(McadError::KernelPanicked {
                operation: "panicking call",
                message: "kernel invariant broken".to_owned(),
            })
        );
    }
}
