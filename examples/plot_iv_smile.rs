// Example: plot_iv_smile.rs
// Runs calibration for a single-expiry BTC options CSV file and produces a PNG
// comparing market IVs with the calibrated SVI smile.
//
// Usage:
//     cargo run --example plot_iv_smile -- <csv_path> <EXP_STRING>
//
// The CSV must contain the same columns expected by test_utils::load_test_data.
// The output image will be written to iv_smile.png in the working directory.

use std::env;
use std::error::Error;

use csv::ReaderBuilder;
use plotters::prelude::*;
use surface_lib::models::svi::svi_model::SVISlice;
use surface_lib::{
    calibrate_svi, default_configs, price_with_svi, CalibrationParams, FixedParameters,
    MarketDataRow, SVIParams, SviModelParams,
};

// ---------------------------------------------------------------------------
// CSV deserialization helpers
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Clone)]
struct CsvRow {
    #[serde(rename = "symbol")]
    symbol: String,
    #[serde(rename = "option_type")]
    option_type: String,
    #[serde(rename = "strike_price")]
    strike_price: f64,
    #[serde(rename = "underlying_price")]
    underlying_price: f64,
    #[serde(rename = "years_to_exp")]
    years_to_exp: f64,
    #[serde(rename = "mark_iv")]
    market_iv: f64,
    #[serde(rename = "bid_iv")]
    bid_iv: Option<f64>,
    #[serde(rename = "ask_iv")]
    ask_iv: Option<f64>,
    vega: f64,
    #[serde(rename = "expiration_ts")]
    expiration: i64,
}

impl From<CsvRow> for MarketDataRow {
    fn from(r: CsvRow) -> Self {
        MarketDataRow {
            option_type: r.option_type,
            strike_price: r.strike_price,
            underlying_price: r.underlying_price,
            years_to_exp: r.years_to_exp,
            market_iv: if r.market_iv > 1.0 {
                r.market_iv / 100.0
            } else {
                r.market_iv
            },
            vega: if r.vega > 0.0 { r.vega } else { 1.0 },
            expiration: r.expiration,
        }
    }
}

fn load_csv(path: &str) -> Result<Vec<CsvRow>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().has_headers(true).from_path(path)?;
    let mut rows = Vec::new();
    for result in rdr.deserialize() {
        let row: CsvRow = result?;
        rows.push(row);
    }
    Ok(rows)
}

fn filter_by_expiration_string(data: Vec<CsvRow>, target_exp: &str) -> Vec<CsvRow> {
    // Try instrument name filtering first
    if let Some(first_row) = data.first() {
        let inst_name = &first_row.symbol;
        println!("Sample symbol: {}", inst_name);

        let token = format!("-{}-", target_exp.to_uppercase());
        let filtered: Vec<_> = data
            .iter()
            .filter(|row| row.symbol.to_uppercase().contains(&token))
            .cloned()
            .collect();

        if !filtered.is_empty() {
            println!(
                "Filtered {} options by symbol containing '{}'",
                filtered.len(),
                target_exp
            );
            return filtered;
        }
    }

    // Fallback: if no instrument name or no matches, show available expirations
    println!(
        "No instrument name matches found for '{}'. Available options:",
        target_exp
    );

    use std::collections::HashMap;
    let mut exp_data: HashMap<i64, (usize, f64)> = HashMap::new(); // timestamp -> (count, avg_days)

    for row in &data {
        let entry = exp_data.entry(row.expiration).or_insert((0, 0.0));
        entry.0 += 1; // count
        entry.1 += row.years_to_exp * 365.0; // sum of days
    }

    for (&timestamp, &(count, days_sum)) in &exp_data {
        let avg_days = days_sum / count as f64;
        println!(
            "  {}: {} options (~{:.1} days to expiration)",
            timestamp, count, avg_days
        );
    }

    // Return empty if no match found
    Vec::new()
}

fn filter_otm_and_moneyness(
    csv_rows: Vec<CsvRow>,
    moneyness_min: f64,
    moneyness_max: f64,
) -> Vec<CsvRow> {
    csv_rows
        .into_iter()
        .filter(|row| {
            let underlying = row.underlying_price;
            let strike = row.strike_price;
            let moneyness = strike / underlying;

            // Check moneyness range
            if moneyness < moneyness_min || moneyness > moneyness_max {
                return false;
            }

            // Check OTM condition
            match row.option_type.as_str() {
                "call" => strike > underlying, // OTM calls: strike > spot
                "put" => strike < underlying,  // OTM puts: strike < spot
                _ => false,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <csv_file> <expiration_str>\nExample: {} options.csv 10JAN25",
            args[0], args[0]
        );
        std::process::exit(1);
    }
    let csv_path = &args[1];
    let expiration_str = &args[2];

    let mut csv_rows = load_csv(csv_path)?;
    csv_rows = filter_by_expiration_string(csv_rows, expiration_str);

    println!("Loaded {} options after expiry filtering", csv_rows.len());

    // Apply OTM and moneyness filter (widened to include more wings)
    let moneyness_min = 0.95;
    let moneyness_max = 1.05;
    csv_rows = filter_otm_and_moneyness(csv_rows, moneyness_min, moneyness_max);

    println!(
        "Filtered to {} OTM options in moneyness range [{:.2}-{:.2}]",
        csv_rows.len(),
        moneyness_min,
        moneyness_max
    );

    if csv_rows.is_empty() {
        return Err("No data after OTM and moneyness filtering".into());
    }
    let data: Vec<MarketDataRow> = csv_rows.iter().cloned().map(|r| r.clone().into()).collect();

    // Calibrate SVI
    let mut config = default_configs::fast();
    // Enable adaptive bounds for better wing fitting
    config.adaptive_bounds.enabled = true;
    config.adaptive_bounds.max_iterations = 200;
    config.adaptive_bounds.proximity_threshold = 0.4; // 40% from edge
    config.adaptive_bounds.expansion_factor = 0.05; // Expand by 5%
                                                    // ---------------------------------------------------------------------
                                                    // Example: override the ATM boost factor used during calibration.
                                                    // By default the calibrator applies `exp(-25 * |k|)` weighting, but we
                                                    // can tune this to make wings more/less influential.  Here we set it to
                                                    // 15.0 which gives slightly more weight to OTM options.
                                                    // ---------------------------------------------------------------------
    let calib_params = CalibrationParams {
        model_params: Some(Box::new(SviModelParams {
            atm_boost_factor: 5.0,
            use_vega_weighting: true,
        })),
        ..CalibrationParams::default()
    };
    let (obj, params_vec, _used_bounds) = calibrate_svi(data.clone(), config, calib_params, None)?;
    println!("Calibration objective: {:.6}", obj);
    println!("Calibrated SVI parameters:");
    println!("  a: {:.6}", params_vec[0]);
    println!("  b: {:.6}", params_vec[1]);
    println!("  rho: {:.6}", params_vec[2]);
    println!("  m: {:.6}", params_vec[3]);
    println!("  sigma: {:.6}", params_vec[4]);

    let t = data[0].years_to_exp;
    let svi_params = SVIParams::new(
        t,
        params_vec[0],
        params_vec[1],
        params_vec[2],
        params_vec[3],
        params_vec[4],
    )?;

    // Price with calibrated parameters
    let fixed = FixedParameters { r: 0.0, q: 0.0 };
    let priced = price_with_svi(svi_params.clone(), data.clone(), fixed);

    // Print debug table
    println!("\nDebug: Strike | Market IV% | Model IV% | Diff%");
    for (row, pr) in data.iter().zip(priced.iter()) {
        let market_iv_pct = row.market_iv * 100.0;
        let model_iv_pct = pr.model_iv * 100.0;
        let diff = model_iv_pct - market_iv_pct;
        println!(
            "{:.0} | {:.2} | {:.2} | {:.2}",
            row.strike_price, market_iv_pct, model_iv_pct, diff
        );
    }

    // Prepare data for plotting
    let mut call_points = Vec::new();
    let mut put_points = Vec::new();
    // Points for smooth model IV line
    let mut model_line = Vec::new();
    let mut error_bars: Vec<(f64, f64, f64)> = Vec::new(); // strike, bid, ask

    for ((csv, row), pr) in csv_rows.iter().zip(data.iter()).zip(priced.iter()) {
        let iv_pct = row.market_iv * 100.0;
        let _model_iv_pct = pr.model_iv * 100.0;

        if let (Some(bid), Some(ask)) = (csv.bid_iv, csv.ask_iv) {
            if bid > 0.0 && ask > bid {
                error_bars.push((row.strike_price, bid, ask));
            }
        }

        if row.option_type == "call" {
            call_points.push((row.strike_price, iv_pct));
        } else {
            put_points.push((row.strike_price, iv_pct));
        }
    }

    let min_strike = data
        .iter()
        .map(|r| r.strike_price)
        .fold(f64::INFINITY, f64::min);
    let max_strike = data
        .iter()
        .map(|r| r.strike_price)
        .fold(f64::NEG_INFINITY, f64::max);
    // Calculate dynamic IV range from actual data
    let market_ivs: Vec<f64> = data.iter().map(|r| r.market_iv * 100.0).collect();
    let model_ivs: Vec<f64> = priced.iter().map(|p| p.model_iv * 100.0).collect();

    let min_market_iv = market_ivs.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_market_iv = market_ivs.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let min_model_iv = model_ivs.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_model_iv = model_ivs.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

    let min_iv = min_market_iv.min(min_model_iv);
    let max_iv = max_market_iv.max(max_model_iv);

    // Add 5% padding to the range for better visualization
    let iv_range = max_iv - min_iv;
    let padding = iv_range * 0.05;
    let y_min = (min_iv - padding).max(0.0); // Don't go below 0%
    let y_max = max_iv + padding;

    // Build smooth model curve across strikes
    let slice = SVISlice::new(svi_params.clone());
    let underlying = data[0].underlying_price.max(1.0);
    let strike_min = (min_strike * 0.9).max(underlying * 0.2);
    let strike_max = max_strike * 1.1;
    let steps = 250;
    for i in 0..=steps {
        let strike = strike_min + (strike_max - strike_min) * (i as f64) / (steps as f64);
        let k = (strike / underlying).ln();
        let iv_pct = slice.implied_vol(k) * 100.0;
        model_line.push((strike, iv_pct));
    }

    // Plot
    let root = SVGBackend::new("iv_smile.svg", (1280, 768)).into_drawing_area();
    root.fill(&WHITE)?;
    let days_to_exp = t * 365.0;
    println!(
        "Time to expiration: {:.6} years = {:.2} days",
        t, days_to_exp
    );
    let mut chart = ChartBuilder::on(&root)
        .margin(20)
        .caption(
            format!(
                "SVI Model vs Market IV Smile | Exp: {} (t={:.4}y, {:.1}d)",
                expiration_str, t, days_to_exp
            ),
            ("sans-serif", 30),
        )
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(min_strike..max_strike, y_min..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Strike ($)")
        .y_desc("Implied Vol (%)")
        .draw()?;

    // Scatter market IVs (smaller dots)
    chart.draw_series(
        call_points
            .iter()
            .map(|pt| Circle::new(*pt, 2, RED.filled())),
    )?;
    chart.draw_series(
        put_points
            .iter()
            .map(|pt| Circle::new(*pt, 2, BLUE.filled())),
    )?;

    // Draw bid-ask error bars
    for (strike, bid, ask) in error_bars {
        chart.draw_series(std::iter::once(PathElement::new(
            vec![(strike, bid), (strike, ask)],
            BLUE.stroke_width(1),
        )))?;
    }

    // Model line
    chart.draw_series(vec![PathElement::new(model_line, RED)])?;

    println!("Chart saved to iv_smile.svg");
    Ok(())
}
