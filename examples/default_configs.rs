use surface_lib::{calibrate_svi, default_configs, CalibrationParams, MarketDataRow};

fn main() {
    // Example market data (minimal example)
    let market_data = vec![
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 100.0,
            underlying_price: 100.0,
            years_to_exp: 0.25,
            market_iv: 0.20,
            vega: 10.0,
            expiration: 1736496000,
        },
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 95.0,
            underlying_price: 100.0,
            years_to_exp: 0.25,
            market_iv: 0.22,
            vega: 8.0,
            expiration: 1736496000,
        },
    ];

    println!("Surface-lib Default Configuration Examples\n");

    // 1. Fast configuration for development
    println!("1. Fast Configuration (good for development):");
    let fast_config = default_configs::fast();
    println!("   Max iterations: {}", fast_config.max_iterations);
    println!("   Tolerance: {:.1e}", fast_config.tolerance);
    println!("   Population size: {}", fast_config.pop_size);
    println!("   Max generations: {}", fast_config.max_gen);
    println!(
        "   CMA-ES max evaluations: {}",
        fast_config.cmaes.max_evaluations
    );
    println!("   Use case: Development, quick prototyping\n");

    // 2. Production configuration
    println!("2. Production Configuration (live trading):");
    let prod_config = default_configs::production();
    println!("   Max iterations: {}", prod_config.max_iterations);
    println!("   Tolerance: {:.1e}", prod_config.tolerance);
    println!("   Population size: {}", prod_config.pop_size);
    println!(
        "   Total evaluations budget: {}",
        prod_config.cmaes.total_evals_budget
    );
    println!("   L-BFGS-B enabled: {}", prod_config.cmaes.lbfgsb_enabled);
    println!("   Use case: Live trading, production systems\n");

    // 3. Research configuration
    println!("3. Research Configuration (maximum accuracy):");
    let research_config = default_configs::research();
    println!("   Max iterations: {}", research_config.max_iterations);
    println!("   Tolerance: {:.1e}", research_config.tolerance);
    println!("   Population size: {}", research_config.pop_size);
    println!("   Max generations: {}", research_config.max_gen);
    println!(
        "   BIPOP restarts: {}",
        research_config.cmaes.bipop_restarts
    );
    println!(
        "   Total evaluations budget: {}",
        research_config.cmaes.total_evals_budget
    );
    println!("   Use case: Academic research, backtesting\n");

    // 4. Minimal configuration
    println!("4. Minimal Configuration (quick validation):");
    let minimal_config = default_configs::minimal();
    println!("   Max iterations: {}", minimal_config.max_iterations);
    println!("   Tolerance: {:.1e}", minimal_config.tolerance);
    println!("   Use case: Quick checks, debugging\n");

    // Example calibration using fast config
    println!("Running example calibration with fast config...");
    let calib_params = CalibrationParams::default();
    match calibrate_svi(market_data, fast_config, calib_params, None) {
        Ok((objective, params, _used_bounds)) => {
            println!("✅ Calibration successful!");
            println!("   Objective: {:.6}", objective);
            println!("   Parameters: {:?}", params);
        }
        Err(e) => {
            println!("❌ Calibration failed: {}", e);
        }
    }
}
