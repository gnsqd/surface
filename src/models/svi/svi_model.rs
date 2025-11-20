// src/models/svi/svi_model.rs

//! Stochastic Volatility Inspired (SVI) model implementation
//!
//! The SVI model provides a parametric form for the implied volatility surface
//! that is both flexible and enforces no-arbitrage conditions. The total variance
//! w(k,t) is given by:
//!
//! w(k) = a + b * (ρ(k-m) + sqrt((k-m)² + σ²))
//!
//! where k is log-moneyness, and the parameters are:
//! - a: vertical shift (controls ATM level)
//! - b: slope factor (controls overall level)
//! - ρ: asymmetry parameter (skew, -1 < ρ < 1)
//! - m: horizontal shift (ATM location)
//! - σ: curvature parameter (controls smile curvature)

use crate::models::traits::SurfaceModel;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Parameters for the SVI (Stochastic Volatility Inspired) model for a single maturity.
///
/// The SVI model provides a parametric form for implied volatility that ensures
/// no calendar arbitrage and can be configured to avoid butterfly arbitrage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SVIParams {
    /// Time to maturity (years)
    pub t: f64,
    /// Vertical shift parameter (controls ATM variance level)
    pub a: f64,
    /// Slope factor (controls overall variance level)
    pub b: f64,
    /// Asymmetry parameter (skew, must be in (-1, 1))
    pub rho: f64,
    /// Horizontal shift parameter (ATM location)
    pub m: f64,
    /// Curvature parameter (controls smile curvature, must be > 0)
    pub sigma: f64,
}

/// Helper function to validate SVI parameters for mathematical and no-arbitrage constraints.
fn validate_svi_params(t: f64, a: f64, b: f64, rho: f64, m: f64, sigma: f64) -> Result<()> {
    if t <= 0.0 || !t.is_finite() {
        return Err(anyhow!(
            "SVIParams validation: time to expiry (t={}) must be > 0 and finite",
            t
        ));
    }
    // Allow negative values for 'a' (vertical shift) provided they are finite. The
    // no-arbitrage constraint below will ensure that the combination of parameters
    // still yields non-negative total variance across strikes.
    if !a.is_finite() {
        return Err(anyhow!(
            "SVIParams validation: parameter a (a={}) must be finite",
            a
        ));
    }
    if b <= 0.0 || !b.is_finite() {
        return Err(anyhow!(
            "SVIParams validation: parameter b (b={}) must be > 0 and finite",
            b
        ));
    }
    if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
        return Err(anyhow!(
            "SVIParams validation: parameter rho (rho={}) must be in (-1, 1) and finite",
            rho
        ));
    }
    if !m.is_finite() {
        return Err(anyhow!(
            "SVIParams validation: parameter m (m={}) must be finite",
            m
        ));
    }
    if sigma <= 0.0 || !sigma.is_finite() {
        return Err(anyhow!(
            "SVIParams validation: parameter sigma (sigma={}) must be > 0 and finite",
            sigma
        ));
    }

    // Additional no-arbitrage constraint: a + b*sigma*sqrt(1-rho^2) >= 0
    // This ensures the total variance is non-negative at the wings
    let min_variance = a + b * sigma * (1.0 - rho * rho).sqrt();
    if min_variance < 0.0 {
        return Err(anyhow!(
            "SVIParams validation: no-arbitrage constraint violated. a + b*sigma*sqrt(1-rho^2) = {} < 0", 
            min_variance
        ));
    }

    Ok(())
}

impl SVIParams {
    /// Creates new SVI parameters with validation.
    pub fn new(t: f64, a: f64, b: f64, rho: f64, m: f64, sigma: f64) -> Result<Self> {
        validate_svi_params(t, a, b, rho, m, sigma)?;

        Ok(Self {
            t,
            a,
            b,
            rho,
            m,
            sigma,
        })
    }

    /// Validates the current parameter set.
    pub fn validate(&self) -> Result<()> {
        validate_svi_params(self.t, self.a, self.b, self.rho, self.m, self.sigma)
    }
}

/// Represents the SVI volatility model for a single maturity slice.
/// Contains the parameters and implements the SVI calculation logic.
#[derive(Debug, Clone, PartialEq)]
pub struct SVISlice {
    pub params: SVIParams,
}

impl SVISlice {
    /// Creates a new SVISlice instance from validated SVIParams.
    pub fn new(params: SVIParams) -> Self {
        // Validation should happen when creating SVIParams
        Self { params }
    }

    /// Calculates total variance w(k) using the SVI formula:
    /// w(k) = a + b * (ρ(k-m) + sqrt((k-m)² + σ²))
    pub fn total_variance_at_k(&self, k: f64) -> f64 {
        let k_minus_m = k - self.params.m;
        let sqrt_term = (k_minus_m * k_minus_m + self.params.sigma * self.params.sigma).sqrt();

        self.params.a + self.params.b * (self.params.rho * k_minus_m + sqrt_term)
    }

    /// Calculates implied volatility σ(k) from total variance.
    pub fn implied_vol(&self, k: f64) -> f64 {
        let total_var = self.total_variance_at_k(k);
        if total_var <= 0.0 {
            return 1e-6; // Small positive number to avoid issues
        }
        (total_var / self.params.t).sqrt()
    }
}

// Define the 5-minute tolerance in years as a constant (matching Wing implementation)
const FIVE_MINUTES_IN_YEARS: f64 = 5.0 / (60.0 * 24.0 * 365.0); // approx 9.51e-6

// Implement the SurfaceModel trait for SVISlice
impl SurfaceModel for SVISlice {
    type Parameters = SVIParams;

    fn parameters(&self) -> &Self::Parameters {
        &self.params
    }

    /// Validates the internal parameters using the shared helper function.
    fn validate_params(&self) -> Result<()> {
        validate_svi_params(
            self.params.t,
            self.params.a,
            self.params.b,
            self.params.rho,
            self.params.m,
            self.params.sigma,
        )
    }

    /// Calculates the model's implied total variance.
    /// **Requires `t` to be within ~5 minutes of the slice's `params.t`.**
    fn total_variance(&self, k: f64, t: f64) -> Result<f64> {
        // Check if the provided time `t` is close enough to the slice's time `self.params.t`
        if (t - self.params.t).abs() > FIVE_MINUTES_IN_YEARS {
            return Err(anyhow!(
                "SVISlice time mismatch: requested t={} is too far from slice t={}. Tolerance: {:.3e} years (~5 min)", 
                t, self.params.t, FIVE_MINUTES_IN_YEARS
            ));
        }
        if !k.is_finite() {
            return Err(anyhow!("Log-moneyness k must be finite (k={})", k));
        }

        let total_var = self.total_variance_at_k(k);

        if !total_var.is_finite() || total_var < 0.0 {
            return Err(anyhow!(
                "Calculated total variance is invalid: {} for k={}, t={}",
                total_var,
                k,
                self.params.t
            ));
        }
        Ok(total_var)
    }

    /// Checks for calendar spread arbitrage. Returns Ok(()) as it's not applicable for a single slice.
    fn check_calendar_arbitrage(&self, _k: f64, _t1: f64, _t2: f64) -> Result<()> {
        Ok(())
    }

    /// Checks for butterfly spread arbitrage violations at `k` and `t`.
    /// Uses Gatheral's g(k) condition: g(k) = (1 - k*w'/(2*w))² - (w')²/4 * (1/w + 1/4) + w''/2 >= 0
    /// **Requires `t` to be within ~5 minutes of the slice's `params.t`.**
    fn check_butterfly_arbitrage_at_k(&self, k: f64, t: f64) -> Result<()> {
        const EPSILON: f64 = 1e-5;
        let tolerance = 1e-9; // Tolerance for g_k check

        // Check if the provided time `t` is close enough to the slice's time `self.params.t`
        if (t - self.params.t).abs() > FIVE_MINUTES_IN_YEARS {
            return Err(anyhow!(
                "SVISlice time mismatch for butterfly check: requested t={} is too far from slice t={}. Tolerance: {:.3e} years (~5 min)", 
                t, self.params.t, FIVE_MINUTES_IN_YEARS
            ));
        }
        if !k.is_finite() {
            return Err(anyhow!(
                "Butterfly check failed: k must be finite (k={})",
                k
            ));
        }

        // Use slice's exact time for consistency
        let slice_t = self.params.t;
        let w = self.total_variance(k, slice_t)?;
        let w_p = self.total_variance(k - EPSILON, slice_t)?;
        let w_n = self.total_variance(k + EPSILON, slice_t)?;

        if w <= tolerance {
            return Ok(()); // No arbitrage if variance is near zero
        }

        // Calculate first and second derivatives using finite differences
        let w_k = (w_n - w_p) / (2.0 * EPSILON); // First derivative
        let w_kk = (w_n - 2.0 * w + w_p) / (EPSILON * EPSILON); // Second derivative

        // Gatheral's g(k) condition
        let term1 = 1.0 - k * w_k / (2.0 * w);
        let g_k = term1 * term1 - (w_k * w_k / 4.0) * (1.0 / w + 0.25) + w_kk / 2.0;

        if g_k < -tolerance {
            Err(anyhow!(
                "Butterfly arbitrage detected at k={:.6}, t={:.4}. g(k) = {:.6e} < 0",
                k,
                t,
                g_k
            ))
        } else {
            Ok(())
        }
    }
}

/// Interpolates SVIParams across maturities using linear interpolation.
/// Returns SVIParams for the requested time `t`.
pub fn interpolate_svi_params(slices: &[(f64, SVIParams)], t: f64) -> SVIParams {
    if slices.is_empty() {
        panic!("Cannot interpolate SVI parameters with empty slice list");
    }

    // Clamp t to the range of provided slice times
    let t_clamped = t.clamp(slices[0].0, slices.last().unwrap().0);

    // If t matches an existing slice time exactly, return that slice's params
    if let Some((_, params)) = slices
        .iter()
        .find(|(slice_t, _)| (*slice_t - t_clamped).abs() < 1e-9)
    {
        return params.clone();
    }

    // Find the bounding slices
    let idx = slices.partition_point(|(slice_t, _)| *slice_t < t_clamped);
    if idx == 0 {
        return slices[0].1.clone();
    }
    if idx >= slices.len() {
        return slices.last().unwrap().1.clone();
    }

    let (t0, params0) = &slices[idx - 1];
    let (t1, params1) = &slices[idx];

    // Linear interpolation weight
    let weight1 = (t_clamped - t0) / (t1 - t0);
    let weight0 = 1.0 - weight1;

    // Interpolate each parameter
    let a_interp = weight0 * params0.a + weight1 * params1.a;
    let b_interp = weight0 * params0.b + weight1 * params1.b;
    let rho_interp = weight0 * params0.rho + weight1 * params1.rho;
    let m_interp = weight0 * params0.m + weight1 * params1.m;
    let sigma_interp = weight0 * params0.sigma + weight1 * params1.sigma;

    // Attempt to create new SVIParams with interpolated values
    // Clamp values to ensure they remain valid
    SVIParams::new(
        t_clamped,
        a_interp,                        // Allow negative values now
        b_interp.max(1e-6),              // Ensure b > 0
        rho_interp.clamp(-0.999, 0.999), // Ensure rho in (-1, 1)
        m_interp,                        // m can be any finite value
        sigma_interp.max(1e-6),          // Ensure sigma > 0
    )
    .unwrap_or_else(|err| {
        // Fallback if interpolation results in invalid params
        eprintln!(
            "Warning: SVI interpolation failed for t={}: {}. Falling back to nearest slice.",
            t_clamped, err
        );
        if (t_clamped - t0) < (t1 - t_clamped) {
            params0.clone()
        } else {
            params1.clone()
        }
    })
}

/// Represents the full SVI volatility surface across multiple maturities.
#[derive(Debug, Clone)]
pub struct SVIModel {
    // Slices sorted by time t
    slices: Vec<(f64, SVIParams)>,
    // Configurable tolerance for calendar arbitrage checks
    calendar_arbitrage_tolerance: f64,
}

impl SVIModel {
    /// Creates a new SVIModel (surface) from a vector of (time, params) tuples.
    /// Sorts the slices by time and performs initial validation.
    pub fn new(
        mut slices: Vec<(f64, SVIParams)>,
        calendar_arbitrage_tolerance: f64,
    ) -> Result<Self> {
        if slices.is_empty() {
            return Err(anyhow!("SVIModel requires at least one slice"));
        }

        // Sort slices by time t
        slices.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Ensure times are unique and increasing
        for i in 0..(slices.len() - 1) {
            if (slices[i + 1].0 - slices[i].0).abs() < 1e-9 {
                return Err(anyhow!("Duplicate slice time detected: {}", slices[i].0));
            }
        }

        let model = Self {
            slices,
            calendar_arbitrage_tolerance,
        };

        // Perform initial validation of the surface
        model.validate_params()?;
        Ok(model)
    }

    /// Interpolates SVI parameters for a given time `t`.
    fn interpolate_params(&self, t: f64) -> SVIParams {
        interpolate_svi_params(&self.slices, t)
    }
}

// Implement SurfaceModel for the SVIModel (Surface)
impl SurfaceModel for SVIModel {
    type Parameters = Vec<(f64, SVIParams)>;

    fn parameters(&self) -> &Self::Parameters {
        &self.slices
    }

    /// Validates parameters and checks calendar arbitrage across slices.
    fn validate_params(&self) -> Result<()> {
        if self.slices.is_empty() {
            return Ok(());
        }

        // 1. Validate parameters for each individual slice
        for (t, params) in &self.slices {
            let slice = SVISlice::new(params.clone());
            slice
                .validate_params()
                .map_err(|e| anyhow!("Invalid parameters for slice at t={}: {}", t, e))?;
        }

        // 2. Check for calendar arbitrage between adjacent slices
        // We sample a few k points and ensure total variance is non-decreasing
        if self.slices.len() > 1 {
            let k_samples = vec![-0.5, -0.2, 0.0, 0.2, 0.5]; // Sample points

            for i in 0..(self.slices.len() - 1) {
                let (t1, params1) = &self.slices[i];
                let (t2, params2) = &self.slices[i + 1];

                let slice1 = SVISlice::new(params1.clone());
                let slice2 = SVISlice::new(params2.clone());

                for &k in &k_samples {
                    let w1 = slice1.total_variance_at_k(k);
                    let w2 = slice2.total_variance_at_k(k);

                    if w2 < w1 - self.calendar_arbitrage_tolerance {
                        eprintln!("Warning: Calendar arbitrage detected between t1={:.4} and t2={:.4} at k={:.3}. w1={:.6}, w2={:.6}", 
                                t1, t2, k, w1, w2);
                        // Note: We print a warning but don't return an error to allow flexibility
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculates total variance by interpolating parameters for time `t`.
    fn total_variance(&self, k: f64, t: f64) -> Result<f64> {
        let mut interpolated_params = self.interpolate_params(t);
        // Set the time `t` on the interpolated parameters to the requested time
        interpolated_params.t = t;

        let temp_slice = SVISlice::new(interpolated_params);
        temp_slice.total_variance(k, t)
    }

    /// Checks calendar arbitrage between two times at a given k.
    fn check_calendar_arbitrage(&self, k: f64, t1: f64, t2: f64) -> Result<()> {
        if t1 >= t2 {
            return Err(anyhow!(
                "Calendar check requires t1 < t2, got t1={}, t2={}",
                t1,
                t2
            ));
        }

        let w1 = self.total_variance(k, t1)?;
        let w2 = self.total_variance(k, t2)?;

        if w2 < w1 - self.calendar_arbitrage_tolerance {
            Err(anyhow!(
                "Calendar arbitrage detected at k={:.6}: w(t1={:.4})={:.6} > w(t2={:.4})={:.6}",
                k,
                t1,
                w1,
                t2,
                w2
            ))
        } else {
            Ok(())
        }
    }

    /// Checks butterfly arbitrage at a specific k and t by creating a temporary slice.
    fn check_butterfly_arbitrage_at_k(&self, k: f64, t: f64) -> Result<()> {
        let mut interpolated_params = self.interpolate_params(t);
        interpolated_params.t = t;

        let temp_slice = SVISlice::new(interpolated_params);
        temp_slice.check_butterfly_arbitrage_at_k(k, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create valid test parameters
    fn create_test_svi_params() -> SVIParams {
        SVIParams::new(
            0.25, // t = 3 months
            0.04, // a = 4% base variance level
            0.2,  // b = slope factor
            -0.3, // rho = negative skew
            0.0,  // m = ATM at log-moneyness 0
            0.2,  // sigma = curvature
        )
        .unwrap()
    }

    #[test]
    fn test_svi_params_validation() {
        // Valid parameters should work
        let valid_params = SVIParams::new(0.25, 0.04, 0.2, -0.3, 0.0, 0.2);
        assert!(valid_params.is_ok());

        // Test invalid parameters
        assert!(SVIParams::new(-0.1, 0.04, 0.2, -0.3, 0.0, 0.2).is_err()); // negative t
        assert!(SVIParams::new(0.25, 0.04, -0.1, -0.3, 0.0, 0.2).is_err()); // negative b
        assert!(SVIParams::new(0.25, 0.04, 0.2, -1.0, 0.0, 0.2).is_err()); // rho <= -1
        assert!(SVIParams::new(0.25, 0.04, 0.2, 1.0, 0.0, 0.2).is_err()); // rho >= 1
        assert!(SVIParams::new(0.25, 0.04, 0.2, -0.3, 0.0, -0.1).is_err()); // negative sigma
    }

    #[test]
    fn test_svi_total_variance_calculation() {
        let params = create_test_svi_params();
        let slice = SVISlice::new(params.clone());

        // Test ATM (k=0)
        let w_atm = slice.total_variance_at_k(0.0);
        let expected_atm = params.a + params.b * params.sigma; // rho*0 + sqrt(0^2 + sigma^2) = sigma
        assert!((w_atm - expected_atm).abs() < 1e-10);

        // Test positive k
        let k_pos = 0.2;
        let w_pos = slice.total_variance_at_k(k_pos);
        let expected_pos = params.a
            + params.b
                * (params.rho * k_pos + (k_pos * k_pos + params.sigma * params.sigma).sqrt());
        assert!((w_pos - expected_pos).abs() < 1e-10);
    }

    #[test]
    fn test_svi_implied_volatility() {
        let params = create_test_svi_params();
        let slice = SVISlice::new(params);

        // Test that implied volatility is positive and reasonable
        let iv_atm = slice.implied_vol(0.0);
        assert!(iv_atm > 0.0);
        assert!(iv_atm < 10.0); // Sanity check

        let iv_otm_call = slice.implied_vol(0.3);
        assert!(iv_otm_call > 0.0);

        let iv_otm_put = slice.implied_vol(-0.3);
        assert!(iv_otm_put > 0.0);

        // With negative rho, we expect some skew (put vol > call vol for same |k|)
        assert!(iv_otm_put > iv_otm_call);
    }
}
