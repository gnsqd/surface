//! Temporal interpolation for fixed time grid construction
//!
//! This module provides functionality to interpolate volatility metrics across multiple
//! maturities to construct standardized time grids. It extends the single-maturity
//! linear IV interpolation to handle multi-maturity option chains.
//!
//! # Overview
//!
//! The temporal interpolation module enables traders and quants to:
//! - Build consistent volatility surfaces across standardized expiry ladders
//! - Interpolate between observed maturities using different methodologies
//! - Extrapolate to shorter/longer tenors when appropriate
//! - Maintain mathematical consistency with strike-space interpolation
//!
//! # Interpolation Methods
//!
//! Three temporal interpolation methods are supported:
//!
//! ## LinearTte
//! Direct linear interpolation on time-to-expiration vs metric value pairs.
//! Simple and intuitive, suitable for most applications.
//!
//! ## LinearVariance  
//! Interpolates total variance (ω = σ²T) and converts back to implied volatility.
//! Mathematically consistent with no-arbitrage conditions and variance swaps.
//! Recommended for professional trading systems.
//!
//! ## SquareRootTime
//! Scales volatility by √(T_target/T_base). Common approximation for
//! short-term extrapolation when volatility is mean-reverting.
//!
//! # Usage Pattern
//!
//! 1. Collect multi-maturity option chain data
//! 2. Configure `TemporalConfig` with desired fixed days and interpolation method
//! 3. Call `build_fixed_time_metrics()` to generate standardized time grid
//! 4. Use resulting metrics for pricing, Greeks, and risk management
//!
//! # Example
//!
//! ```rust,no_run
//! use surface_lib::{MarketDataRow, LinearIvConfig, TemporalConfig, build_fixed_time_metrics};
//!
//! # let market_data: Vec<MarketDataRow> = vec![];
//! # let forward = 100.0;
//! let temporal_config = TemporalConfig {
//!     fixed_days: vec![1, 7, 14, 30],
//!     ..Default::default()
//! };
//! let strike_config = LinearIvConfig::default();
//!
//! let metrics = build_fixed_time_metrics(
//!     &market_data, forward, &temporal_config, &strike_config
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use super::interp::build_linear_iv;
use super::types::*;

/// Floating point epsilon for temporal interpolation comparisons
/// Generous tolerance to handle day/year conversions and accumulated rounding
const TEMPORAL_EPSILON: f64 = 1e-8;

/// Group market data by time-to-expiration, returning sorted groups
/// Each group contains all market data for a single maturity
fn group_by_tte(data: &[MarketDataRow]) -> Vec<(f64, Vec<MarketDataRow>)> {
    let mut tte_to_data: HashMap<String, Vec<MarketDataRow>> = HashMap::new();

    // Group by TTE with limited precision to handle floating point issues
    for row in data {
        let tte_key = format!("{:.8}", row.years_to_exp); // 8 decimal places precision
        tte_to_data.entry(tte_key).or_default().push(row.clone());
    }

    // Convert to vector and sort by TTE
    let mut groups: Vec<(f64, Vec<MarketDataRow>)> = tte_to_data
        .into_iter()
        .map(|(tte_str, data)| {
            let tte: f64 = tte_str.parse().unwrap();
            (tte, data)
        })
        .collect();

    groups.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    groups
}

/// Linear interpolation helper for temporal interpolation
///
/// Performs piecewise linear interpolation on sorted (TTE, metric_value) pairs
/// with configurable extrapolation behavior and floating point precision handling.
///
/// # Arguments
///
/// * `tte_metrics` - Sorted array of (time-to-expiration, metric_value) pairs
/// * `target_tte` - Target time to interpolate at
/// * `allow_short_extrap` - Allow extrapolation below minimum TTE
/// * `allow_long_extrap` - Allow extrapolation above maximum TTE
///
/// # Returns
///
/// * `Some(value)` - Interpolated or extrapolated metric value
/// * `None` - If extrapolation is required but disabled, or if data is insufficient
///
/// # Extrapolation Behavior
///
/// When extrapolation is enabled, linear extrapolation uses the slope from the
/// nearest two points. Exact matches at boundaries return precise values to
/// handle floating point precision issues.
fn temporal_interp(
    tte_metrics: &[(f64, f64)], // (tte, metric_value) pairs
    target_tte: f64,
    allow_short_extrap: bool,
    allow_long_extrap: bool,
) -> Option<f64> {
    if tte_metrics.is_empty() {
        return None;
    }

    if tte_metrics.len() == 1 {
        return Some(tte_metrics[0].1);
    }

    let min_tte = tte_metrics[0].0;
    let max_tte = tte_metrics[tte_metrics.len() - 1].0;

    // Check extrapolation bounds with epsilon tolerance
    if (target_tte - min_tte) < -TEMPORAL_EPSILON && !allow_short_extrap {
        return None;
    }
    if (target_tte - max_tte) > TEMPORAL_EPSILON && !allow_long_extrap {
        return None;
    }

    // Handle extrapolation cases
    if target_tte <= min_tte {
        // If exactly at min_tte, return the exact value
        if (target_tte - min_tte).abs() < 1e-10 {
            return Some(tte_metrics[0].1);
        }

        if tte_metrics.len() < 2 {
            return Some(tte_metrics[0].1);
        }
        let (tte1, val1) = tte_metrics[0];
        let (tte2, val2) = tte_metrics[1];
        let slope = (val2 - val1) / (tte2 - tte1);
        return Some(val1 + slope * (target_tte - tte1));
    }

    if target_tte >= max_tte {
        // If exactly at max_tte, return the exact value
        if (target_tte - max_tte).abs() < 1e-10 {
            return Some(tte_metrics[tte_metrics.len() - 1].1);
        }

        let n = tte_metrics.len();
        let (tte1, val1) = tte_metrics[n - 2];
        let (tte2, val2) = tte_metrics[n - 1];
        let slope = (val2 - val1) / (tte2 - tte1);
        return Some(val2 + slope * (target_tte - tte2));
    }

    // Find interpolation interval
    for i in 0..tte_metrics.len() - 1 {
        let (tte1, val1) = tte_metrics[i];
        let (tte2, val2) = tte_metrics[i + 1];

        if target_tte >= tte1 && target_tte <= tte2 {
            let t = (target_tte - tte1) / (tte2 - tte1);
            return Some(val1 + t * (val2 - val1));
        }
    }

    None
}

/// Interpolate a single metric value using the specified temporal method
///
/// This function extracts a specific metric from multiple maturity outputs and
/// interpolates it temporally using one of three supported methods. It handles
/// the conversion between interpolation methods transparently.
///
/// # Arguments
///
/// * `tte_metrics` - Array of (TTE, output) pairs from multiple maturities
/// * `target_tte` - Target time-to-expiration for interpolation
/// * `method` - Temporal interpolation method to use
/// * `allow_short_extrap` - Enable extrapolation below minimum observed TTE
/// * `allow_long_extrap` - Enable extrapolation above maximum observed TTE  
/// * `metric_extractor` - Closure to extract metric value from LinearIvOutput
///
/// # Method-Specific Behavior
///
/// * **LinearTte**: Direct interpolation on (TTE, metric) pairs
/// * **LinearVariance**: Converts to total variance, interpolates, converts back
/// * **SquareRootTime**: Scales metric by sqrt(target_tte/observed_tte) ratio
fn interpolate_metric_value(
    tte_metrics: &[(f64, LinearIvOutput)],
    target_tte: f64,
    method: TemporalInterpMethod,
    allow_short_extrap: bool,
    allow_long_extrap: bool,
    metric_extractor: impl Fn(&LinearIvOutput) -> f64,
) -> Option<f64> {
    let metric_pairs: Vec<(f64, f64)> = tte_metrics
        .iter()
        .map(|(tte, output)| (*tte, metric_extractor(output)))
        .collect();

    match method {
        TemporalInterpMethod::LinearTte => temporal_interp(
            &metric_pairs,
            target_tte,
            allow_short_extrap,
            allow_long_extrap,
        ),
        TemporalInterpMethod::LinearVariance => {
            // Convert to total variance (w = iv^2 * t), interpolate, then back to IV
            let variance_pairs: Vec<(f64, f64)> = metric_pairs
                .iter()
                .map(|(tte, iv)| (*tte, iv * iv * tte))
                .collect();

            let interpolated_variance = temporal_interp(
                &variance_pairs,
                target_tte,
                allow_short_extrap,
                allow_long_extrap,
            )?;

            if interpolated_variance <= 0.0 {
                return None;
            }

            Some((interpolated_variance / target_tte).sqrt())
        }
        TemporalInterpMethod::SquareRootTime => {
            // Scale by sqrt(t): iv_target = iv_base * sqrt(t_target / t_base)
            // Handle edge case of zero TTE
            if target_tte <= 0.0 {
                return None;
            }

            // Scale values by 1/sqrt(tte) for interpolation
            let scaled_pairs: Vec<(f64, f64)> = metric_pairs
                .iter()
                .filter_map(|(tte, iv)| {
                    if *tte > 0.0 {
                        Some((*tte, iv / tte.sqrt()))
                    } else {
                        None // Skip invalid TTE values
                    }
                })
                .collect();

            if scaled_pairs.is_empty() {
                return None;
            }

            let scaled_value = temporal_interp(
                &scaled_pairs,
                target_tte,
                allow_short_extrap,
                allow_long_extrap,
            )?;

            Some(scaled_value * target_tte.sqrt())
        }
    }
}

/// Build fixed time metrics by interpolating across multiple maturities
///
/// This is the main function for temporal interpolation, taking multi-maturity option
/// data and producing interpolated volatility metrics at standardized time points.
///
/// # Arguments
///
/// * `data` - Multi-maturity option chain data with consistent underlying and forward
/// * `forward` - Forward price for all contracts (should be consistent across maturities)
/// * `temp_config` - Temporal interpolation configuration specifying target days and method
/// * `strike_config` - Configuration for per-maturity linear IV interpolation
///
/// # Returns
///
/// A vector of `FixedTimeMetrics` containing interpolated ATM IV and delta metrics
/// for each requested time point, sorted by time-to-expiration.
///
/// # Process
///
/// 1. **Group by maturity**: Market data is grouped by time-to-expiration with
///    floating point precision handling
/// 2. **Per-maturity interpolation**: Each maturity group is processed using
///    standard linear IV interpolation to extract ATM IV and delta metrics
/// 3. **Temporal interpolation**: Metrics are interpolated across time using
///    the specified method (LinearTte, LinearVariance, or SquareRootTime)
/// 4. **Extrapolation handling**: Points outside the observed range are handled
///    according to the extrapolation settings
/// 5. **Result assembly**: Final metrics are assembled and sorted by time
///
/// # Interpolation Methods
///
/// - **LinearTte**: Direct linear interpolation on (time, metric) pairs
/// - **LinearVariance**: Interpolates total variance, then converts back to IV
/// - **SquareRootTime**: Scales by sqrt(time) ratio for mean-reverting processes
///
/// # Error Conditions
///
/// * Insufficient maturities (< `min_maturities`)
/// * Insufficient points per maturity for linear IV interpolation
/// * Empty market data
/// * Individual maturity processing failures (reported with context)
///
/// # Example
///
/// ```rust,no_run
/// use surface_lib::{MarketDataRow, LinearIvConfig, TemporalConfig, TemporalInterpMethod, build_fixed_time_metrics};
///
/// # let market_data: Vec<MarketDataRow> = vec![];
/// let forward = 100.0;
/// let temp_config = TemporalConfig {
///     fixed_days: vec![1, 7, 14, 30, 60],
///     interp_method: TemporalInterpMethod::LinearVariance,
///     allow_short_extrapolate: true,
///     allow_long_extrapolate: false,
///     min_maturities: 2,
/// };
/// let strike_config = LinearIvConfig::default();
///
/// let metrics = build_fixed_time_metrics(&market_data, forward, &temp_config, &strike_config)?;
///
/// for metric in &metrics {
///     println!("{}d: ATM IV = {:.1}%", metric.tte_days, metric.atm_iv * 100.0);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Performance Notes
///
/// * Complexity scales linearly with number of maturities and target days
/// * Each maturity requires full linear IV interpolation (O(n log n) per maturity)
/// * Memory usage is proportional to the number of unique delta levels across all maturities
pub fn build_fixed_time_metrics(
    data: &[MarketDataRow],
    forward: f64,
    temp_config: &TemporalConfig,
    strike_config: &LinearIvConfig,
) -> Result<Vec<FixedTimeMetrics>> {
    if data.is_empty() {
        return Err(anyhow!("No market data provided"));
    }

    // Group data by time-to-expiration
    let tte_groups = group_by_tte(data);

    if tte_groups.len() < temp_config.min_maturities {
        return Err(anyhow!(
            "Insufficient maturities: {} < {}",
            tte_groups.len(),
            temp_config.min_maturities
        ));
    }

    // Build LinearIvOutput for each maturity
    let mut maturity_outputs = Vec::new();

    for (tte, group_data) in &tte_groups {
        match build_linear_iv(group_data, forward, *tte, strike_config) {
            Ok(output) => {
                maturity_outputs.push((*tte, output));
            }
            Err(e) => {
                return Err(anyhow!("Failed to build linear IV for TTE {}: {}", tte, e));
            }
        }
    }

    if maturity_outputs.is_empty() {
        return Err(anyhow!("No valid maturity outputs produced"));
    }

    // Sort by TTE for interpolation
    maturity_outputs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let min_tte = maturity_outputs[0].0;
    let max_tte = maturity_outputs[maturity_outputs.len() - 1].0;

    // Build metrics for each requested fixed day
    let mut results = Vec::new();

    for &fixed_days in &temp_config.fixed_days {
        let target_tte = fixed_days as f64 / 365.0;

        // Check if this point should be skipped due to extrapolation settings
        // Use epsilon comparison for floating point precision
        if (target_tte - min_tte) < -TEMPORAL_EPSILON && !temp_config.allow_short_extrapolate {
            continue;
        }
        if (target_tte - max_tte) > TEMPORAL_EPSILON && !temp_config.allow_long_extrapolate {
            continue;
        }

        // Interpolate ATM IV
        let atm_iv = interpolate_metric_value(
            &maturity_outputs,
            target_tte,
            temp_config.interp_method,
            temp_config.allow_short_extrapolate,
            temp_config.allow_long_extrapolate,
            |output| output.atm_iv,
        );

        let atm_iv = match atm_iv {
            Some(iv) if iv > 0.0 => iv,
            _ => continue, // Skip this point if ATM IV interpolation fails
        };

        // Collect all unique delta levels across all maturities
        let mut all_delta_levels = std::collections::HashSet::new();
        for (_, output) in &maturity_outputs {
            for delta_metric in &output.delta_metrics {
                // Use limited precision for delta matching
                let delta_key = format!("{:.6}", delta_metric.delta_level);
                all_delta_levels.insert(delta_key);
            }
        }

        let mut delta_metrics = Vec::new();

        // Interpolate each delta level
        for delta_key in all_delta_levels {
            let delta_level: f64 = delta_key.parse().unwrap();

            // Extract RR and BF values for this delta across all maturities
            let rr_values: Vec<(f64, f64)> = maturity_outputs
                .iter()
                .filter_map(|(tte, output)| {
                    output
                        .delta_metrics
                        .iter()
                        .find(|dm| (dm.delta_level - delta_level).abs() < 1e-6)
                        .map(|dm| (*tte, dm.risk_reversal))
                })
                .collect();

            let bf_values: Vec<(f64, f64)> = maturity_outputs
                .iter()
                .filter_map(|(tte, output)| {
                    output
                        .delta_metrics
                        .iter()
                        .find(|dm| (dm.delta_level - delta_level).abs() < 1e-6)
                        .map(|dm| (*tte, dm.butterfly))
                })
                .collect();

            // Only proceed if we have sufficient data for this delta level
            if rr_values.len() >= 2 && bf_values.len() >= 2 {
                // Interpolate RR and BF for this delta level
                let rr = temporal_interp(
                    &rr_values,
                    target_tte,
                    temp_config.allow_short_extrapolate,
                    temp_config.allow_long_extrapolate,
                );

                let bf = temporal_interp(
                    &bf_values,
                    target_tte,
                    temp_config.allow_short_extrapolate,
                    temp_config.allow_long_extrapolate,
                );

                if let (Some(rr_val), Some(bf_val)) = (rr, bf) {
                    delta_metrics.push(DeltaMetrics {
                        delta_level,
                        risk_reversal: rr_val,
                        butterfly: bf_val,
                    });
                }
            }
        }

        // Sort delta metrics by delta level for consistency
        delta_metrics.sort_by(|a, b| a.delta_level.partial_cmp(&b.delta_level).unwrap());

        results.push(FixedTimeMetrics {
            tte_days: fixed_days,
            tte_years: target_tte,
            atm_iv,
            delta_metrics,
        });
    }

    // Sort results by TTE days
    results.sort_by_key(|m| m.tte_days);

    Ok(results)
}
