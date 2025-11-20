# Testing Guide for Surface-Lib

This document explains how to run the test suite for the surface-lib project.

## Running Tests from Command Line

### Run all tests
```bash
cargo test
```

### Run a specific test function
```bash
cargo test test_svi_calibration_10jan25 -- --nocapture
```

### Run tests with full output (recommended for debugging)
```bash
cargo test --test integration_tests -- --nocapture
```

## Running Tests from Cursor IDE

### Method 1: Using Test Lens (Recommended)
With the current rust-analyzer configuration, you should see "Run Test" and "Debug Test" buttons (code lens) above each test function in `tests/svi_tests.rs` and `tests/linear_iv_tests.rs`. Click these to run individual tests.

### Method 2: Using VS Code Tasks
Press `Ctrl+Shift+P` (or `Cmd+Shift+P` on Mac) and type "Tasks: Run Task", then select one of:
- `cargo test` - Run all tests
- `cargo test -- --nocapture` - Run with full output
- `cargo test test_svi_calibration_10jan25` - Run the SVI calibration test
- `cargo test test_data_loading_and_filtering` - Run the data loading test

### Method 3: Using Debug Configuration
Press `F5` or go to Run & Debug view and select one of the preconfigured launch configurations:
- "Test integration_tests" - Run all tests
- "Test SVI Calibration 10JAN25" - Run SVI calibration test
- "Test Data Loading" - Run data loading test

## Test Data

The tests use real market data from `tests/data/options_snapshots_20250101.csv`. This file contains Bitcoin options data with the following structure:
- Multiple expiration dates (10JAN25, 17JAN25, etc.)
- Both calls and puts
- Market implied volatilities and Greeks
- Real strike prices and underlying prices

## Test Structure

### `test_svi_calibration_10jan25`
- Tests the complete SVI model calibration pipeline
- Validates parameter bounds and no-arbitrage constraints
- Checks pricing accuracy against market data
- Expected runtime: ~0.05 seconds

### `test_data_loading_and_filtering`
- Tests CSV data loading and parsing
- Validates data integrity and type conversion
- Tests expiration filtering functionality
- Expected runtime: ~0.01 seconds

## Troubleshooting

### IDE Not Showing Test Buttons
1. Ensure rust-analyzer extension is installed and active
2. Restart the language server: `Ctrl+Shift+P` → "rust-analyzer: Restart server"
3. Check that you're in the `surface-lib` directory when opening the file
4. Verify the project compiles: `cargo check`

### Tests Failing
1. Check that the test data file exists: `tests/options_snapshots_20250101.csv`
2. Ensure all dependencies are installed: `cargo build`
3. Run with `--nocapture` to see detailed output

### Performance Issues
The tests exercise the same CMA-ES + L-BFGS-B pipeline used by the library itself. On a typical developer machine the full suite should complete in under a few seconds. If tests are slow, try running a single test with `-- --nocapture` to inspect progress and CMA-ES verbosity.
