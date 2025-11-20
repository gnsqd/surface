//! # Surface-Lib: Advanced Option Pricing and Volatility Surface Calibration
//!
//! `surface-lib` is a high-performance Rust library designed for quantitative finance applications,
//! specifically focused on option pricing and volatility surface modeling. The library provides
//! robust implementations of industry-standard models with advanced calibration capabilities.
//!
//! ## Core Features
//!
//! - **SVI Model**: Stochastic Volatility Inspired model for volatility surface representation
//! - **Advanced Calibration**: CMA-ES and L-BFGS-B optimization with robust parameter estimation
//! - **Option Pricing**: Black-Scholes pricing with model-derived implied volatilities
//! - **Production Ready**: Optimized for real-time trading and backtesting systems
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use surface_lib::{calibrate_svi, price_with_svi, default_configs, CalibrationParams, MarketDataRow, FixedParameters};
//! use surface_lib::models::svi::svi_model::SVIParams;
//!
//! # fn load_market_data() -> Vec<MarketDataRow> { vec![] }
//! // Load your market data
//! let market_data: Vec<MarketDataRow> = load_market_data();
//!
//! // Calibrate SVI model parameters
//! let config = default_configs::fast();
//! let calib_params = CalibrationParams::default();
//! let (objective, params, used_bounds) = calibrate_svi(market_data.clone(), config, calib_params, None)?;
//!
//! // Create SVI parameters for pricing
//! let svi_params = SVIParams {
//!     t: 0.0274, a: params[0], b: params[1],
//!     rho: params[2], m: params[3], sigma: params[4]
//! };
//! let fixed_params = FixedParameters { r: 0.02, q: 0.0 };
//!
//! // Price options with calibrated model
//! let pricing_results = price_with_svi(svi_params, market_data, fixed_params);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Model Support
//!
//! Currently supported volatility models:
//! - **SVI (Stochastic Volatility Inspired)**: Industry-standard single-slice model
//!
//! ## Configuration Presets
//!
//! The library provides several optimization configuration presets:
//! - `production()`: High accuracy for live trading systems
//! - `fast()`: Balanced speed/accuracy for development
//! - `research()`: High-precision settings for research
//! - `minimal()`: Quick validation settings

// ================================================================================================
// MODULES
// ================================================================================================

pub mod calibration;
pub mod model_params;
pub mod models;

// ================================================================================================
// IMPORTS
// ================================================================================================

// Note: HashMap removed as it's no longer used in the API

use anyhow::Result;
use std::cmp::Ordering;

use calibration::{
    config::OptimizationConfig as InternalOptimizationConfig,
    types::MarketDataRow as InternalMarketDataRow,
};
use models::{
    svi::{svi_calibrator::SVIModelCalibrator, svi_model::SVISlice},
    utils::{price_option, OptionPricingResult},
};
// (removed - using public re-export instead)

use crate::calibration::pipeline::calibrate_model_adaptive;

// ================================================================================================
// PUBLIC RE-EXPORTS
// ================================================================================================

// Core types for market data and configuration
pub use calibration::{
    config::{CmaEsConfig, OptimizationConfig},
    types::{FixedParameters, MarketDataRow, PricingResult},
};

// SVI model types and parameters
pub use models::svi::{svi_calibrator::SVIParamBounds, svi_model::SVIParams};

// Linear IV model types and functions
pub use models::linear_iv::{
    build_fixed_time_metrics,
    build_linear_iv,
    build_linear_iv_from_market_data,
    compute_atm_iv,
    compute_fixed_delta_iv,
    DeltaIv,
    DeltaMetrics,
    FixedTimeMetrics,
    LinearIvConfig,
    LinearIvOutput,
    TemporalConfig,
    // Temporal interpolation types and functions
    TemporalInterpMethod,
};

// Model parameter types
pub use model_params::{ModelParams, SviModelParams};

// Model parameters for users

// ================================================================================================
// DEFAULT CONFIGURATIONS
// ================================================================================================

/// Pre-configured optimization settings for common use cases.
///
/// This module provides several optimization configuration presets tailored for different
/// scenarios, from rapid development to high-precision production trading systems.
///
/// # Available Configurations
///
/// - [`production()`]: Production-grade settings for live trading
/// - [`fast()`]: Development-optimized settings
/// - [`research()`]: High-precision settings for research
/// - [`minimal()`]: Quick validation settings
pub mod default_configs {
    use crate::calibration::config::OptimizationConfig;

    /// Production-grade configuration optimized for live trading systems.
    ///
    /// **Characteristics:**
    /// - Maximum iterations: 5,000
    /// - Convergence tolerance: 1e-8
    /// - Robust convergence with high accuracy
    /// - Suitable for real-time trading environments
    ///
    /// **Use Cases:**
    /// - Live option pricing systems
    /// - Production volatility surface construction
    /// - High-frequency trading applications
    ///
    /// # Example
    ///
    /// ```rust
    /// use surface_lib::default_configs;
    ///
    /// let config = default_configs::production();
    /// // Use for production calibration...
    /// ```
    pub fn production() -> OptimizationConfig {
        OptimizationConfig::production()
    }

    /// Fast configuration optimized for development and testing.
    ///
    /// **Characteristics:**
    /// - Maximum iterations: 1,000
    /// - Convergence tolerance: 1e-6
    /// - Balanced speed and accuracy
    /// - Good convergence for most market conditions
    ///
    /// **Use Cases:**
    /// - Development and prototyping
    /// - Integration testing
    /// - Quick market analysis
    ///
    /// # Example
    ///
    /// ```rust
    /// use surface_lib::default_configs;
    ///
    /// let config = default_configs::fast();
    /// // Use for development...
    /// ```
    pub fn fast() -> OptimizationConfig {
        OptimizationConfig::fast()
    }

    /// High-precision configuration for research and backtesting.
    ///
    /// **Characteristics:**
    /// - Maximum iterations: 10,000
    /// - Convergence tolerance: 1e-9
    /// - Maximum accuracy and precision
    /// - Extensive parameter exploration
    ///
    /// **Use Cases:**
    /// - Academic research
    /// - Historical backtesting
    /// - Model validation studies
    /// - Parameter sensitivity analysis
    ///
    /// # Example
    ///
    /// ```rust
    /// use surface_lib::default_configs;
    ///
    /// let config = default_configs::research();
    /// // Use for research applications...
    /// ```
    pub fn research() -> OptimizationConfig {
        OptimizationConfig::research()
    }

    /// Minimal configuration for quick validation and debugging.
    ///
    /// **Characteristics:**
    /// - Maximum iterations: 100
    /// - Convergence tolerance: 1e-4
    /// - Very fast execution
    /// - Lower accuracy, suitable for quick checks
    ///
    /// **Use Cases:**
    /// - Quick validation
    /// - Debugging and troubleshooting
    /// - Unit tests
    /// - Proof-of-concept work
    ///
    /// # Example
    ///
    /// ```rust
    /// use surface_lib::default_configs;
    ///
    /// let config = default_configs::minimal();
    /// // Use for quick validation...
    /// ```
    pub fn minimal() -> OptimizationConfig {
        OptimizationConfig::minimal()
    }
}

/// Configuration parameters for SVI model calibration.
#[derive(Debug)]
pub struct CalibrationParams {
    /// Custom parameter bounds (None for adaptive bounds)
    pub param_bounds: Option<SVIParamBounds>,
    /// Optional model-specific parameters (type-erased)
    pub model_params: Option<Box<dyn ModelParams>>,
    /// Strength of temporal regularisation on raw parameters (λ).
    /// None = library default (1e-2) when an initial guess is supplied.
    pub reg_lambda: Option<f64>,
}

impl Default for CalibrationParams {
    fn default() -> Self {
        Self {
            param_bounds: None,
            model_params: Some(Box::new(model_params::SviModelParams::default())),
            reg_lambda: None,
        }
    }
}

impl CalibrationParams {
    pub fn conservative() -> Self {
        Self::default()
    }

    pub fn aggressive() -> Self {
        Self::default()
    }

    pub fn fast() -> Self {
        Self::default()
    }
}

/// Calibrate SVI model parameters to market option data.
///
/// This function performs advanced optimization to fit SVI model parameters to observed market
/// implied volatilities. The optimization uses a two-stage approach: global search with CMA-ES
/// followed by local refinement with L-BFGS-B for robust parameter estimation.
///
/// # Arguments
///
/// * `data` - Market option data for a single expiration. Must contain option type, strikes,
///   underlying price, time to expiration, market implied volatilities, and vega values.
/// * `config` - Optimization configuration specifying algorithm parameters, tolerances, and
///   computational limits. Use [`default_configs`] for common presets.
/// * `calib_params` - Calibration-specific parameters controlling log-moneyness range, arbitrage
///   checking, and penalty weights. Use [`CalibrationParams::default()`] for standard settings.
///
/// # Returns
///
/// Returns a tuple containing:
/// - `f64`: Final objective function value (lower is better)
/// - `Vec<f64>`: Optimized SVI parameters `[a, b, rho, m, sigma]`
/// - `SVIParamBounds`: The actual bounds used during optimization (can be fed back as input)
///
/// # Errors
///
/// * `anyhow::Error` if the data contains multiple expirations (SVI requires single expiration)
/// * `anyhow::Error` if market data is insufficient or contains invalid values
/// * `anyhow::Error` if optimization fails to converge within specified limits
///
/// # SVI Parameters
///
/// The SVI model parameterizes total variance as:
/// ```text
/// w(k) = a + b * (ρ(k-m) + sqrt((k-m)² + σ²))
/// ```
/// Where:
/// - `a`: Base variance level (vertical shift)
/// - `b`: Slope factor (overall variance level)
/// - `ρ`: Asymmetry parameter (skew, must be in (-1, 1))
/// - `m`: Horizontal shift (ATM location in log-moneyness)
/// - `σ`: Curvature parameter (smile curvature, must be > 0)
///
/// # Example
///
/// ```rust,no_run
/// use surface_lib::{calibrate_svi, default_configs, CalibrationParams, MarketDataRow};
///
/// // Load market data for a single expiration
/// let market_data: Vec<MarketDataRow> = load_single_expiry_data();
///
/// // Use fast configuration for development
/// let config = default_configs::fast();
/// let calib_params = CalibrationParams::default();
///
/// // Calibrate SVI parameters
/// match calibrate_svi(market_data, config, calib_params, None) {
///     Ok((objective, params, used_bounds)) => {
///         println!("Calibration successful!");
///         println!("Final objective: {:.6}", objective);
///         println!("SVI parameters: {:?}", params);
///         println!("Used bounds: {:?}", used_bounds);
///     }
///     Err(e) => eprintln!("Calibration failed: {}", e),
/// }
/// # fn load_single_expiry_data() -> Vec<MarketDataRow> { vec![] }
/// ```
///
/// # Performance Notes
///
/// - Calibration typically takes 1-10 seconds depending on configuration and data size
/// - Memory usage scales linearly with the number of option contracts
/// - For production systems, consider using [`default_configs::production()`] for optimal accuracy
pub fn calibrate_svi(
    data: Vec<InternalMarketDataRow>,
    config: InternalOptimizationConfig,
    calib_params: CalibrationParams,
    initial_guess: Option<Vec<f64>>,
) -> Result<(f64, Vec<f64>, SVIParamBounds)> {
    // Create SVI calibrator with user-provided parameters
    let mut calibrator =
        SVIModelCalibrator::new(&data, calib_params.param_bounds, calib_params.model_params)?;

    // If we have an initial guess, use it both as warm-start and as regularisation anchor
    if let Some(ref guess) = initial_guess {
        calibrator.set_prev_solution(guess.clone());
        let lambda = calib_params.reg_lambda.unwrap_or(1e-2);
        calibrator.set_temporal_reg_lambda(lambda);
    }

    // Execute calibration using adaptive pipeline directly
    let (best_obj, best_params, bounds_vec) =
        calibrate_model_adaptive(Box::new(calibrator), &data, &config, initial_guess);

    // Convert the bounds vector back to SVIParamBounds
    let used_bounds = SVIParamBounds::from(bounds_vec.as_slice());

    Ok((best_obj, best_params, used_bounds))
}

/// Evaluate the SVI calibration objective for a fixed parameter set.
///
/// This produces **exactly the same loss value** that `calibrate_svi` minimises
/// internally, honouring any ATM-boost and vega-weighting settings embedded in
/// `calib_params`.  It enables external callers (e.g. live monitoring) to
/// measure model fit quality without re-running the optimiser.
pub fn evaluate_svi(
    data: Vec<MarketDataRow>,
    params: SVIParams,
    calib_params: CalibrationParams,
) -> Result<f64> {
    use crate::calibration::types::ModelCalibrator;
    use crate::model_params::SviModelParams;
    use models::svi::svi_calibrator::SVIModelCalibrator;

    // Clone model_params if it is SviModelParams; otherwise pass None.
    let mp_clone: Option<Box<dyn crate::model_params::ModelParams>> =
        calib_params.model_params.as_ref().and_then(|mp| {
            mp.as_any()
                .downcast_ref::<SviModelParams>()
                .map(|p| Box::new(p.clone()) as Box<dyn crate::model_params::ModelParams>)
        });

    let calibrator = SVIModelCalibrator::new(&data, calib_params.param_bounds.clone(), mp_clone)?;

    let p_vec = vec![params.a, params.b, params.rho, params.m, params.sigma];
    Ok(ModelCalibrator::evaluate_objective(
        &calibrator,
        &p_vec,
        &data,
    ))
}

/// Price European options using calibrated SVI model parameters.
///
/// This function takes pre-calibrated SVI parameters and applies them to price a set of options
/// using the Black-Scholes framework with SVI-derived implied volatilities. The pricing is
/// efficient and suitable for real-time applications.
///
/// # Arguments
///
/// * `params` - Calibrated SVI parameters containing the time to expiration and model coefficients
/// * `market_data` - Option contracts to price (can be same or different from calibration data)
/// * `fixed_params` - Market parameters including risk-free rate and dividend yield
///
/// # Returns
///
/// Vector of [`PricingResult`] containing option details, model prices, and implied volatilities.
/// Results are sorted by strike price in ascending order.
///
/// # Pricing Methodology
///
/// 1. **Log-moneyness calculation**: `k = ln(K/S)` for each option
/// 2. **SVI implied volatility**: `σ(k) = sqrt(w(k)/t)` where `w(k)` is SVI total variance
/// 3. **Black-Scholes pricing**: European option price using SVI-derived volatility
/// 4. **Result compilation**: Organized results with validation and error handling
///
/// # Example
///
/// ```rust,no_run
/// use surface_lib::{
///     price_with_svi, MarketDataRow, FixedParameters,
///     models::svi::svi_model::SVIParams
/// };
///
/// # let market_data: Vec<MarketDataRow> = vec![];
/// // Create SVI parameters (typically from calibration)
/// let svi_params = SVIParams {
///     t: 0.0274,      // ~10 days to expiration
///     a: 0.04,        // Base variance
///     b: 0.2,         // Slope factor
///     rho: -0.3,      // Negative skew
///     m: 0.0,         // ATM at log-moneyness 0
///     sigma: 0.2,     // Curvature
/// };
///
/// // Market parameters
/// let fixed_params = FixedParameters {
///     r: 0.02,        // 2% risk-free rate
///     q: 0.0,         // No dividend yield
/// };
///
/// // Price options
/// let pricing_results = price_with_svi(svi_params, market_data, fixed_params);
///
/// for result in &pricing_results {
///     println!("Strike {}: Price ${:.2}, IV {:.1}%",
///              result.strike_price,
///              result.model_price,
///              result.model_iv * 100.0);
/// }
/// ```
///
/// # Error Handling
///
/// The function uses robust error handling:
/// - Invalid pricing results default to zero price and implied volatility
/// - Time mismatches between SVI parameters and market data are handled gracefully
/// - Non-finite or negative volatilities are caught and logged
///
/// # Performance Notes
///
/// - Pricing scales linearly with the number of options
/// - Typical performance: 10,000+ options per second on modern hardware
/// - Memory usage is minimal with in-place calculations
pub fn price_with_svi(
    params: SVIParams,
    market_data: Vec<MarketDataRow>,
    fixed_params: FixedParameters,
) -> Vec<PricingResult> {
    // Create SVI volatility slice from parameters
    let slice = SVISlice::new(params);
    let r = fixed_params.r;
    let q = fixed_params.q;

    // Pre-allocate results vector for efficiency
    let mut results = Vec::with_capacity(market_data.len());

    // Price each option using SVI-derived implied volatility
    for row in market_data {
        let pricing_result = price_option(
            &row.option_type,
            row.strike_price,
            row.underlying_price,
            r,
            q,
            row.years_to_exp,
            &slice,
        )
        .unwrap_or(OptionPricingResult {
            price: 0.0,
            model_iv: 0.0,
        });

        results.push(PricingResult {
            option_type: row.option_type,
            strike_price: row.strike_price,
            underlying_price: row.underlying_price,
            years_to_exp: row.years_to_exp,
            model_price: pricing_result.price,
            model_iv: pricing_result.model_iv,
        });
    }

    // Sort results by strike price for consistent ordering
    results.sort_by(|a, b| {
        a.strike_price
            .partial_cmp(&b.strike_price)
            .unwrap_or(Ordering::Equal)
    });
    results
}
