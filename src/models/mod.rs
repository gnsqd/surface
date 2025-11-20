pub mod bs;
pub mod linear_iv;
pub mod svi;

/// Common traits used by all surface models
pub mod traits {
    use anyhow::Result;

    /// Surface model trait for implied volatility calculations
    pub trait SurfaceModel {
        type Parameters;

        fn parameters(&self) -> &Self::Parameters;
        fn validate_params(&self) -> Result<()>;
        fn total_variance(&self, k: f64, t: f64) -> Result<f64>;
        fn check_calendar_arbitrage(&self, k: f64, t1: f64, t2: f64) -> Result<()>;
        fn check_butterfly_arbitrage_at_k(&self, k: f64, t: f64) -> Result<()>;
    }
}

/// Utility functions for option pricing and calculations
pub mod utils {
    use crate::models::traits::SurfaceModel;
    use anyhow::{anyhow, Result};

    /// Calculate log-moneyness: ln(K/S)
    pub fn log_moneyness(strike: f64, spot: f64) -> f64 {
        (strike / spot).ln()
    }

    /// Option pricing result
    pub struct OptionPricingResult {
        pub price: f64,
        pub model_iv: f64,
    }

    /// Price an option using a surface model
    pub fn price_option<T: SurfaceModel>(
        option_type: &str,
        strike: f64,
        spot: f64,
        r: f64,
        q: f64,
        t: f64,
        model: &T,
    ) -> Result<OptionPricingResult> {
        let k = log_moneyness(strike, spot);
        let total_var = model.total_variance(k, t)?;

        if total_var <= 0.0 {
            return Err(anyhow!("Non-positive total variance: {}", total_var));
        }

        let model_iv = (total_var / t).sqrt();
        let price = black_scholes_price(option_type, spot, strike, r, q, t, model_iv)?;

        Ok(OptionPricingResult { price, model_iv })
    }

    /// Black-Scholes option pricing
    fn black_scholes_price(
        option_type: &str,
        s: f64,
        k: f64,
        r: f64,
        q: f64,
        t: f64,
        sigma: f64,
    ) -> Result<f64> {
        if sigma <= 0.0 || t <= 0.0 {
            return Err(anyhow!("Invalid parameters: sigma={}, t={}", sigma, t));
        }

        let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();

        let price = match option_type.to_lowercase().as_str() {
            "call" => s * (-q * t).exp() * normal_cdf(d1) - k * (-r * t).exp() * normal_cdf(d2),
            "put" => k * (-r * t).exp() * normal_cdf(-d2) - s * (-q * t).exp() * normal_cdf(-d1),
            _ => return Err(anyhow!("Invalid option type: {}", option_type)),
        };

        Ok(price)
    }

    /// Standard normal cumulative distribution function approximation
    fn normal_cdf(x: f64) -> f64 {
        0.5 * (1.0 + erf(x / 2.0_f64.sqrt()))
    }

    /// Error function approximation
    fn erf(x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

        sign * y
    }
}
