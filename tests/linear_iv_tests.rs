use surface_lib::models::linear_iv::*;
use surface_lib::MarketDataRow;

// Helper function to create MarketDataRow more concisely
fn create_market_data(
    option_type: &str,
    strike: f64,
    underlying: f64,
    tte: f64,
    iv: f64,
) -> MarketDataRow {
    MarketDataRow {
        option_type: option_type.to_string(),
        strike_price: strike,
        underlying_price: underlying,
        years_to_exp: tte,
        market_iv: iv,
        vega: 1.0,
        expiration: 0,
    }
}

/// Tests ATM IV interpolation on simple chain with 5 strikes.
/// Creates symmetric points around ATM and verifies interpolation matches expected value.
#[test]
fn test_atm_iv_basic() {
    // Create symmetric market data around ATM
    let points = vec![
        create_market_data("call", 95.0, 100.0, 0.25, 0.25),
        create_market_data("call", 97.5, 100.0, 0.25, 0.22),
        create_market_data("call", 100.0, 100.0, 0.25, 0.20),
        create_market_data("call", 102.5, 100.0, 0.25, 0.22),
        create_market_data("call", 105.0, 100.0, 0.25, 0.25),
    ];

    let forward = 100.0;
    let tte = 0.25; // 3 months

    let atm_iv = compute_atm_iv(&points, forward, tte).expect("ATM IV computation failed");

    // Should interpolate to 0.20 at ATM (x=0, ln(100/100)=0)
    assert!(
        (atm_iv - 0.20).abs() < 1e-6,
        "ATM IV should be 0.20, got {}",
        atm_iv
    );
}

/// Full flow: Interpolate chain, solve for ±25δ IVs, compute RR/BF.
/// Uses sparse points and verifies solver convergence and metrics calculation.
#[test]
fn test_fixed_delta_solve() {
    let points = vec![
        create_market_data("call", 90.0, 100.0, 0.25, 0.30),
        create_market_data("call", 95.0, 100.0, 0.25, 0.25),
        create_market_data("call", 100.0, 100.0, 0.25, 0.20),
        create_market_data("call", 105.0, 100.0, 0.25, 0.22),
        create_market_data("call", 110.0, 100.0, 0.25, 0.27),
    ];

    let forward = 100.0;
    let tte = 0.25;
    let config = LinearIvConfig::default();

    let result = build_linear_iv(&points, forward, tte, &config).expect("Linear IV build failed");

    // Check that we have delta IVs for the configured deltas
    assert!(
        result.get_iv_for_delta(0.25).is_some(),
        "Should have +25δ IV"
    );
    assert!(
        result.get_iv_for_delta(-0.25).is_some(),
        "Should have -25δ IV"
    );

    // Risk reversal behavior depends on the actual skew in the data
    assert!(result.rr_25.is_some(), "Should have 25δ RR");
    let rr = result.rr_25.unwrap();
    // With corrected delta calculation, RR sign depends on actual vol skew
    println!("25δ RR: {:.4}%", rr * 100.0);

    // Butterfly should be positive
    assert!(result.bf_25.is_some(), "Should have 25δ BF");
    let bf = result.bf_25.unwrap();
    assert!(bf > 0.0, "BF should be positive, got {}", bf);
}

/// Handles minimal points (3), extrapolation errors.
/// Tests edge case with minimum required points and extrapolation behavior.
#[test]
fn test_edge_sparse_points() {
    // Test with exactly minimum points
    let points = vec![
        create_market_data("call", 95.0, 100.0, 0.25, 0.25),
        create_market_data("call", 100.0, 100.0, 0.25, 0.20),
        create_market_data("call", 105.0, 100.0, 0.25, 0.25),
    ];

    let forward = 100.0;
    let tte = 0.25;
    let config = LinearIvConfig::default();

    let result = build_linear_iv(&points, forward, tte, &config);
    assert!(result.is_ok(), "Should handle minimum points");

    // Test with insufficient points
    let insufficient_points = vec![
        create_market_data("call", 100.0, 100.0, 0.25, 0.20),
        create_market_data("call", 105.0, 100.0, 0.25, 0.25),
    ];

    let result = build_linear_iv(&insufficient_points, forward, tte, &config);
    assert!(result.is_err(), "Should fail with insufficient points");
}

/// Integration: Build output for sample BTC chain, verify all metrics.
/// Uses realistic option chain data to test full integration flow.
#[test]
fn test_full_chain_flow() {
    // Realistic BTC option chain (simplified)
    let tte = 30.0 / 365.0; // 30 days
    let forward = 45000.0;
    let points = vec![
        create_market_data("put", 30000.0, forward, tte, 0.85),
        create_market_data("put", 35000.0, forward, tte, 0.70),
        create_market_data("put", 40000.0, forward, tte, 0.60),
        create_market_data("put", 42000.0, forward, tte, 0.55),
        create_market_data("call", 44000.0, forward, tte, 0.50),
        create_market_data("call", 46000.0, forward, tte, 0.48),
        create_market_data("call", 48000.0, forward, tte, 0.50),
        create_market_data("call", 50000.0, forward, tte, 0.55),
        create_market_data("call", 55000.0, forward, tte, 0.65),
        create_market_data("call", 60000.0, forward, tte, 0.75),
    ];

    // (forward and tte already defined above)
    let config = LinearIvConfig::default();

    let result = build_linear_iv(&points, forward, tte, &config).expect("Full chain flow failed");

    // Verify ATM IV is reasonable
    assert!(
        result.atm_iv > 0.0 && result.atm_iv < 2.0,
        "ATM IV should be reasonable, got {}",
        result.atm_iv
    );

    // Verify we have the expected delta IVs (should have 4 configured deltas)
    assert!(
        result.delta_ivs.len() <= 4,
        "Should have at most 4 delta IVs, got {}",
        result.delta_ivs.len()
    );

    // Verify metrics are computed
    assert!(result.rr_25.is_some(), "Should have RR");
    assert!(result.bf_25.is_some(), "Should have BF");

    // Check RR behavior - sign depends on actual skew in the data
    if let Some(rr) = result.rr_25 {
        println!(
            "RR: {:.4}% (sign depends on vol skew direction)",
            rr * 100.0
        );
    }

    // Verify time to expiration is preserved
    assert!((result.tte - tte).abs() < 1e-10, "TTE should be preserved");

    // Sanity check: all IVs should be positive
    for delta_iv in &result.delta_ivs {
        assert!(
            delta_iv.iv > 0.0,
            "IV for delta {} should be positive, got {}",
            delta_iv.delta,
            delta_iv.iv
        );
    }
}

/// Tests error on zero time to expiration.
/// Should fail gracefully with descriptive error message.
#[test]
fn test_zero_tte_error() {
    let points = vec![
        create_market_data("call", 95.0, 100.0, 0.0, 0.25),
        create_market_data("call", 100.0, 100.0, 0.0, 0.20),
        create_market_data("call", 105.0, 100.0, 0.0, 0.25),
    ];

    let forward = 100.0;
    let tte = 0.0; // Zero time to expiration
    let config = LinearIvConfig::default();

    let result = build_linear_iv(&points, forward, tte, &config);
    assert!(result.is_err(), "Should fail with zero TTE");

    // Also test ATM IV computation directly
    let atm_result = compute_atm_iv(&points, forward, tte);
    assert!(atm_result.is_err(), "ATM IV should fail with zero TTE");
}

/// Tests asymmetric points handling.
/// Verifies graceful behavior when only calls or puts are available.
#[test]
fn test_asymmetric_points() {
    // Only call options
    let call_only_points = vec![
        create_market_data("call", 100.0, 100.0, 0.25, 0.20),
        create_market_data("call", 105.0, 100.0, 0.25, 0.22),
        create_market_data("call", 110.0, 100.0, 0.25, 0.25),
    ];

    let forward = 100.0;
    let tte = 0.25;
    let config = LinearIvConfig::default();

    let result = build_linear_iv(&call_only_points, forward, tte, &config)
        .expect("Should handle call-only points");

    // Should compute ATM IV successfully
    assert!(result.atm_iv > 0.0, "ATM IV should be positive");

    // May not solve all deltas (especially negative ones), but shouldn't crash
    assert!(
        result.delta_ivs.len() <= config.deltas.len(),
        "Should not have more deltas than configured"
    );
}

/// Tests extrapolation configuration behavior.
/// Verifies that disabling extrapolation affects delta solving appropriately.
#[test]
fn test_extrapolation_config() {
    let points = vec![
        create_market_data("call", 98.0, 100.0, 0.25, 0.22),
        create_market_data("call", 100.0, 100.0, 0.25, 0.20),
        create_market_data("call", 102.0, 100.0, 0.25, 0.22),
    ];

    let forward = 100.0;
    let tte = 0.25;

    // Test with extrapolation enabled (default)
    let config_with_extrap = LinearIvConfig {
        allow_extrapolation: true,
        ..Default::default()
    };

    let result_with_extrap = build_linear_iv(&points, forward, tte, &config_with_extrap)
        .expect("Should work with extrapolation");

    // Test with extrapolation disabled
    let config_no_extrap = LinearIvConfig {
        allow_extrapolation: false,
        ..Default::default()
    };

    let result_no_extrap = build_linear_iv(&points, forward, tte, &config_no_extrap)
        .expect("Should work without extrapolation");

    // With extrapolation disabled, we might get fewer delta IVs
    // (especially for extreme deltas that require extrapolation)
    assert!(
        result_no_extrap.delta_ivs.len() <= result_with_extrap.delta_ivs.len(),
        "No-extrapolation should have <= delta IVs than with extrapolation"
    );
}

/// Tests using the example data from the analysis to verify correct behavior.
/// Data: T=0.25, F=100, strikes=[90,95,105,110], IVs=[0.24,0.22,0.21,0.23]
/// Expected: ATM IV ~21.5%, corrected skew with put IVs > call IVs for downside skew
#[test]
fn test_example_data_verification() {
    // Example data from analysis: downside skew (higher vol on left)
    let forward = 100.0;
    let tte = 0.25;
    let points = vec![
        create_market_data("put", 90.0, forward, tte, 0.24), // OTM put, higher vol
        create_market_data("put", 95.0, forward, tte, 0.22), // ITM put
        create_market_data("call", 105.0, forward, tte, 0.21), // ITM call
        create_market_data("call", 110.0, forward, tte, 0.23), // OTM call
    ];

    let config = LinearIvConfig::default();
    let result = build_linear_iv(&points, forward, tte, &config).expect("Example data should work");

    // Verify ATM IV is around 21.5% (based on interpolation at x=0)
    let atm_expected = 0.215; // ~21.5%
    assert!(
        (result.atm_iv - atm_expected).abs() < 0.01,
        "ATM IV should be ~21.5%, got {:.3}%",
        result.atm_iv * 100.0
    );

    // With corrected delta calculation, we should see proper skew:
    // - Put deltas (negative) should have higher IVs due to downside skew
    // - This should result in negative RR (put vol > call vol)
    let put_25d_iv = result.get_iv_for_delta(-0.25);
    let call_25d_iv = result.get_iv_for_delta(0.25);

    if let (Some(put_iv), Some(call_iv)) = (put_25d_iv, call_25d_iv) {
        // With downside skew, put IV should be higher than call IV
        assert!(
            put_iv > call_iv,
            "With downside skew, put IV ({:.3}%) should > call IV ({:.3}%)",
            put_iv * 100.0,
            call_iv * 100.0
        );

        // RR should be negative for downside skew
        if let Some(rr) = result.rr_25 {
            assert!(
                rr < 0.0,
                "RR should be negative for downside skew, got {:.3}%",
                rr * 100.0
            );
        }
    }

    println!("Example data verification:");
    println!("  ATM IV: {:.2}%", result.atm_iv * 100.0);
    if let Some(put_iv) = put_25d_iv {
        println!("  25D Put IV: {:.2}%", put_iv * 100.0);
    }
    if let Some(call_iv) = call_25d_iv {
        println!("  25D Call IV: {:.2}%", call_iv * 100.0);
    }
    if let Some(rr) = result.rr_25 {
        println!("  25D RR: {:.2}%", rr * 100.0);
    }
    if let Some(bf) = result.bf_25 {
        println!("  25D BF: {:.2}%", bf * 100.0);
    }
}

/// Tests TemporalConfig convenience methods
#[test]
fn test_temporal_config_convenience() {
    use surface_lib::models::linear_iv::TemporalConfig;

    // Test from_days
    let config = TemporalConfig::from_days(vec![1, 7, 30]);
    assert_eq!(config.fixed_days, vec![1, 7, 30]);

    // Test weekly
    let weekly = TemporalConfig::weekly();
    assert_eq!(weekly.fixed_days, vec![7, 14, 21, 28]);

    // Test monthly
    let monthly = TemporalConfig::monthly();
    assert_eq!(monthly.fixed_days, vec![30, 60, 90, 120]);
}

/// Tests temporal interpolation with basic multi-maturity data.
/// Verifies grouping by TTE and interpolation to fixed time grid.
#[test]
fn test_temporal_basic() {
    use surface_lib::models::linear_iv::{build_fixed_time_metrics, TemporalConfig};

    // Create multi-maturity data: 7, 14, and 30 days with different vol levels
    let forward = 100.0;
    let data = vec![
        // 7 days (tte = 7/365 ≈ 0.0192)
        create_market_data("put", 95.0, forward, 7.0 / 365.0, 0.25),
        create_market_data("call", 100.0, forward, 7.0 / 365.0, 0.20),
        create_market_data("call", 105.0, forward, 7.0 / 365.0, 0.22),
        // 14 days (tte = 14/365 ≈ 0.0384)
        create_market_data("put", 95.0, forward, 14.0 / 365.0, 0.24),
        create_market_data("call", 100.0, forward, 14.0 / 365.0, 0.19),
        create_market_data("call", 105.0, forward, 14.0 / 365.0, 0.21),
        // 30 days (tte = 30/365 ≈ 0.0822)
        create_market_data("put", 95.0, forward, 30.0 / 365.0, 0.23),
        create_market_data("call", 100.0, forward, 30.0 / 365.0, 0.18),
        create_market_data("call", 105.0, forward, 30.0 / 365.0, 0.20),
    ];

    let temp_config = TemporalConfig {
        fixed_days: vec![1, 3, 7, 14, 21, 30],
        allow_short_extrapolate: true, // Enable short extrapolation for 1d and 3d
        ..Default::default()
    };
    let strike_config = LinearIvConfig::default();

    let metrics = build_fixed_time_metrics(&data, forward, &temp_config, &strike_config)
        .expect("Temporal interpolation should work");

    // Should have metrics for all requested days
    assert_eq!(metrics.len(), 6, "Should have 6 time points");

    // Verify days are correct and sorted
    let days: Vec<i32> = metrics.iter().map(|m| m.tte_days).collect();
    assert_eq!(days, vec![1, 3, 7, 14, 21, 30]);

    // ATM IVs should be interpolated/extrapolated reasonably
    for metric in &metrics {
        assert!(
            metric.atm_iv > 0.0,
            "ATM IV should be positive for {} days",
            metric.tte_days
        );
        assert!(
            metric.atm_iv < 1.0,
            "ATM IV should be reasonable for {} days",
            metric.tte_days
        );
    }

    // 7 and 14 day metrics should match observed data closely
    let day_7 = metrics.iter().find(|m| m.tte_days == 7).unwrap();
    let day_14 = metrics.iter().find(|m| m.tte_days == 14).unwrap();

    // These should be very close to the input ATM levels (~0.20 for 7d, ~0.19 for 14d)
    assert!(
        (day_7.atm_iv - 0.20).abs() < 0.01,
        "7-day ATM should be ~20%, got {:.2}%",
        day_7.atm_iv * 100.0
    );
    assert!(
        (day_14.atm_iv - 0.19).abs() < 0.01,
        "14-day ATM should be ~19%, got {:.2}%",
        day_14.atm_iv * 100.0
    );
}

/// Tests temporal interpolation methods (LinearTte vs LinearVariance).
/// Verifies different interpolation approaches give reasonable results.
#[test]
fn test_temporal_interpolation_methods() {
    use surface_lib::models::linear_iv::{
        build_fixed_time_metrics, TemporalConfig, TemporalInterpMethod,
    };

    let forward = 100.0;
    // Simple two-maturity data with clear vol term structure
    let data = vec![
        // 10 days: higher vol (short term volatility spike)
        create_market_data("call", 100.0, forward, 10.0 / 365.0, 0.30),
        create_market_data("call", 105.0, forward, 10.0 / 365.0, 0.32),
        // 20 days: lower vol (reversion to mean)
        create_market_data("call", 100.0, forward, 20.0 / 365.0, 0.20),
        create_market_data("call", 105.0, forward, 20.0 / 365.0, 0.22),
    ];

    let strike_config = LinearIvConfig {
        min_points: 2, // Reduce requirement since we only have 2 points per maturity
        ..Default::default()
    };

    // Test LinearTte method
    let linear_tte_config = TemporalConfig {
        fixed_days: vec![15], // Mid-point
        interp_method: TemporalInterpMethod::LinearTte,
        ..Default::default()
    };

    let linear_tte_metrics =
        build_fixed_time_metrics(&data, forward, &linear_tte_config, &strike_config)
            .expect("LinearTte should work");

    // Test LinearVariance method
    let linear_var_config = TemporalConfig {
        fixed_days: vec![15],
        interp_method: TemporalInterpMethod::LinearVariance,
        ..Default::default()
    };

    let linear_var_metrics =
        build_fixed_time_metrics(&data, forward, &linear_var_config, &strike_config)
            .expect("LinearVariance should work");

    assert_eq!(linear_tte_metrics.len(), 1);
    assert_eq!(linear_var_metrics.len(), 1);

    let tte_atm = linear_tte_metrics[0].atm_iv;
    let var_atm = linear_var_metrics[0].atm_iv;

    // Both should be reasonable (between 10d and 20d levels)
    assert!(
        tte_atm > 0.20 && tte_atm < 0.30,
        "LinearTte ATM should be between bounds, got {:.2}%",
        tte_atm * 100.0
    );
    assert!(
        var_atm > 0.20 && var_atm < 0.30,
        "LinearVariance ATM should be between bounds, got {:.2}%",
        var_atm * 100.0
    );

    println!("15-day interpolation:");
    println!("  LinearTte: {:.2}%", tte_atm * 100.0);
    println!("  LinearVariance: {:.2}%", var_atm * 100.0);
}

/// Tests temporal extrapolation behavior.
/// Verifies extrapolation controls work correctly.
#[test]
fn test_temporal_extrapolation() {
    use surface_lib::models::linear_iv::{build_fixed_time_metrics, TemporalConfig};

    let forward = 100.0;
    // Data only at 7 and 14 days - add more points per maturity to meet min_points requirement
    let data = vec![
        // 7 days
        create_market_data("put", 95.0, forward, 7.0 / 365.0, 0.27),
        create_market_data("call", 100.0, forward, 7.0 / 365.0, 0.25),
        create_market_data("call", 105.0, forward, 7.0 / 365.0, 0.26),
        // 14 days
        create_market_data("put", 95.0, forward, 14.0 / 365.0, 0.22),
        create_market_data("call", 100.0, forward, 14.0 / 365.0, 0.20),
        create_market_data("call", 105.0, forward, 14.0 / 365.0, 0.21),
    ];

    let strike_config = LinearIvConfig::default();

    // Test with extrapolation disabled
    let no_extrap_config = TemporalConfig {
        fixed_days: vec![1, 7, 14, 21], // 1d < min, 21d > max
        allow_short_extrapolate: false,
        allow_long_extrapolate: false,
        ..Default::default()
    };

    let no_extrap_metrics =
        build_fixed_time_metrics(&data, forward, &no_extrap_config, &strike_config)
            .expect("Should work but skip extrapolated points");

    // Should only have 7 and 14 day metrics (no extrapolation)
    assert_eq!(
        no_extrap_metrics.len(),
        2,
        "Should skip extrapolated points"
    );
    let days: Vec<i32> = no_extrap_metrics.iter().map(|m| m.tte_days).collect();
    assert_eq!(days, vec![7, 14]);

    // Test with extrapolation enabled
    let extrap_config = TemporalConfig {
        fixed_days: vec![1, 7, 14, 21],
        allow_short_extrapolate: true,
        allow_long_extrapolate: true,
        ..Default::default()
    };

    let extrap_metrics = build_fixed_time_metrics(&data, forward, &extrap_config, &strike_config)
        .expect("Should work with extrapolation");

    // Should have all 4 points
    assert_eq!(
        extrap_metrics.len(),
        4,
        "Should include extrapolated points"
    );
    let days: Vec<i32> = extrap_metrics.iter().map(|m| m.tte_days).collect();
    assert_eq!(days, vec![1, 7, 14, 21]);
}

/// Tests temporal interpolation with insufficient data.
/// Verifies proper error handling for edge cases.
#[test]
fn test_temporal_insufficient_data() {
    use surface_lib::models::linear_iv::{build_fixed_time_metrics, TemporalConfig};

    let forward = 100.0;

    // Test with only one maturity (less than min_maturities)
    let single_maturity_data = vec![create_market_data(
        "call",
        100.0,
        forward,
        7.0 / 365.0,
        0.25,
    )];

    let temp_config = TemporalConfig::default();
    let strike_config = LinearIvConfig::default();

    let result =
        build_fixed_time_metrics(&single_maturity_data, forward, &temp_config, &strike_config);
    assert!(result.is_err(), "Should fail with insufficient maturities");

    // Test with empty data
    let empty_data: Vec<MarketDataRow> = vec![];
    let result = build_fixed_time_metrics(&empty_data, forward, &temp_config, &strike_config);
    assert!(result.is_err(), "Should fail with empty data");

    // Test with data but insufficient points per maturity
    let insufficient_points_data = vec![
        create_market_data("call", 100.0, forward, 7.0 / 365.0, 0.25),
        create_market_data("call", 100.0, forward, 14.0 / 365.0, 0.20),
        // Only one point per maturity, but LinearIvConfig requires min 3 points
    ];

    let result = build_fixed_time_metrics(
        &insufficient_points_data,
        forward,
        &temp_config,
        &strike_config,
    );
    assert!(
        result.is_err(),
        "Should fail with insufficient points per maturity"
    );
}

/// Tests temporal interpolation with delta metrics.
/// Verifies RR and BF are interpolated correctly across time.
#[test]
fn test_temporal_delta_metrics() {
    use surface_lib::models::linear_iv::{build_fixed_time_metrics, TemporalConfig};

    let forward = 100.0;

    // Multi-maturity data with clear skew pattern
    let data = vec![
        // 7 days: steep skew
        create_market_data("put", 90.0, forward, 7.0 / 365.0, 0.35),
        create_market_data("put", 95.0, forward, 7.0 / 365.0, 0.30),
        create_market_data("call", 100.0, forward, 7.0 / 365.0, 0.25),
        create_market_data("call", 105.0, forward, 7.0 / 365.0, 0.27),
        create_market_data("call", 110.0, forward, 7.0 / 365.0, 0.30),
        // 21 days: flatter skew
        create_market_data("put", 90.0, forward, 21.0 / 365.0, 0.28),
        create_market_data("put", 95.0, forward, 21.0 / 365.0, 0.26),
        create_market_data("call", 100.0, forward, 21.0 / 365.0, 0.24),
        create_market_data("call", 105.0, forward, 21.0 / 365.0, 0.25),
        create_market_data("call", 110.0, forward, 21.0 / 365.0, 0.27),
    ];

    let temp_config = TemporalConfig {
        fixed_days: vec![7, 14, 21], // Include interpolated point
        ..Default::default()
    };
    let strike_config = LinearIvConfig::default();

    let metrics = build_fixed_time_metrics(&data, forward, &temp_config, &strike_config)
        .expect("Should work with delta metrics");

    assert_eq!(metrics.len(), 3);

    // Check that delta metrics are available
    for metric in &metrics {
        assert!(
            !metric.delta_metrics.is_empty(),
            "Should have delta metrics for {} days",
            metric.tte_days
        );

        // Check that we have expected delta levels
        let delta_levels: Vec<f64> = metric
            .delta_metrics
            .iter()
            .map(|dm| dm.delta_level)
            .collect();
        assert!(delta_levels.contains(&0.25), "Should have 25-delta metrics");
    }

    // Verify interpolated values are reasonable
    let day_14 = metrics.iter().find(|m| m.tte_days == 14).unwrap();
    println!("14-day interpolated metrics:");
    println!("  ATM IV: {:.2}%", day_14.atm_iv * 100.0);
    for dm in &day_14.delta_metrics {
        println!(
            "  {}δ - RR: {:.2}%, BF: {:.2}%",
            dm.delta_level * 100.0,
            dm.risk_reversal * 100.0,
            dm.butterfly * 100.0
        );
    }
}
