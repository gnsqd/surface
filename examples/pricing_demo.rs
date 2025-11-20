// examples/pricing_demo.rs

//! Demonstration of SVI model calibration and option pricing
//!
//! This example shows how to:
//! 1. Load market data
//! 2. Calibrate SVI model parameters
//! 3. Use the calibrated parameters to price options
//! 4. Compare model prices and implied volatilities with market data

use anyhow::Result;
use surface_lib::{
    calibrate_svi, default_configs, models::svi::svi_model::SVIParams, price_with_svi,
    CalibrationParams, FixedParameters, MarketDataRow,
};

fn main() -> Result<()> {
    println!("SVI Model Calibration and Pricing Demo");
    println!("=====================================");

    // Create some synthetic market data for demonstration
    let market_data = create_demo_data();

    println!("Market data loaded: {} options", market_data.len());
    println!(
        "Expiration: 10 days ({:.4} years)",
        market_data[0].years_to_exp
    );
    println!("Underlying price: ${:.0}", market_data[0].underlying_price);

    // Use fast configuration for the demo
    let config = default_configs::fast();

    println!("\nStep 1: Calibrating SVI model...");

    // Calibrate the SVI model
    let calib_params = CalibrationParams::default();
    let calibration_result = calibrate_svi(market_data.clone(), config, calib_params, None)?;
    let (objective, best_params, _used_bounds) = calibration_result;

    println!("Calibration completed!");
    println!("  Objective value: {:.6}", objective);
    println!("  SVI parameters:");
    println!("    a (base variance): {:.6}", best_params[0]);
    println!("    b (slope factor):  {:.6}", best_params[1]);
    println!("    rho (asymmetry):   {:.6}", best_params[2]);
    println!("    m (ATM shift):     {:.6}", best_params[3]);
    println!("    sigma (curvature): {:.6}", best_params[4]);

    println!("\nStep 2: Pricing options with calibrated model...");

    // Create SVIParams from calibrated parameters
    let time_to_exp = market_data[0].years_to_exp;
    let svi_params = SVIParams {
        t: time_to_exp,
        a: best_params[0],
        b: best_params[1],
        rho: best_params[2],
        m: best_params[3],
        sigma: best_params[4],
    };

    // Define fixed parameters
    let fixed_params = FixedParameters {
        r: 0.02, // 2% risk-free rate
        q: 0.0,  // No dividend yield
    };

    // Price all options
    let pricing_results = price_with_svi(svi_params, market_data, fixed_params);

    println!("Options priced: {}", pricing_results.len());
    println!("\nPricing Results:");
    println!(
        "{:<8} {:<8} {:<12} {:<12}",
        "Type", "Strike", "Model IV", "Model Price"
    );
    println!("{}", "-".repeat(50));

    // Display first 10 results
    for result in pricing_results.iter().take(10) {
        println!(
            "{:<8} {:<8.0} {:<12.4} {:<12.2}",
            result.option_type, result.strike_price, result.model_iv, result.model_price
        );
    }

    // Calculate statistics
    let avg_price: f64 =
        pricing_results.iter().map(|r| r.model_price).sum::<f64>() / pricing_results.len() as f64;

    println!("\nSummary Statistics:");
    println!("  Average option price: ${:.2}", avg_price);
    println!(
        "  All options successfully priced: {}",
        pricing_results.iter().all(|r| r.model_price > 0.0)
    );

    Ok(())
}

/// Create synthetic market data for demonstration
fn create_demo_data() -> Vec<MarketDataRow> {
    let underlying_price = 94109.0;
    let years_to_exp = 0.0274; // ~10 days
    let expiration = 1735804800; // Jan 10, 2025

    // Define strikes and their market IVs (decreasing volatility smile)
    let option_data = vec![
        (75000.0, 0.77, "call"),
        (75000.0, 0.77, "put"),
        (80000.0, 0.65, "call"),
        (80000.0, 0.65, "put"),
        (85000.0, 0.58, "call"),
        (85000.0, 0.58, "put"),
        (90000.0, 0.52, "call"),
        (90000.0, 0.52, "put"),
        (94000.0, 0.48, "call"),
        (94000.0, 0.48, "put"), // Near ATM
        (95000.0, 0.49, "call"),
        (95000.0, 0.49, "put"),
        (100000.0, 0.52, "call"),
        (100000.0, 0.52, "put"),
        (105000.0, 0.56, "call"),
        (105000.0, 0.56, "put"),
        (110000.0, 0.62, "call"),
        (110000.0, 0.62, "put"),
    ];

    option_data
        .into_iter()
        .map(|(strike, iv, option_type)| {
            MarketDataRow {
                option_type: option_type.to_string(),
                strike_price: strike,
                underlying_price,
                years_to_exp,
                market_iv: iv,
                vega: 50.0, // Simplified vega
                expiration,
            }
        })
        .collect()
}
