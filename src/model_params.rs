//! Model-specific parameter containers used to tweak calibrators without hard-coding
//! constants in the model implementation. Each model should provide its own struct
//! that implements the [`ModelParams`] trait so that the calibration pipeline can
//! pass arbitrary parameters down to the calibrator in a type-erased fashion.

use serde::{Deserialize, Serialize};
use std::any::Any;

/// Marker trait for type-erased parameter structs.
///
/// The trait is deliberately minimal: it only provides a safe down-casting hook
/// via `as_any`.  This keeps the object safe and avoids imposing additional
/// requirements on concrete parameter types.
pub trait ModelParams: Send + Sync + std::fmt::Debug {
    /// Returns the boxed value as `&dyn Any` so that callers can attempt a
    /// concrete `downcast_ref::<T>()` when the concrete type is known.
    fn as_any(&self) -> &dyn Any;
}

/// Parameters that influence the SVI calibrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SviModelParams {
    /// Exponential weight multiplier for ATM options in the objective function.
    ///
    /// The weight applied to an observation is computed as `exp(-atm_boost_factor
    /// * |k|)` where `k` is log-moneyness.  A higher value therefore increases
    ///   the relative importance of points close to ATM.
    pub atm_boost_factor: f64,

    /// Whether to multiply the objective weight by option vega.  Setting this to
    /// `false` makes every strike contribute equally (after ATM weighting).
    pub use_vega_weighting: bool,
}

impl Default for SviModelParams {
    fn default() -> Self {
        Self {
            atm_boost_factor: 25.0,
            use_vega_weighting: true,
        }
    }
}

impl ModelParams for SviModelParams {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
