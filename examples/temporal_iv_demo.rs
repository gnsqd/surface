use surface_lib::{
    build_fixed_time_metrics, LinearIvConfig, MarketDataRow, TemporalConfig, TemporalInterpMethod,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Temporal IV Interpolation Demo - Fixed Time Grid");
    println!("===============================================");

    // Create multi-maturity BTC option data with realistic vol term structure
    let forward = 65000.0; // $65k BTC

    let market_data = vec![
        // 7 days: High front-end vol due to event risk
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 55000.0,
            underlying_price: forward,
            years_to_exp: 7.0 / 365.0,
            market_iv: 0.85,
            vega: 12.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 60000.0,
            underlying_price: forward,
            years_to_exp: 7.0 / 365.0,
            market_iv: 0.72,
            vega: 18.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 65000.0,
            underlying_price: forward,
            years_to_exp: 7.0 / 365.0,
            market_iv: 0.65,
            vega: 22.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 70000.0,
            underlying_price: forward,
            years_to_exp: 7.0 / 365.0,
            market_iv: 0.68,
            vega: 18.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 75000.0,
            underlying_price: forward,
            years_to_exp: 7.0 / 365.0,
            market_iv: 0.75,
            vega: 12.0,
            expiration: 0,
        },
        // 14 days: Moderate vol level
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 55000.0,
            underlying_price: forward,
            years_to_exp: 14.0 / 365.0,
            market_iv: 0.78,
            vega: 15.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 60000.0,
            underlying_price: forward,
            years_to_exp: 14.0 / 365.0,
            market_iv: 0.68,
            vega: 22.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 65000.0,
            underlying_price: forward,
            years_to_exp: 14.0 / 365.0,
            market_iv: 0.62,
            vega: 25.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 70000.0,
            underlying_price: forward,
            years_to_exp: 14.0 / 365.0,
            market_iv: 0.64,
            vega: 22.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 75000.0,
            underlying_price: forward,
            years_to_exp: 14.0 / 365.0,
            market_iv: 0.70,
            vega: 15.0,
            expiration: 0,
        },
        // 30 days: Lower vol, mean reversion
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 55000.0,
            underlying_price: forward,
            years_to_exp: 30.0 / 365.0,
            market_iv: 0.70,
            vega: 18.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 60000.0,
            underlying_price: forward,
            years_to_exp: 30.0 / 365.0,
            market_iv: 0.62,
            vega: 28.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 65000.0,
            underlying_price: forward,
            years_to_exp: 30.0 / 365.0,
            market_iv: 0.58,
            vega: 32.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 70000.0,
            underlying_price: forward,
            years_to_exp: 30.0 / 365.0,
            market_iv: 0.60,
            vega: 28.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 75000.0,
            underlying_price: forward,
            years_to_exp: 30.0 / 365.0,
            market_iv: 0.65,
            vega: 18.0,
            expiration: 0,
        },
    ];

    println!("Market Data Summary:");
    println!("  Forward: ${:.0}", forward);
    println!("  Available maturities: 7, 14, 30 days");
    println!(
        "  Strikes per maturity: 5 (${:.0} to ${:.0})",
        55000.0, 75000.0
    );
    println!("  Total contracts: {}", market_data.len());
    println!();

    // Configuration for Linear IV interpolation per maturity
    let strike_config = LinearIvConfig::default();

    // Configuration for temporal interpolation across maturities
    let temporal_config = TemporalConfig {
        fixed_days: vec![1, 3, 7, 14, 21, 30, 45, 60], // Standardized expiry ladder
        interp_method: TemporalInterpMethod::LinearVariance, // Consistent with strike interpolation
        allow_short_extrapolate: true,                 // Enable 1d and 3d extrapolation
        allow_long_extrapolate: true,                  // Enable 45d and 60d extrapolation
        min_maturities: 2,
    };

    println!("Temporal Configuration:");
    println!("  Target days: {:?}", temporal_config.fixed_days);
    println!(
        "  Interpolation method: {:?}",
        temporal_config.interp_method
    );
    println!(
        "  Short extrapolation: {}",
        temporal_config.allow_short_extrapolate
    );
    println!(
        "  Long extrapolation: {}",
        temporal_config.allow_long_extrapolate
    );
    println!();

    // Build fixed time grid metrics
    let fixed_time_metrics =
        build_fixed_time_metrics(&market_data, forward, &temporal_config, &strike_config)?;

    println!("Fixed Time Grid Results:");
    println!("========================");
    println!();

    for metric in &fixed_time_metrics {
        println!(
            "{}d expiry ({:.3} years):",
            metric.tte_days, metric.tte_years
        );
        println!("  ATM IV: {:.1}%", metric.atm_iv * 100.0);

        if !metric.delta_metrics.is_empty() {
            println!("  Delta Metrics:");
            for dm in &metric.delta_metrics {
                println!(
                    "    {}δ: RR = {:+.1}%, BF = {:+.1}%",
                    (dm.delta_level * 100.0) as i32,
                    dm.risk_reversal * 100.0,
                    dm.butterfly * 100.0
                );
            }
        }
        println!();
    }

    // Demonstrate different interpolation methods
    println!("Comparison of Interpolation Methods (21d expiry):");
    println!("=================================================");

    for method in [
        TemporalInterpMethod::LinearTte,
        TemporalInterpMethod::LinearVariance,
        TemporalInterpMethod::SquareRootTime,
    ] {
        let comparison_config = TemporalConfig {
            fixed_days: vec![21],
            interp_method: method,
            allow_short_extrapolate: true,
            allow_long_extrapolate: true,
            min_maturities: 2,
        };

        let comparison_metrics =
            build_fixed_time_metrics(&market_data, forward, &comparison_config, &strike_config)?;

        if let Some(metric_21d) = comparison_metrics.first() {
            println!("{:?}: ATM IV = {:.1}%", method, metric_21d.atm_iv * 100.0);
        }
    }
    println!();

    // Term structure analysis
    println!("Term Structure Analysis:");
    println!("========================");
    println!("Days    ATM IV   Ann.Vol  Days to Next  IV Carry");
    println!("----    ------   -------  ------------  --------");

    for (i, metric) in fixed_time_metrics.iter().enumerate() {
        let annualized_vol = metric.atm_iv;
        let days_to_next = if i + 1 < fixed_time_metrics.len() {
            fixed_time_metrics[i + 1].tte_days - metric.tte_days
        } else {
            0
        };

        let iv_carry = if i > 0 {
            let prev_iv = fixed_time_metrics[i - 1].atm_iv;
            ((metric.atm_iv - prev_iv) / prev_iv * 100.0) as i32
        } else {
            0
        };

        println!(
            "{:>3}d    {:>5.1}%    {:>6.1}%    {:>10}    {:>+4}%",
            metric.tte_days,
            metric.atm_iv * 100.0,
            annualized_vol * 100.0,
            if days_to_next > 0 {
                days_to_next.to_string()
            } else {
                "-".to_string()
            },
            iv_carry
        );
    }
    println!();

    println!("Summary:");
    println!("--------");
    println!(
        "• Successfully built fixed time grid from {} input maturities",
        temporal_config.min_maturities.max(3)
    ); // We have 3 maturities
    println!(
        "• Interpolated/extrapolated to {} standardized expiries",
        fixed_time_metrics.len()
    );
    println!("• Maintained consistent vol surface methodology across time and strike");
    println!("• Ready for downstream pricing, Greeks calculation, and risk management");
    println!();

    println!("Demo completed successfully!");

    Ok(())
}
