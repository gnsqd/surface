use surface_lib::{build_linear_iv_from_market_data, LinearIvConfig, MarketDataRow};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Linear IV Interpolation Demo");
    println!("============================");

    // Create sample market data for a BTC option chain
    let forward = 100000.0; // $100k BTC
    let tte = 30.0 / 365.0; // 30 days to expiration

    let market_data = vec![
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 85000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.65,
            vega: 10.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 90000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.55,
            vega: 15.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "put".to_string(),
            strike_price: 95000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.48,
            vega: 20.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 100000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.45,
            vega: 25.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 105000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.47,
            vega: 20.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 110000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.52,
            vega: 15.0,
            expiration: 0,
        },
        MarketDataRow {
            option_type: "call".to_string(),
            strike_price: 115000.0,
            underlying_price: forward,
            years_to_exp: tte,
            market_iv: 0.58,
            vega: 10.0,
            expiration: 0,
        },
    ];

    let config = LinearIvConfig::default();

    println!("Market data:");
    for (i, point) in market_data.iter().enumerate() {
        println!(
            "  {}. Strike: ${}, IV: {:.1}%, Type: {}",
            i + 1,
            point.strike_price,
            point.market_iv * 100.0,
            point.option_type
        );
    }
    println!("Forward: ${:.0}", forward);
    println!("Time to expiration: {:.0} days", tte * 365.0);
    println!();

    // Perform linear IV interpolation (uses forward and tte from market data)
    let result = build_linear_iv_from_market_data(&market_data, &config)?;

    println!("Results:");
    println!("--------");
    println!("ATM IV: {:.2}%", result.atm_iv * 100.0);
    println!();

    println!("Delta IVs:");
    for delta_iv in &result.delta_ivs {
        println!(
            "  {}δ: {:.2}%",
            if delta_iv.delta > 0.0 {
                format!("+{:.0}", delta_iv.delta * 100.0)
            } else {
                format!("{:.0}", delta_iv.delta * 100.0)
            },
            delta_iv.iv * 100.0
        );
    }
    println!();

    if let Some(rr) = result.rr_25 {
        println!("25δ Risk Reversal: {:.2}%", rr * 100.0);
    }

    if let Some(bf) = result.bf_25 {
        println!("25δ Butterfly: {:.2}%", bf * 100.0);
    }

    println!();
    println!("Demo completed successfully!");

    Ok(())
}
