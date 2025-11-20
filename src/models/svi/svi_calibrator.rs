// src/models/svi/svi_calibrator.rs

//! SVI model calibrator implementation
//!
//! This module implements the calibrator for the SVI (Stochastic Volatility Inspired) model.
//! The calibrator follows the same pattern as other models in the codebase, implementing
//! the ModelCalibrator trait and providing methods for parameter optimization.

use crate::calibration::config::OptimizationConfig;
use crate::calibration::types::{MarketDataRow, ModelCalibrator, PricingResult};
use crate::model_params::{ModelParams, SviModelParams};
use crate::models::svi::svi_model::{SVIParams, SVISlice};
use crate::models::utils::{log_moneyness, price_option, OptionPricingResult};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Structure to hold parameter bounds for the SVI model calibration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SVIParamBounds {
    /// Vertical shift parameter bounds (controls ATM variance level)
    pub a: (f64, f64),
    /// Slope factor bounds (controls overall variance level)
    pub b: (f64, f64),
    /// Asymmetry parameter bounds (skew, must be in (-1, 1))
    pub rho: (f64, f64),
    /// Horizontal shift parameter bounds (ATM location)
    pub m: (f64, f64),
    /// Curvature parameter bounds (controls smile curvature, must be > 0)
    pub sigma: (f64, f64),
}

impl Default for SVIParamBounds {
    fn default() -> Self {
        Self {
            a: (-0.5, 0.5),
            b: (0.01, 2.0),
            rho: (-0.99, -0.01), // Restrict to negative for BTC skew
            m: (-1.0, 1.0),
            sigma: (0.01, 2.0),
        }
    }
}

impl From<&[(f64, f64)]> for SVIParamBounds {
    fn from(bounds: &[(f64, f64)]) -> Self {
        if bounds.len() != 5 {
            return Self::default();
        }
        Self {
            a: bounds[0],
            b: bounds[1],
            rho: bounds[2],
            m: bounds[3],
            sigma: bounds[4],
        }
    }
}

/// Calibrator for the SVI model with 5 parameters per expiry:
/// [a, b, rho, m, sigma]
#[derive(Debug, Clone)]
pub struct SVIModelCalibrator {
    /// Store only the single expiration (timestamp, years_to_exp)
    expiration: (i64, f64),
    /// Parameters for a single slice (length 5)
    param_bounds: Vec<(f64, f64)>,

    /// Model-specific parameters (e.g. ATM boost)
    params: SviModelParams,

    /// Optional previous solution for temporal regularization
    prev_solution: Option<Vec<f64>>,
    temporal_reg_lambda: f64,
}

impl SVIModelCalibrator {
    /// Constructor from market data and configuration parameters.
    pub fn new(
        data: &[MarketDataRow],
        param_bounds_opt: Option<SVIParamBounds>,
        model_params: Option<Box<dyn ModelParams>>, // new optional parameters
    ) -> Result<Self> {
        // Group data by expiration to ensure single expiry requirement
        let mut grouped = HashMap::<i64, Vec<f64>>::new();
        for r in data {
            grouped
                .entry(r.expiration)
                .or_default()
                .push(r.years_to_exp);
        }

        // Ensure exactly one expiration is present
        if grouped.len() != 1 {
            return Err(anyhow!(
                "SVIModelCalibrator requires data for exactly one expiration, but found {}. Expirations: {:?}", 
                grouped.len(), grouped.keys().collect::<Vec<_>>()
            ));
        }

        // Get the single expiration timestamp and calculate average time
        let (single_exp_ts, times) = grouped.into_iter().next().unwrap();
        let single_avg_t = times.iter().copied().sum::<f64>() / times.len() as f64;
        let expiration = (single_exp_ts, single_avg_t);

        let bounds = param_bounds_opt.unwrap_or_default();

        /*
        // Auto-adjust bounds based on time to expiry (adaptive bounds placeholder)
        let bounds = param_bounds.unwrap_or_else(|| {
            let days = single_avg_t * 365.0;
            if days < 1.0 {
                // For intraday options (< 1 day), allow very tight parameters
                SVIParamBounds {
                    a: (-0.1, 0.1),
                    b: (0.001, 0.5),
                    rho: (-0.99, -0.01),
                    m: (-0.5, 0.5),
                    sigma: (0.001, 0.1),
                }
            } else if days < 3.0 {
                // Very short-term (1-3 days): keep m near ATM to avoid extreme shifts
                SVIParamBounds {
                    a: (-0.5, 1.0),
                    b: (0.01, 5.0),
                    rho: (-0.999, -0.01),
                    m: (-0.3, 0.3),
                    sigma: (0.01, 2.0),
                }
            } else if days < 7.0 {
                // Short-term (< 1 week) – moderate m range
                SVIParamBounds {
                    a: (-0.5, 1.0),
                    b: (0.01, 5.0),
                    rho: (-0.999, -0.01),
                    m: (-1.0, 1.0),
                    sigma: (0.01, 2.0),
                }
            } else if days < 30.0 {
                // For medium-term options (< 1 month)
                SVIParamBounds {
                    a: (-0.5, 0.8),
                    b: (0.01, 3.0),
                    rho: (-0.99, -0.01),
                    m: (-1.5, 1.5),
                    sigma: (0.03, 1.0),
                }
            } else {
                // For longer-term options, use default bounds
                SVIParamBounds::default()
            }
        });
        */

        // Fill parameter bounds vector from the struct (5 parameters: a, b, rho, m, sigma)
        let param_bounds = vec![bounds.a, bounds.b, bounds.rho, bounds.m, bounds.sigma];

        // Note: relaxed_bounds removed, using param_bounds directly

        // Resolve model-specific parameters (default if not supplied or type mismatch)
        let params = if let Some(mp) = model_params {
            mp.as_any()
                .downcast_ref::<SviModelParams>()
                .cloned()
                .unwrap_or_default()
        } else {
            SviModelParams::default()
        };

        Ok(Self {
            expiration,
            param_bounds,
            params,
            prev_solution: None,
            temporal_reg_lambda: 0.0,
        })
    }

    pub fn set_prev_solution(&mut self, prev_sol: Vec<f64>) {
        if prev_sol.len() == self.param_count() {
            self.prev_solution = Some(prev_sol);
        }
    }

    pub fn set_temporal_reg_lambda(&mut self, lambda: f64) {
        self.temporal_reg_lambda = lambda.max(0.0);
    }
}

impl ModelCalibrator for SVIModelCalibrator {
    fn model_name(&self) -> &str {
        "svi"
    }

    fn param_count(&self) -> usize {
        self.param_bounds.len() // Should be 5
    }

    fn param_bounds(&self) -> &[(f64, f64)] {
        &self.param_bounds
    }

    /// Evaluate objective function using vega-weighted RMSE on total variance with
    /// an additional exponential ATM weighting.
    /// x is the parameter vector [a, b, rho, m, sigma].
    fn evaluate_objective(&self, x: &[f64], data: &[MarketDataRow]) -> f64 {
        assert_eq!(
            x.len(),
            5,
            "Input parameter vector length must be 5 for SVI model"
        );

        let (exp_ts, t) = self.expiration;

        // 1. Build the SVI slice from the candidate parameters ----------------------------
        let params = match SVIParams::new(t, x[0], x[1], x[2], x[3], x[4]) {
            Ok(p) => p,
            Err(_) => return 1.0e12, // Reject invalid parameter sets outright
        };
        let slice = SVISlice::new(params);

        // 2. Weighted error computation ----------------------------------------------------
        let mut weighted_error_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut valid_points = 0u32;

        for row in data {
            if row.expiration != exp_ts {
                continue; // Keep only this slice's points
            }

            let k = log_moneyness(row.strike_price, row.underlying_price);
            let model_iv = slice.implied_vol(k);
            let market_iv_dec = row.market_iv; // already in decimal form

            // Skip points with non-positive IVs
            if model_iv <= 0.0 || market_iv_dec <= 0.0 {
                continue;
            }

            // Total variance (w = σ² · t) difference – preferred over raw IV diff for
            // short-dated options where IV is highly non-linear in the parameters.
            let model_w = model_iv * model_iv * t;
            let market_w = market_iv_dec * market_iv_dec * t;
            let diff = model_w - market_w;
            let squared_error = diff * diff;

            // --- Weighting scheme --------------------------------------------------------
            // 1. Vega weighting (optional)
            let vega_weight = if self.params.use_vega_weighting {
                if row.vega > 0.0 {
                    row.vega
                } else {
                    1.0
                }
            } else {
                1.0
            };
            // 2. ATM emphasis – exponential decay as |k| grows.
            let atm_weight = (-self.params.atm_boost_factor * k.abs()).exp();
            let weight = vega_weight * atm_weight;

            weighted_error_sum += weight * squared_error;
            weight_sum += weight;
            valid_points += 1;
        }

        if valid_points == 0 || weight_sum <= 1e-12 {
            return 1.0e12; // Fail-safe if no usable points
        }

        // Weighted root-mean-squared error on total variance
        let mut obj = (weighted_error_sum / weight_sum).sqrt();

        // -----------------------------------------------------------------------------------
        // Optional temporal regularisation on raw parameters
        // -----------------------------------------------------------------------------------
        if let (Some(prev), lambda) = (&self.prev_solution, self.temporal_reg_lambda) {
            if lambda > 0.0 && prev.len() == x.len() {
                let penalty: f64 = x
                    .iter()
                    .zip(prev.iter())
                    .map(|(v, p)| (v - p).powi(2))
                    .sum::<f64>()
                    * lambda;
                obj += penalty;
            }
        }
        obj
    }

    // Note: create_param_map removed as param_map is no longer returned from calibration API

    fn price_options(
        &self,
        market_data: &[MarketDataRow],
        best_params: &[f64],
        config: &OptimizationConfig,
    ) -> Vec<PricingResult> {
        assert_eq!(best_params.len(), 5, "Expected 5 parameters for SVI model");
        let (exp_ts, t) = self.expiration;

        // Extract parameters
        let a = best_params[0];
        let b = best_params[1];
        let rho = best_params[2];
        let m = best_params[3];
        let sigma = best_params[4];

        let final_params = match SVIParams::new(t, a, b, rho, m, sigma) {
            Ok(params) => params,
            Err(e) => {
                eprintln!(
                    "Error creating final SVIParams for pricing: {}. Using fallback parameters.",
                    e
                );
                SVIParams::new(0.1, 0.04, 0.2, -0.3, 0.0, 0.2).unwrap() // Fallback
            }
        };
        let final_slice = SVISlice::new(final_params);

        let r = config.fixed_params.r;
        let q = config.fixed_params.q;
        let mut results = Vec::with_capacity(market_data.len());

        for row in market_data {
            // Filter data for the single expiration this calibrator handles
            if row.expiration == exp_ts {
                let t_row = row.years_to_exp;
                let underlying = row.underlying_price;
                let strike = row.strike_price;

                // Price the option using SVI model
                let pricing_result = if underlying > 1e-8 {
                    price_option(
                        &row.option_type,
                        strike,
                        underlying,
                        r,
                        q,
                        t_row,
                        &final_slice,
                    )
                } else {
                    Ok(OptionPricingResult {
                        price: 0.0,
                        model_iv: 0.0,
                    })
                };

                let (model_price, model_iv) = match pricing_result {
                    Ok(pr) => (pr.price, pr.model_iv),
                    Err(e) => {
                        eprintln!(
                            "Error pricing option (exp={}, strike={}): {}",
                            exp_ts, strike, e
                        );
                        (0.0, 0.0)
                    }
                };

                results.push(PricingResult {
                    option_type: row.option_type.clone(),
                    strike_price: row.strike_price,
                    underlying_price: row.underlying_price,
                    years_to_exp: row.years_to_exp,
                    model_price,
                    model_iv,
                });
            }
        }

        results.sort_by(|a, b| a.strike_price.partial_cmp(&b.strike_price).unwrap());
        results
    }

    fn param_names(&self) -> Vec<&str> {
        vec!["a", "b", "rho", "m", "sigma"]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_prev_solution(&mut self, prev_solution: Vec<f64>) {
        self.set_prev_solution(prev_solution);
    }

    fn set_temporal_reg_lambda(&mut self, lambda: f64) {
        self.set_temporal_reg_lambda(lambda);
    }

    // Note: relaxed methods removed as they were redundant

    fn expand_bounds_if_needed(
        &mut self,
        params: &[f64],
        proximity_threshold: f64,
        expansion_factor: f64,
    ) -> bool {
        let mut adjusted = false;
        for (bounds, param) in self.param_bounds.iter_mut().zip(params.iter()) {
            let range = bounds.1 - bounds.0;
            let lower_thresh = bounds.0 + range * proximity_threshold;
            let upper_thresh = bounds.1 - range * proximity_threshold;
            if *param <= lower_thresh {
                let expansion = range * expansion_factor;
                bounds.0 -= expansion;
                adjusted = true;
            }
            if *param >= upper_thresh {
                let expansion = range * expansion_factor;
                bounds.1 += expansion;
                adjusted = true;
            }
            // Note: bounds already updated in-place above
        }
        adjusted
    }
}
