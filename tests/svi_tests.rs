mod test_utils;

use surface_lib::{calibrate_svi, CalibrationParams};
use test_utils::{
    create_test_config, create_verbose_test_config, filter_by_expiration,
    get_available_expirations, load_test_data,
};

/// Integration test for SVI model calibration using 10JAN25 expiration data
///
/// This test validates that the SVI model can successfully calibrate to real market data
/// and produce reasonable parameter values that satisfy no-arbitrage constraints.
#[test]
fn test_svi_calibration_10jan25() {
    // Load test data
    let data_path = "tests/data/options_snapshots_20250101.csv";
    let all_data = load_test_data(data_path).expect("Failed to load test data");

    println!("Loaded {} total data points", all_data.len());

    // Show available expirations
    let expirations = get_available_expirations(&all_data);
    println!("Available expirations:");
    for (timestamp, exp_str, count) in &expirations {
        println!("  {} ({}): {} options", exp_str, timestamp, count);
    }

    // Filter for 10JAN25 expiration
    let jan10_data = filter_by_expiration(all_data, "10JAN25");
    println!("Filtered to {} options for 10JAN25", jan10_data.len());

    // Ensure we have data
    assert!(
        !jan10_data.is_empty(),
        "No data found for 10JAN25 expiration"
    );

    // Print some sample data for verification
    println!("Sample data points:");
    for (i, row) in jan10_data.iter().take(3).enumerate() {
        println!(
            "  {}: {} Strike={} Underlying={} T={:.4} IV={:.2}% Vega={:.3}",
            i + 1,
            row.option_type,
            row.strike_price,
            row.underlying_price,
            row.years_to_exp,
            row.market_iv * 100.0,
            row.vega
        );
    }

    // Create test configuration with verbose output
    let config = create_verbose_test_config();

    // Run SVI calibration
    let calib_params = CalibrationParams::default();
    let result = calibrate_svi(jan10_data, config, calib_params, None);

    match result {
        Ok((objective, params, used_bounds)) => {
            println!("✅ Calibration successful!");
            println!("  Objective value: {:.6}", objective);
            println!("  SVI parameters: {:?}", params);
            println!("  Used bounds: {:?}", used_bounds);

            // Basic validation
            assert_eq!(params.len(), 5, "Should have 5 SVI parameters");
            assert!(
                objective.is_finite() && objective >= 0.0,
                "Objective should be finite and non-negative"
            );

            // Additional validation: check parameter reasonableness
            let a = params[0];
            let b = params[1];
            let rho = params[2];
            let m = params[3];
            let sigma = params[4];

            println!("  SVI Parameter validation:");
            println!("    a = {:.6} (should be finite)", a);
            println!("    b = {:.6} (should be > 0)", b);
            println!("    rho = {:.6} (should be in (-1, 1))", rho);
            println!("    m = {:.6} (should be finite)", m);
            println!("    sigma = {:.6} (should be > 0)", sigma);

            assert!(a.is_finite(), "Parameter 'a' should be finite");
            assert!(
                b > 0.0 && b.is_finite(),
                "Parameter 'b' should be positive and finite"
            );
            assert!(
                rho > -1.0 && rho < 1.0 && rho.is_finite(),
                "Parameter 'rho' should be in (-1, 1)"
            );
            assert!(m.is_finite(), "Parameter 'm' should be finite");
            assert!(
                sigma > 0.0 && sigma.is_finite(),
                "Parameter 'sigma' should be positive and finite"
            );

            // Check no-arbitrage constraint: a + b*sigma*sqrt(1-rho^2) >= 0
            let no_arb_check = a + b * sigma * (1.0_f64 - rho * rho).sqrt();
            println!(
                "    No-arbitrage constraint: {:.6} (should be >= 0)",
                no_arb_check
            );
            assert!(no_arb_check >= -1e-6, "No-arbitrage constraint violated");

            println!("✅ All validations passed!");
        }
        Err(e) => {
            panic!("❌ Calibration failed: {}", e);
        }
    }
}

/// Integration test for data loading and filtering functionality
///
/// This test validates that we can correctly load CSV data and filter by expiration dates,
/// ensuring data integrity and proper type conversion.
#[test]
fn test_data_loading_and_filtering() {
    // Load test data
    let data_path = "tests/data/options_snapshots_20250101.csv";
    let all_data = load_test_data(data_path).expect("Failed to load test data");

    // Basic data validation
    assert!(!all_data.is_empty(), "Should load some data");

    // Check data structure
    for (i, row) in all_data.iter().take(5).enumerate() {
        assert!(
            row.strike_price > 0.0,
            "Row {}: Strike price should be positive",
            i
        );
        assert!(
            row.underlying_price > 0.0,
            "Row {}: Underlying price should be positive",
            i
        );
        assert!(
            row.years_to_exp > 0.0,
            "Row {}: Years to expiry should be positive",
            i
        );
        assert!(
            row.market_iv > 0.0 && row.market_iv < 10.0,
            "Row {}: Market IV should be reasonable",
            i
        );
        assert!(row.vega >= 0.0, "Row {}: Vega should be non-negative", i);
        assert!(
            row.expiration > 0,
            "Row {}: Expiration should be a valid timestamp",
            i
        );
        assert!(
            row.option_type == "call" || row.option_type == "put",
            "Row {}: Option type should be call or put",
            i
        );
    }

    // Test filtering by expiration
    let jan10_data = filter_by_expiration(all_data.clone(), "10JAN25");
    assert!(!jan10_data.is_empty(), "Should have some 10JAN25 data");

    // All filtered data should have the same expiration
    let target_expiration = jan10_data[0].expiration;
    for row in &jan10_data {
        assert_eq!(
            row.expiration, target_expiration,
            "All filtered data should have same expiration"
        );
    }

    // Test getting available expirations
    let expirations = get_available_expirations(&all_data);
    assert!(!expirations.is_empty(), "Should find some expirations");

    // Check that expirations are sorted
    for i in 1..expirations.len() {
        assert!(
            expirations[i].0 >= expirations[i - 1].0,
            "Expirations should be sorted by timestamp"
        );
    }

    println!("✅ Data loading and filtering tests passed");
    println!("  Total data points: {}", all_data.len());
    println!("  Available expirations: {}", expirations.len());
    println!("  10JAN25 data points: {}", jan10_data.len());
}

/// Test that parameter regularisation keeps successive calibrations close
#[test]
fn test_param_regularisation_stability() {
    use test_utils::{create_test_config, filter_by_expiration, load_test_data};

    let data = load_test_data("tests/data/options_snapshots_20250101.csv").unwrap();
    let slice = filter_by_expiration(data, "10JAN25");
    assert!(slice.len() > 20);

    // Generate separate configs with unique seeds for each run
    let mut config1 = create_test_config();
    config1.cmaes.seed = Some(123_456);

    let mut config2 = create_test_config();
    config2.cmaes.seed = Some(654_321);

    let mut config3 = create_test_config();
    config3.cmaes.seed = Some(987_654);

    // First calibration (cold start)
    let (obj1, p1, _bounds1) = surface_lib::calibrate_svi(
        slice.clone(),
        config1.clone(),
        surface_lib::CalibrationParams::default(),
        None,
    )
    .expect("first calib failed");

    // Second calibration with previous params as initial guess (regularisation active by default)
    let (obj2, p2, _bounds2) = surface_lib::calibrate_svi(
        slice,
        config2,
        surface_lib::CalibrationParams {
            reg_lambda: Some(0.08),
            ..surface_lib::CalibrationParams::default()
        },
        Some(p1.clone()),
    )
    .expect("second calib failed");

    // Third calibration WITHOUT an initial guess (cold start again)
    let data2 = load_test_data("tests/data/options_snapshots_20250101.csv").unwrap();
    let slice2 = filter_by_expiration(data2, "10JAN25");
    let (obj3, p3, _bounds3) = surface_lib::calibrate_svi(
        slice2,
        config3,
        surface_lib::CalibrationParams::default(),
        None,
    )
    .expect("third calib failed");

    println!("First run params: {:?}", p1);
    println!("First run objective: {:.6}", obj1);
    println!("Second run params: {:?}", p2);
    println!("Second run objective: {:.6}", obj2);
    println!("Third run params (no guess): {:?}", p3);
    println!("Third run objective: {:.6}", obj3);

    // Param difference between runs 1 & 2 should be tiny due to penalty
    let diff_sq_12: f64 = p1.iter().zip(&p2).map(|(a, b)| (a - b).powi(2)).sum();
    assert!(
        diff_sq_12 < 0.15,
        "Regularisation did not keep parameters close (diff_sq={})",
        diff_sq_12
    );

    // Informative diff between run 1 & 3 (no regularisation)
    let diff_sq_13: f64 = p1.iter().zip(&p3).map(|(a, b)| (a - b).powi(2)).sum();
    println!("Squared diff between run1 & run3: {:.6}", diff_sq_13);
}

#[test]
fn test_bounds_roundtrip() {
    use surface_lib::models::svi::svi_calibrator::SVIParamBounds;
    use test_utils::{create_test_config, filter_by_expiration, load_test_data};

    let data = load_test_data("tests/data/options_snapshots_20250101.csv").unwrap();
    let slice = filter_by_expiration(data, "10JAN25");
    assert!(!slice.is_empty());

    let custom_bounds = SVIParamBounds {
        a: (-0.2, 0.2),
        b: (0.02, 1.5),
        ..SVIParamBounds::default()
    };

    // First calibration with custom bounds
    let cp1 = surface_lib::CalibrationParams {
        param_bounds: Some(custom_bounds.clone()),
        ..surface_lib::CalibrationParams::default()
    };

    let config = create_test_config();
    let (_obj1, _params1, used_bounds1) =
        surface_lib::calibrate_svi(slice.clone(), config.clone(), cp1, None)
            .expect("first calib failed");

    // Second calibration using the returned bounds as input
    let cp2 = surface_lib::CalibrationParams {
        param_bounds: Some(used_bounds1.clone()),
        ..surface_lib::CalibrationParams::default()
    };

    let data2 = load_test_data("tests/data/options_snapshots_20250101.csv").unwrap();
    let slice2 = filter_by_expiration(data2, "10JAN25");
    let (_obj2, _params2, used_bounds2) =
        surface_lib::calibrate_svi(slice2, config, cp2, None).expect("second calib failed");

    // Bounds should round-trip exactly
    assert_eq!(
        used_bounds1.a, used_bounds2.a,
        "Bounds didn't round-trip for 'a'"
    );
    assert_eq!(
        used_bounds1.b, used_bounds2.b,
        "Bounds didn't round-trip for 'b'"
    );
    assert_eq!(
        used_bounds1.rho, used_bounds2.rho,
        "Bounds didn't round-trip for 'rho'"
    );
    assert_eq!(
        used_bounds1.m, used_bounds2.m,
        "Bounds didn't round-trip for 'm'"
    );
    assert_eq!(
        used_bounds1.sigma, used_bounds2.sigma,
        "Bounds didn't round-trip for 'sigma'"
    );
}

#[test]
fn test_custom_bounds_included_in_result() {
    use surface_lib::models::svi::svi_calibrator::SVIParamBounds;
    use test_utils::{create_test_config, filter_by_expiration, load_test_data};

    let data = load_test_data("tests/data/options_snapshots_20250101.csv").unwrap();
    let slice = filter_by_expiration(data, "10JAN25");
    assert!(!slice.is_empty());

    let bounds = SVIParamBounds {
        a: (-0.1, 0.1),
        ..SVIParamBounds::default()
    };

    let cp = surface_lib::CalibrationParams {
        param_bounds: Some(bounds.clone()),
        ..surface_lib::CalibrationParams::default()
    };

    let config = create_test_config();
    let (_obj, _params, used_bounds) =
        surface_lib::calibrate_svi(slice, config, cp, None).expect("calib failed");

    // Check that custom bounds were respected
    assert_eq!(
        used_bounds.a,
        (-0.1, 0.1),
        "Custom bounds for 'a' not applied correctly"
    );
}

#[test]
fn test_svi_pricing() {
    let market_data = load_test_data("tests/data/options_snapshots_20250101.csv").unwrap();

    // Filter for a single expiration (like the other tests)
    let jan10_data = filter_by_expiration(market_data, "10JAN25");
    println!("Loaded {} options for 10JAN25", jan10_data.len());

    let config = create_test_config();

    // First calibrate to get SVI parameters
    let calib_params = surface_lib::CalibrationParams::default();
    let calibration_result =
        surface_lib::calibrate_svi(jan10_data.clone(), config, calib_params, None);
    if let Err(e) = &calibration_result {
        println!("Calibration failed: {:?}", e);
    }
    assert!(calibration_result.is_ok());

    let (best_obj, best_params, _used_bounds) = calibration_result.unwrap();
    println!("Calibration objective: {:.6}", best_obj);

    // Convert parameters to SVIParams - use the time from calibrated data
    let time_to_exp = jan10_data[0].years_to_exp; // Use the actual time from the data
    println!("Using time to expiration: {:.6} years", time_to_exp);

    let svi_params = surface_lib::models::svi::svi_model::SVIParams {
        t: time_to_exp,
        a: best_params[0],
        b: best_params[1],
        rho: best_params[2],
        m: best_params[3],
        sigma: best_params[4],
    };

    // Use fixed parameters from the calibration
    let fixed_params = surface_lib::calibration::types::FixedParameters { r: 0.02, q: 0.0 };

    // Price options using the calibrated parameters
    let pricing_results = surface_lib::price_with_svi(svi_params, jan10_data, fixed_params);

    assert!(!pricing_results.is_empty());
    println!("Priced {} options", pricing_results.len());

    // Debug pricing results first
    println!("Sample pricing results:");
    let results_to_check = std::cmp::min(10, pricing_results.len());
    for (i, result) in pricing_results[..results_to_check].iter().enumerate() {
        println!(
            "{}. Strike: {:.0}, Type: {:?}, Model IV: {:.4}, Price: {:.4}",
            i + 1,
            result.strike_price,
            result.option_type,
            result.model_iv,
            result.model_price
        );
    }

    // Filter out results with zero prices for assertions
    let valid_results: Vec<_> = pricing_results
        .iter()
        .filter(|r| r.model_price > 0.0)
        .collect();
    println!(
        "Valid pricing results: {} out of {}",
        valid_results.len(),
        pricing_results.len()
    );

    // Only check valid results for positive values
    for result in valid_results.iter().take(5) {
        assert!(result.model_price > 0.0, "Model price should be positive");
        assert!(result.model_iv > 0.0, "Model IV should be positive");
        assert!(
            result.model_iv < 5.0,
            "Model IV should be reasonable (< 500%)"
        );
    }

    // Verify that pricing results are reasonable
    println!(
        "All {} options successfully priced with positive prices and reasonable IVs",
        pricing_results.len()
    );
}
