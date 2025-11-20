# Surface-Lib

A high-performance Rust library for volatility surface modeling and calibration. Currently supports the SVI (Stochastic Volatility Inspired) model with advanced optimization capabilities for quantitative finance applications.

## Features

- **SVI Model**: Complete implementation of the SVI volatility model with parameter validation and no-arbitrage constraints
- **Advanced Calibration**: CMA-ES and L-BFGS-B optimization with robust parameter estimation
- **Model Parameters**: Configurable weighting schemes (ATM boost, vega weighting) for fine-tuning calibration
- **Option Pricing**: Black-Scholes pricing with calibrated volatility surfaces
- **Production Ready**: Optimized for real-time trading and backtesting systems
- **Type Safety**: Comprehensive error handling and parameter validation

## Usage

### Basic Example

```rust
use surface_lib::{
    calibrate_svi, price_with_svi, default_configs, CalibrationParams, 
    MarketDataRow, FixedParameters, SVIParams
};

// Create market data with required fields
let market_data = vec![
    MarketDataRow {
        option_type: "call".to_string(),
        strike_price: 100.0,
        underlying_price: 95.0,
        years_to_exp: 0.25,
        market_iv: 0.20,  // 20% volatility as decimal
        vega: 0.15,
        expiration: 1640995200, // Unix timestamp
    },
    // ... more data points
];

// Step 1: Calibrate SVI parameters
let config = default_configs::fast();
let calib_params = CalibrationParams::default();
let (objective, params, used_bounds) = calibrate_svi(
    market_data.clone(), 
    config, 
    calib_params,
    None, // Optional initial guess
)?;

// Step 2: Create SVI parameters from calibration results
let svi_params = SVIParams {
    t: 0.25,        // Time to expiration (should match your data)
    a: params[0],   // Base variance level
    b: params[1],   // Slope factor  
    rho: params[2], // Asymmetry parameter
    m: params[3],   // Horizontal shift
    sigma: params[4], // Curvature parameter
};

// Step 3: Price options with calibrated model
let fixed_params = FixedParameters {
    r: 0.02,  // Risk-free rate
    q: 0.0,   // Dividend yield
};

let pricing_results = price_with_svi(svi_params, market_data, fixed_params);

println!("Calibration objective: {}", objective);
println!("SVI parameters: {:?}", params);
for result in pricing_results {
    println!("Strike {}: Model Price ${:.2}, Model IV {:.1}%", 
             result.strike_price, result.model_price, result.model_iv * 100.0);
}
```

### Custom Model Parameters

Control the calibration weighting scheme with model-specific parameters:

```rust
use surface_lib::{CalibrationParams, SviModelParams};

// Default behavior (ATM boost = 25.0, vega weighting enabled)
let calib_params = CalibrationParams::default();

// Custom parameters for different weighting schemes
let mut calib_params = CalibrationParams::default();
calib_params.model_params = Some(Box::new(SviModelParams {
    atm_boost_factor: 15.0,        // Lower ATM emphasis (more wing weight)
    use_vega_weighting: false,     // Equal weight for all strikes
}));

let (objective, params, used_bounds) = calibrate_svi(market_data, config, calib_params, None)?;
```

**Model Parameters:**
- `atm_boost_factor`: Controls ATM weighting with `exp(-factor * |log_moneyness|)`. Higher values emphasize ATM options more strongly (default: 25.0)
- `use_vega_weighting`: Whether to weight observations by their vega values. Set to `false` for equal strike weighting (default: true)

### Configuration Presets

The library provides several optimization configuration presets:

```rust
use surface_lib::default_configs;

// For production trading systems
let config = default_configs::production();

// For development and testing (balanced speed/accuracy)
let config = default_configs::fast();

// For research and backtesting (highest precision)
let config = default_configs::research();

// For quick validation and debugging
let config = default_configs::minimal();
```

## Data Structure

The library expects a `MarketDataRow` with these essential fields:

```rust
pub struct MarketDataRow {
    pub option_type: String,      // "call" or "put"
    pub strike_price: f64,        // Strike price
    pub underlying_price: f64,    // Underlying asset price
    pub years_to_exp: f64,        // Time to expiration in years
    pub market_iv: f64,           // Market IV as decimal (0.25 = 25%)
    pub vega: f64,                // Option vega (for weighting)
    pub expiration: i64,          // Expiration timestamp
}
```

## SVI Model

The SVI model parameterizes total variance as:

```
w(k) = a + b * (ρ(k-m) + sqrt((k-m)² + σ²))
```

Where:
- `k` is log-moneyness: `ln(K/S)`
- `a` is the base variance level (vertical shift parameter)
- `b` is the slope factor (overall variance level)
- `ρ` is the asymmetry parameter (skew, must be in (-1, 1))
- `m` is the horizontal shift parameter (ATM location in log-moneyness)
- `σ` is the curvature parameter (smile curvature, must be > 0)

The model automatically enforces no-arbitrage constraints during calibration.

## API Reference

### Calibration

#### `calibrate_svi(data, config, calib_params)`

Calibrates SVI model parameters to market option data for a single expiration.

**Arguments:**
- `data: Vec<MarketDataRow>` - Market option data (single expiration only)
- `config: OptimizationConfig` - Optimization settings (use `default_configs` presets)
- `calib_params: CalibrationParams` - Calibration and model parameters
- `initial_guess: Option<Vec<f64>>` - Optional initial parameter guess for warm-started calibration

**Returns:**
- `(f64, Vec<f64>, SVIParamBounds)` - (objective_value, parameters, effective_parameter_bounds)

#### `price_with_svi(params, market_data, fixed_params)`

Prices European options using calibrated SVI parameters.

**Arguments:**
- `params: SVIParams` - Calibrated SVI parameters
- `market_data: Vec<MarketDataRow>` - Options to price
- `fixed_params: FixedParameters` - Risk-free rate and dividend yield

**Returns:**
- `Vec<PricingResult>` - Pricing results with model prices and implied volatilities

### Model Parameters

#### `SviModelParams`

Controls SVI-specific calibration behavior:

```rust
pub struct SviModelParams {
    pub atm_boost_factor: f64,    // ATM weighting strength (default: 25.0)
    pub use_vega_weighting: bool, // Enable vega weighting (default: true)
}
```

**Usage Examples:**
- **Equal weighting**: `SviModelParams { atm_boost_factor: 0.0, use_vega_weighting: false }`
- **Strong ATM focus**: `SviModelParams { atm_boost_factor: 50.0, use_vega_weighting: true }`
- **Wing emphasis**: `SviModelParams { atm_boost_factor: 10.0, use_vega_weighting: true }`

## Advanced Features

- **Configurable Weighting**: ATM boost and vega weighting can be independently controlled
- **Auto-Bounds**: Parameter bounds automatically adjust based on time to expiration  
- **Two-Stage Optimization**: Global search with CMA-ES followed by local refinement with L-BFGS-B
- **Production Ready**: Optimized for real-time trading and backtesting systems

## Requirements

- Data must contain options for a single expiration (SVI is a single-slice model)
- Minimum 5-10 data points recommended for stable calibration
- Market implied volatilities should be provided as decimals (0.25 for 25%)

## License

Licensed under either of:

- MIT license (`LICENSE-MIT`)
- Apache License, Version 2.0 (`LICENSE-APACHE`)

at your option.
