// Re-export MarketDataRow from calibration types for consistency
pub use crate::calibration::types::MarketDataRow;

/// Configuration for linear IV interpolation
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinearIvConfig {
    /// Delta values to compute (e.g., [-0.25, -0.1, 0.1, 0.25])
    pub deltas: Vec<f64>,
    /// Solver tolerance for delta solving
    pub solver_tol: f64,
    /// Minimum number of market data points required
    pub min_points: usize,
    /// Allow extrapolation beyond market data range
    pub allow_extrapolation: bool,
    /// Risk-free interest rate (default: 0.0)
    pub risk_free_rate: f64,
    /// Dividend yield (default: 0.0)
    pub dividend_yield: f64,
}

impl Default for LinearIvConfig {
    fn default() -> Self {
        Self {
            deltas: vec![-0.25, -0.1, 0.1, 0.25],
            solver_tol: 1e-6,
            min_points: 3,
            allow_extrapolation: true,
            risk_free_rate: 0.0,
            dividend_yield: 0.0,
        }
    }
}

/// Delta-IV pair
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeltaIv {
    pub delta: f64,
    pub iv: f64,
}

/// Risk reversal and butterfly metrics for a specific delta level
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeltaMetrics {
    pub delta_level: f64,
    pub risk_reversal: f64,
    pub butterfly: f64,
}

/// Output from linear IV interpolation
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinearIvOutput {
    /// ATM implied volatility
    pub atm_iv: f64,
    /// Vector of delta-IV pairs
    pub delta_ivs: Vec<DeltaIv>,
    /// 25-delta risk reversal (if available) - kept for backward compatibility
    pub rr_25: Option<f64>,
    /// 25-delta butterfly (if available) - kept for backward compatibility
    pub bf_25: Option<f64>,
    /// All computed delta metrics (RR and BF for all available symmetric pairs)
    pub delta_metrics: Vec<DeltaMetrics>,
    /// Time to expiration in years
    pub tte: f64,
}

impl LinearIvOutput {
    /// Get IV for a specific delta
    pub fn get_iv_for_delta(&self, target_delta: f64) -> Option<f64> {
        self.delta_ivs
            .iter()
            .find(|&div| (div.delta - target_delta).abs() < 1e-10)
            .map(|div| div.iv)
    }
}

/// Methods for interpolating metrics across time-to-expiration
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TemporalInterpMethod {
    /// Direct linear interpolation on TTE-value pairs
    LinearTte,
    /// Interpolate total variance (w = iv^2 * tte) then back out IV
    /// Consistent with strike interpolation in variance space
    #[default]
    LinearVariance,
    /// Scale using sqrt(tte), common for short tenors
    SquareRootTime,
}

/// Configuration for temporal interpolation to fixed time grid
///
/// Controls how volatility metrics are interpolated across multiple maturities
/// to produce standardized expiry ladders. Essential for building consistent
/// volatility surfaces for pricing and risk management.
///
/// # Example Usage
///
/// ```rust
/// # use surface_lib::{TemporalConfig, TemporalInterpMethod};
/// // Standard weekly/monthly expiry ladder
/// let config = TemporalConfig {
///     fixed_days: vec![1, 7, 14, 30, 60, 90],
///     interp_method: TemporalInterpMethod::LinearVariance,
///     allow_short_extrapolate: true,  // Enable 1d extrapolation
///     allow_long_extrapolate: false,  // Conservative on long end
///     min_maturities: 3,              // Require good coverage
/// };
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemporalConfig {
    /// Fixed days to interpolate to (e.g., [1, 3, 7, 14, 30])
    ///
    /// Represents the standardized expiry ladder in calendar days.
    /// Common patterns include weekly (7, 14, 21, 28) or monthly (30, 60, 90) grids.
    pub fixed_days: Vec<i32>,

    /// Interpolation method for metrics across time
    ///
    /// * `LinearTte` - Simple linear interpolation, intuitive but may violate no-arbitrage
    /// * `LinearVariance` - Variance-space interpolation, mathematically consistent
    /// * `SquareRootTime` - Square-root time scaling, suitable for mean-reverting volatility
    pub interp_method: TemporalInterpMethod,

    /// Allow extrapolation to shorter TTEs than observed
    ///
    /// When `true`, enables extrapolation to expiries shorter than the minimum
    /// observed maturity. Use with caution as short-term extrapolation can be volatile.
    pub allow_short_extrapolate: bool,

    /// Allow extrapolation to longer TTEs than observed
    ///
    /// When `true`, enables extrapolation to expiries longer than the maximum
    /// observed maturity. Generally safer than short extrapolation.
    pub allow_long_extrapolate: bool,

    /// Minimum number of maturities required
    ///
    /// Minimum number of distinct maturities needed for interpolation.
    /// Must be ≥ 2 for any interpolation. Higher values provide better stability.
    pub min_maturities: usize,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            fixed_days: vec![1, 3, 7, 14, 30],
            interp_method: TemporalInterpMethod::LinearVariance,
            allow_short_extrapolate: false,
            allow_long_extrapolate: true,
            min_maturities: 2,
        }
    }
}

impl TemporalConfig {
    /// Create TemporalConfig from a list of days with sensible defaults
    ///
    /// # Example
    /// ```rust
    /// # use surface_lib::TemporalConfig;
    /// let config = TemporalConfig::from_days(vec![7, 14, 30, 60]);
    /// assert_eq!(config.fixed_days, vec![7, 14, 30, 60]);
    /// ```
    pub fn from_days(days: Vec<i32>) -> Self {
        Self {
            fixed_days: days,
            ..Default::default()
        }
    }

    /// Create weekly expiry ladder (7, 14, 21, 28 days)
    pub fn weekly() -> Self {
        Self::from_days(vec![7, 14, 21, 28])
    }

    /// Create monthly expiry ladder (30, 60, 90, 120 days)  
    pub fn monthly() -> Self {
        Self::from_days(vec![30, 60, 90, 120])
    }
}

/// Metrics for a specific fixed time-to-expiration point
///
/// Contains interpolated volatility metrics for a single standardized expiry.
/// This is the primary output of temporal interpolation, providing ATM volatility
/// and delta-based metrics (risk reversal and butterfly) at fixed time points.
///
/// # Usage
///
/// These metrics are typically used for:
/// * Constructing volatility surfaces for pricing engines
/// * Computing Greeks and risk sensitivities
/// * Volatility trading and hedging decisions
/// * Risk management and scenario analysis
///
/// # Example
///
/// ```rust
/// # use surface_lib::FixedTimeMetrics;
/// # let metrics = FixedTimeMetrics { tte_days: 30, tte_years: 30.0/365.0, atm_iv: 0.2, delta_metrics: vec![] };
/// println!("30d expiry: ATM IV = {:.1}%", metrics.atm_iv * 100.0);
///
/// for dm in &metrics.delta_metrics {
///     println!("  {}δ: RR = {:+.1}%, BF = {:+.1}%",
///              dm.delta_level * 100.0,
///              dm.risk_reversal * 100.0,
///              dm.butterfly * 100.0);
/// }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FixedTimeMetrics {
    /// Time to expiration in calendar days
    ///
    /// Integer representation of the standardized expiry, matching the
    /// requested `fixed_days` from `TemporalConfig`.
    pub tte_days: i32,

    /// Time to expiration in years (day count = ACT/365)
    ///
    /// Precise fractional representation used for all calculations.
    /// Computed as `tte_days / 365.0` using actual/365 day count convention.
    pub tte_years: f64,

    /// ATM implied volatility at this time-to-expiration
    ///
    /// Annualized implied volatility at the money (log-moneyness = 0),
    /// interpolated using the specified temporal method. This is the
    /// primary volatility level for this expiry.
    pub atm_iv: f64,

    /// Delta metrics (RR and BF) for all available delta levels
    ///
    /// Risk reversal and butterfly metrics for symmetric delta pairs
    /// (e.g., ±10δ, ±25δ). Only populated for delta levels that have
    /// sufficient data across the input maturities.
    pub delta_metrics: Vec<DeltaMetrics>,
}
