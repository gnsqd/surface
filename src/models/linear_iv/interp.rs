use anyhow::{anyhow, Result};
use roots::find_root_brent;
use statrs::distribution::{ContinuousCDF, Normal};

use super::types::*;

/// Compute sorted (log-moneyness, total_variance) points from market data
/// Filters out invalid IVs (market_iv <= 0), handles duplicates by averaging, and sorts by log-moneyness
pub fn prepare_points(points: &[MarketDataRow], forward: f64, tte: f64) -> Vec<(f64, f64)> {
    use std::collections::HashMap;

    let mut x_to_omegas: HashMap<String, Vec<f64>> = HashMap::new();

    for point in points {
        if point.market_iv <= 0.0 {
            continue; // Skip invalid IVs
        }

        let x = (point.strike_price / forward).ln(); // log-moneyness
        let omega = point.market_iv * point.market_iv * tte; // total variance

        // Use a string key with limited precision to group similar x values
        let x_key = format!("{:.8}", x); // 8 decimal places precision
        x_to_omegas.entry(x_key).or_default().push(omega);
    }

    // Average duplicates and collect results
    let mut result: Vec<(f64, f64)> = x_to_omegas
        .into_iter()
        .map(|(x_str, omegas)| {
            let x: f64 = x_str.parse().unwrap();
            let avg_omega = omegas.iter().sum::<f64>() / omegas.len() as f64;
            (x, avg_omega)
        })
        .collect();

    // Sort by log-moneyness for efficient interpolation
    result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    result
}

/// Linear interpolation in variance space
/// Returns None if query_x is outside the range and extrapolation is disabled or fails
pub fn linear_interp(sorted_points: &[(f64, f64)], query_x: f64) -> Option<f64> {
    linear_interp_with_config(sorted_points, query_x, true)
}

/// Linear interpolation with configurable extrapolation
/// Returns None if query_x is outside the range and extrapolation is disabled or fails
pub fn linear_interp_with_config(
    sorted_points: &[(f64, f64)],
    query_x: f64,
    allow_extrapolation: bool,
) -> Option<f64> {
    if sorted_points.is_empty() {
        return None;
    }

    if sorted_points.len() == 1 {
        return Some(sorted_points[0].1);
    }

    let first_x = sorted_points[0].0;
    let last_x = sorted_points[sorted_points.len() - 1].0;

    // Handle extrapolation based on configuration
    if query_x < first_x {
        if !allow_extrapolation {
            return None; // Extrapolation disabled
        }
        // Extrapolate using first two points
        if sorted_points.len() < 2 {
            return Some(sorted_points[0].1);
        }
        let (x1, y1) = sorted_points[0];
        let (x2, y2) = sorted_points[1];
        let slope = (y2 - y1) / (x2 - x1);
        let extrapolated = y1 + slope * (query_x - x1);
        return if extrapolated > 0.0 {
            Some(extrapolated)
        } else {
            None
        };
    }

    if query_x > last_x {
        if !allow_extrapolation {
            return None; // Extrapolation disabled
        }
        // Extrapolate using last two points
        let n = sorted_points.len();
        let (x1, y1) = sorted_points[n - 2];
        let (x2, y2) = sorted_points[n - 1];
        let slope = (y2 - y1) / (x2 - x1);
        let extrapolated = y2 + slope * (query_x - x2);
        return if extrapolated > 0.0 {
            Some(extrapolated)
        } else {
            None
        };
    }

    // Find the interval containing query_x
    for i in 0..sorted_points.len() - 1 {
        let (x1, y1) = sorted_points[i];
        let (x2, y2) = sorted_points[i + 1];

        if query_x >= x1 && query_x <= x2 {
            // Linear interpolation
            let t = (query_x - x1) / (x2 - x1);
            let interpolated = y1 + t * (y2 - y1);
            return Some(interpolated);
        }
    }

    None
}

/// Compute ATM implied volatility via linear interpolation at x=0
pub fn compute_atm_iv(points: &[MarketDataRow], forward: f64, tte: f64) -> Result<f64> {
    if tte <= 0.0 {
        return Err(anyhow!("Time to expiration must be positive, got: {}", tte));
    }

    let sorted_points = prepare_points(points, forward, tte);

    if sorted_points.len() < 2 {
        return Err(anyhow!("Insufficient points for ATM IV interpolation"));
    }

    let omega_atm = linear_interp(&sorted_points, 0.0)
        .ok_or_else(|| anyhow!("Failed to interpolate ATM variance"))?;

    if omega_atm <= 0.0 {
        return Err(anyhow!("Non-positive ATM variance: {}", omega_atm));
    }

    Ok((omega_atm / tte).sqrt())
}

/// Black-Scholes delta calculation with dividend yield
/// Uses the standard normal CDF from statrs for precision
/// x = ln(K/F), so d1 uses -x for standard Black-Scholes formula
pub fn bs_delta(x: f64, sigma: f64, tte: f64, is_call: bool, q: f64) -> f64 {
    if sigma <= 0.0 || tte <= 0.0 {
        return if is_call { 0.0 } else { -1.0 };
    }

    // Standard Black-Scholes d1: (-x) because x = ln(K/F) and we need ln(F/K)
    let d1 = -x / (sigma * tte.sqrt()) + 0.5 * sigma * tte.sqrt();
    let normal = Normal::new(0.0, 1.0).unwrap();

    // Apply dividend yield factor e^(-q*T)
    let fwd_factor = (-q * tte).exp();

    if is_call {
        normal.cdf(d1) * fwd_factor
    } else {
        (normal.cdf(d1) - 1.0) * fwd_factor
    }
}

/// Solve for the log-moneyness that gives the target delta
/// Uses Brent's method for robust convergence
pub fn compute_fixed_delta_iv(
    target_delta: f64,
    sorted_points: &[(f64, f64)],
    tte: f64,
    tol: f64,
) -> Result<f64> {
    compute_fixed_delta_iv_with_config(target_delta, sorted_points, tte, tol, true, 0.0)
}

/// Solve for the log-moneyness that gives the target delta with configurable extrapolation
/// Uses Brent's method for robust convergence
pub fn compute_fixed_delta_iv_with_config(
    target_delta: f64,
    sorted_points: &[(f64, f64)],
    tte: f64,
    tol: f64,
    allow_extrapolation: bool,
    q: f64,
) -> Result<f64> {
    if sorted_points.is_empty() {
        return Err(anyhow!("No points available for delta solving"));
    }

    let is_call = target_delta > 0.0;

    // Define the objective function: bs_delta(x, sigma(x), tte, is_call, q) - target_delta
    let objective = |x: f64| -> f64 {
        let omega = match linear_interp_with_config(sorted_points, x, allow_extrapolation) {
            Some(w) if w > 0.0 => w,
            _ => {
                // More neutral fallback values to avoid biasing the solver
                // Return a large error to push solver away from this region
                return if is_call {
                    if target_delta > 0.0 {
                        -10.0
                    } else {
                        10.0
                    }
                } else if target_delta < 0.0 {
                    10.0
                } else {
                    -10.0
                };
            }
        };

        let sigma = (omega / tte).sqrt();
        bs_delta(x, sigma, tte, is_call, q) - target_delta
    };

    // Determine search bounds based on sorted points
    let min_x = sorted_points[0].0;
    let max_x = sorted_points[sorted_points.len() - 1].0;

    // Expand search range for delta solving
    let search_min = min_x - 1.0;
    let search_max = max_x + 1.0;

    // Use Brent's method to find the root
    match find_root_brent(search_min, search_max, &objective, &mut tol.clone()) {
        Ok(x_solution) => {
            // Convert back to implied volatility
            let omega =
                linear_interp_with_config(sorted_points, x_solution, allow_extrapolation)
                    .ok_or_else(|| anyhow!("Failed to interpolate at solution x={}", x_solution))?;

            if omega <= 0.0 {
                return Err(anyhow!("Non-positive variance at solution: {}", omega));
            }

            Ok((omega / tte).sqrt())
        }
        Err(_) => Err(anyhow!(
            "Root finding failed for target_delta={}",
            target_delta
        )),
    }
}

/// Compute risk reversal and butterfly metrics for all symmetric delta pairs
pub fn compute_all_metrics(
    delta_ivs: &[DeltaIv],
    atm_iv: f64,
) -> (Vec<DeltaMetrics>, Option<f64>, Option<f64>) {
    let mut metrics = Vec::new();
    let mut rr_25 = None;
    let mut bf_25 = None;

    // Find all positive deltas and check for their negative counterparts
    let positive_deltas: Vec<f64> = delta_ivs
        .iter()
        .filter_map(|div| {
            if div.delta > 0.0 {
                Some(div.delta)
            } else {
                None
            }
        })
        .collect();

    for &pos_delta in &positive_deltas {
        let neg_delta = -pos_delta;

        let call_iv = delta_ivs
            .iter()
            .find(|div| (div.delta - pos_delta).abs() < 1e-10)
            .map(|div| div.iv);

        let put_iv = delta_ivs
            .iter()
            .find(|div| (div.delta - neg_delta).abs() < 1e-10)
            .map(|div| div.iv);

        if let (Some(call_vol), Some(put_vol)) = (call_iv, put_iv) {
            let rr = call_vol - put_vol;
            let bf = (call_vol + put_vol) / 2.0 - atm_iv;

            metrics.push(DeltaMetrics {
                delta_level: pos_delta,
                risk_reversal: rr,
                butterfly: bf,
            });

            // Store 25-delta metrics for backward compatibility
            if (pos_delta - 0.25).abs() < 1e-10 {
                rr_25 = Some(rr);
                bf_25 = Some(bf);
            }
        }
    }

    (metrics, rr_25, bf_25)
}

/// Compute risk reversal and butterfly metrics from delta IVs (backward compatibility)
pub fn compute_metrics(delta_ivs: &[DeltaIv], atm_iv: f64) -> (Option<f64>, Option<f64>) {
    let (_, rr_25, bf_25) = compute_all_metrics(delta_ivs, atm_iv);
    (rr_25, bf_25)
}

/// Check if points have reasonable coverage for delta calculations
/// Logs warnings if asymmetric coverage might affect certain deltas
fn check_point_coverage(points: &[MarketDataRow], config: &LinearIvConfig) {
    let has_calls = points.iter().any(|p| p.option_type == "call");
    let has_puts = points.iter().any(|p| p.option_type == "put");

    let has_negative_deltas = config.deltas.iter().any(|&d| d < 0.0);
    let has_positive_deltas = config.deltas.iter().any(|&d| d > 0.0);

    if has_negative_deltas && !has_puts {
        eprintln!("Warning: Negative deltas requested but no put options in data. Some delta calculations may fail.");
    }

    if has_positive_deltas && !has_calls {
        eprintln!("Warning: Positive deltas requested but no call options in data. Some delta calculations may fail.");
    }

    if !has_calls && !has_puts {
        eprintln!("Warning: No clear option type classification in data.");
    }
}

/// Convenience function to build linear IV output using underlying price from market data
/// Uses the underlying_price from the first market data point as the forward price
pub fn build_linear_iv_from_market_data(
    points: &[MarketDataRow],
    config: &LinearIvConfig,
) -> Result<LinearIvOutput> {
    if points.is_empty() {
        return Err(anyhow!("No market data provided"));
    }

    let forward = points[0].underlying_price;
    let tte = points[0].years_to_exp;

    build_linear_iv(points, forward, tte, config)
}

/// Main function to build complete linear IV output
/// Orchestrates all the above functions to produce the final result
pub fn build_linear_iv(
    points: &[MarketDataRow],
    forward: f64,
    tte: f64,
    config: &LinearIvConfig,
) -> Result<LinearIvOutput> {
    if points.len() < config.min_points {
        return Err(anyhow!(
            "Insufficient points: {} < {}",
            points.len(),
            config.min_points
        ));
    }

    // Check for potential issues with point coverage
    check_point_coverage(points, config);

    // Compute ATM IV
    let atm_iv = compute_atm_iv(points, forward, tte)?;

    // Prepare sorted points for delta solving
    let sorted_points = prepare_points(points, forward, tte);

    // Compute fixed-delta IVs
    let mut delta_ivs = Vec::new();

    for &delta in &config.deltas {
        match compute_fixed_delta_iv_with_config(
            delta,
            &sorted_points,
            tte,
            config.solver_tol,
            config.allow_extrapolation,
            config.dividend_yield,
        ) {
            Ok(iv) => {
                delta_ivs.push(DeltaIv { delta, iv });
            }
            Err(_) => {
                // Skip deltas that fail to solve (e.g., too far OTM)
                continue;
            }
        }
    }

    // Compute all metrics (including backward-compatible 25-delta)
    let (delta_metrics, rr_25, bf_25) = compute_all_metrics(&delta_ivs, atm_iv);

    Ok(LinearIvOutput {
        atm_iv,
        delta_ivs,
        rr_25,
        bf_25,
        delta_metrics,
        tte,
    })
}
