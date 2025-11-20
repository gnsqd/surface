// A minimal Black-Scholes implementation that provides call and put pricing helpers
// required by the calibration pipeline.  Implied-volatility and Greeks are
// intentionally omitted to keep the lightweight focus of surface-lib.

#[allow(non_snake_case)]
fn norm_cdf(x: f64) -> f64 {
    // 0.5 * [1 + erf(x / sqrt(2))]
    0.5 * (1.0 + libm::erf(x / (2.0_f64).sqrt()))
}

/// Price of a European call option under Black-Scholes assumptions.
#[allow(non_snake_case)]
pub fn bs_call_price(S: f64, K: f64, r: f64, q: f64, T: f64, sigma: f64) -> f64 {
    if T <= 0.0 || sigma <= 0.0 {
        return (S * (-q * T).exp() - K * (-r * T).exp()).max(0.0);
    }
    let d1 = ((S / K).ln() + (r - q + 0.5 * sigma.powi(2)) * T) / (sigma * T.sqrt());
    let d2 = d1 - sigma * T.sqrt();
    S * (-q * T).exp() * norm_cdf(d1) - K * (-r * T).exp() * norm_cdf(d2)
}

/// Price of a European put option under Black-Scholes assumptions.
#[allow(non_snake_case)]
pub fn bs_put_price(S: f64, K: f64, r: f64, q: f64, T: f64, sigma: f64) -> f64 {
    if T <= 0.0 || sigma <= 0.0 {
        return (K * (-r * T).exp() - S * (-q * T).exp()).max(0.0);
    }
    let d1 = ((S / K).ln() + (r - q + 0.5 * sigma.powi(2)) * T) / (sigma * T.sqrt());
    let d2 = d1 - sigma * T.sqrt();
    let nd1m = 1.0 - norm_cdf(d1);
    let nd2m = 1.0 - norm_cdf(d2);
    K * (-r * T).exp() * nd2m - S * (-q * T).exp() * nd1m
}
