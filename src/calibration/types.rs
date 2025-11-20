use serde::{Deserialize, Serialize};
// Note: HashMap removed as param_map is no longer used
use crate::calibration::config::OptimizationConfig;
use std::any::Any;

/// Minimal market data structure with only essential fields for surface calibration
#[derive(Debug, Clone)]
pub struct MarketDataRow {
    /// Option type: "call" or "put"
    pub option_type: String,
    /// Strike price
    pub strike_price: f64,
    /// Underlying asset price
    pub underlying_price: f64,
    /// Time to expiration in years
    pub years_to_exp: f64,
    /// Market implied volatility (as decimal, e.g., 0.25 for 25%)
    pub market_iv: f64,
    /// Option vega (for weighting)
    pub vega: f64,
    /// Expiration timestamp (for grouping by expiry)
    pub expiration: i64,
}

/// Fixed parameters that are not calibrated by the optimizer
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FixedParameters {
    pub r: f64,
    pub q: f64,
}

impl Default for FixedParameters {
    fn default() -> Self {
        Self { r: 0.02, q: 0.0 }
    }
}

/// Model calibrator trait for parameter optimization
pub trait ModelCalibrator: Send + Sync {
    /// Returns the name of the model (e.g., "svi")
    fn model_name(&self) -> &str;

    /// How many parameters are in the model's optimization vector
    fn param_count(&self) -> usize;

    /// Returns the vector of (min, max) bounds for each parameter
    fn param_bounds(&self) -> &[(f64, f64)];

    /// Given a parameter vector `x` and data, returns the objective value
    fn evaluate_objective(&self, x: &[f64], data: &[MarketDataRow]) -> f64;

    // Note: relaxed_param_bounds and relaxed_evaluate_objective removed
    // as they were redundant with param_bounds and evaluate_objective

    // Note: create_param_map removed as param_map is no longer returned from calibration API

    /// Price options using the calibrated parameters
    fn price_options(
        &self,
        market_data: &[MarketDataRow],
        best_params: &[f64],
        config: &OptimizationConfig,
    ) -> Vec<PricingResult>;

    /// Returns parameter names in the order they appear in the optimization vector
    fn param_names(&self) -> Vec<&str>;

    /// Set the previous solution for temporal regularization
    fn set_prev_solution(&mut self, _prev_solution: Vec<f64>) {
        // Default implementation does nothing
    }

    /// Set the lambda parameter for temporal regularization
    fn set_temporal_reg_lambda(&mut self, _lambda: f64) {
        // Default implementation does nothing
    }

    /// Expand internal parameter bounds if parameters are near current bounds.
    /// Returns true if any bound was adjusted.
    fn expand_bounds_if_needed(
        &mut self,
        _params: &[f64],
        _proximity_threshold: f64,
        _expansion_factor: f64,
    ) -> bool {
        false
    }

    /// Support for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Lightweight struct to hold the essential pricing results for each option
#[derive(Debug, Clone)]
pub struct PricingResult {
    /// Option type: "call" or "put"
    pub option_type: String,
    /// Strike price
    pub strike_price: f64,
    /// Underlying asset price
    pub underlying_price: f64,
    /// Time to expiration in years
    pub years_to_exp: f64,
    /// Model option price
    pub model_price: f64,
    /// Model implied volatility (as decimal)
    pub model_iv: f64,
}
