use serde::Deserialize;
use std::collections::HashMap;
use surface_lib::{MarketDataRow, OptimizationConfig};

/// CSV row structure matching the actual data format
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Some fields are only used via serde deserialization
struct CsvRow {
    #[serde(rename = "symbol")]
    symbol: String,
    #[serde(rename = "snapshot_ts")]
    snapshot_ts: String,
    #[serde(rename = "option_type")]
    option_type: String,
    #[serde(rename = "strike_price")]
    strike_price: f64,
    #[serde(rename = "underlying_price")]
    underlying_price: f64,
    #[serde(rename = "years_to_exp")]
    years_to_exp: f64,
    #[serde(rename = "mark_iv")]
    mark_iv: f64,
    #[serde(rename = "open_interest", default)]
    open_interest: f64,
    #[serde(rename = "vega", default)]
    vega: f64,
    #[serde(rename = "expiration_ts", default)]
    expiration_ts: Option<i64>,
}

/// Extract expiration string from symbol (e.g., "BTC-10JAN25-100000-C" -> "10JAN25")
fn extract_expiration_from_symbol(symbol: &str) -> Option<String> {
    let parts: Vec<&str> = symbol.split('-').collect();
    if parts.len() >= 3 {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Global mapping to store timestamp -> expiration string mappings learned from CSV data
static mut EXPIRATION_MAPPING: Option<HashMap<i64, String>> = None;

/// Load market data from CSV file and convert to surface-lib format
pub fn load_test_data(file_path: &str) -> Result<Vec<MarketDataRow>, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(file_path)?;
    let mut data = Vec::new();
    let mut timestamp_to_expiration: HashMap<i64, String> = HashMap::new();

    for result in reader.deserialize() {
        let row: CsvRow = result?;

        // Extract expiration string from symbol and map it to timestamp
        if let Some(expiration_str) = extract_expiration_from_symbol(&row.symbol) {
            if let Some(timestamp) = row.expiration_ts {
                timestamp_to_expiration.insert(timestamp, expiration_str);
            }
        }

        // Convert CSV row to surface-lib MarketDataRow format
        let market_data = MarketDataRow {
            option_type: row.option_type,
            strike_price: row.strike_price,
            underlying_price: row.underlying_price,
            years_to_exp: row.years_to_exp,
            market_iv: row.mark_iv / 100.0, // Convert from percentage to decimal
            vega: if row.vega > 0.0 { row.vega } else { 1.0 }, // Default vega if missing
            expiration: row.expiration_ts.unwrap_or_else(|| {
                // Fallback: derive from years_to_exp
                let seconds_per_year = 365.25 * 24.0 * 3600.0;
                let base_timestamp = 1735689600; // 2025-01-01 00:00:00 UTC
                base_timestamp + (row.years_to_exp * seconds_per_year) as i64
            }),
        };

        data.push(market_data);
    }

    // Store the mapping globally for use by timestamp_to_expiration_string
    unsafe {
        EXPIRATION_MAPPING = Some(timestamp_to_expiration);
    }

    Ok(data)
}

/// Filter data by expiration timestamp (approximate matching)
pub fn filter_by_expiration(data: Vec<MarketDataRow>, expiration_str: &str) -> Vec<MarketDataRow> {
    let target_timestamp = match expiration_str {
        "10JAN25" => 1736496000, // From the actual CSV data
        "17JAN25" => 1737100800,
        "24JAN25" => 1737705600,
        "31JAN25" => 1738310400,
        _ => {
            eprintln!("Unknown expiration: {}", expiration_str);
            return Vec::new();
        }
    };

    data.into_iter()
        .filter(|row| {
            // Allow some tolerance for timestamp matching (within 1 day)
            (row.expiration - target_timestamp).abs() < 86400
        })
        .collect()
}

/// Get available expirations in the dataset
pub fn get_available_expirations(data: &[MarketDataRow]) -> Vec<(i64, String, usize)> {
    let mut expiration_counts = HashMap::new();

    for row in data {
        *expiration_counts.entry(row.expiration).or_insert(0) += 1;
    }

    let mut expirations: Vec<_> = expiration_counts.into_iter().collect();
    expirations.sort_by_key(|(timestamp, _)| *timestamp);

    expirations
        .into_iter()
        .map(|(timestamp, count)| {
            let expiration_str = timestamp_to_expiration_string(timestamp);
            (timestamp, expiration_str, count)
        })
        .collect()
}

/// Convert timestamp back to expiration string for display
fn timestamp_to_expiration_string(timestamp: i64) -> String {
    unsafe {
        if let Some(ref mapping) = EXPIRATION_MAPPING {
            // Try exact match first
            if let Some(expiration_str) = mapping.get(&timestamp) {
                return expiration_str.clone();
            }

            // Try approximate matching (within 1 day) if exact match fails
            for (&mapped_timestamp, expiration_str) in mapping.iter() {
                if (timestamp - mapped_timestamp).abs() < 86400 {
                    return expiration_str.clone();
                }
            }
        }
    }

    // Fallback to unknown if no mapping found
    format!("UNKNOWN_{}", timestamp)
}

/// Create default test configuration
pub fn create_test_config() -> OptimizationConfig {
    // Use the fast default configuration for tests
    OptimizationConfig::fast()
}

/// Create test configuration with verbose output
pub fn create_verbose_test_config() -> OptimizationConfig {
    let mut config = OptimizationConfig::fast();
    config.cmaes.verbosity = 2; // Enable verbose output
    config
}
