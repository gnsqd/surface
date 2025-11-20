use crate::calibration::config::OptimizationConfig;
use crate::calibration::types::{MarketDataRow, ModelCalibrator};
// Note: HashMap removed as param_map is no longer used
use cmaes_lbfgsb::cmaes::{canonical_cmaes_optimize, CmaesCanonicalConfig};
use cmaes_lbfgsb::lbfgsb_optimize::lbfgsb_optimize;

/// A simplified calibration process for surface models
pub struct CalibrationProcess {
    model: Box<dyn ModelCalibrator>,
    config: OptimizationConfig,
    market_data: Vec<MarketDataRow>,
    initial_guess: Option<Vec<f64>>,
}

impl CalibrationProcess {
    pub fn new(
        model: Box<dyn ModelCalibrator>,
        config: OptimizationConfig,
        market_data: Vec<MarketDataRow>,
    ) -> Self {
        Self {
            model,
            config,
            market_data,
            initial_guess: None,
        }
    }

    /// Set initial guess for optimization
    pub fn with_initial_guess(mut self, guess: Vec<f64>) -> Self {
        self.initial_guess = Some(guess);
        self
    }

    /// Run the calibration process and return the best parameters
    pub fn run(&self) -> (f64, Vec<f64>) {
        let (best_obj, best_params) = calibrate_model(
            &*self.model,
            &self.market_data,
            &self.config,
            self.initial_guess.clone(),
        );
        (best_obj, best_params)
    }
}

/// Advanced optimization function combining CMA-ES for global search and L-BFGS-B for local refinement.
/// Uses relaxed bounds and objective function for global search, then standard bounds for refinement.
pub fn calibrate_model(
    model: &dyn ModelCalibrator,
    market_data: &[MarketDataRow],
    config: &OptimizationConfig,
    initial_guess: Option<Vec<f64>>,
) -> (f64, Vec<f64>) {
    // Standard bounds and objective used for L-BFGS-B
    let bounds = model.param_bounds();
    let obj_fn = |x: &[f64]| model.evaluate_objective(x, market_data);

    // 1) CMA-ES approach, either a "mini CMA-ES" around the initial guess or full CMA-ES if none provided.
    // Use relaxed bounds and objective function for the global search
    let (best_obj, best_sol) = {
        // Use the same bounds and objective for CMA-ES
        let relaxed_bounds = model.param_bounds();
        let relaxed_obj_fn = |x: &[f64]| model.evaluate_objective(x, market_data);

        // Prepare CMA-ES config with all the sophisticated settings
        let cmaes_config = CmaesCanonicalConfig {
            population_size: config.pop_size,
            max_generations: config.max_gen,
            seed: config.cmaes.seed.unwrap_or(123456),
            c1: None, // Use defaults for now - could be added to config later
            c_mu: None,
            c_sigma: None,
            d_sigma: None,
            parallel_eval: config.cmaes.parallel_eval,
            verbosity: config.cmaes.verbosity,
            ipop_restarts: config.cmaes.ipop_restarts,
            ipop_increase_factor: config.cmaes.ipop_increase_factor,
            bipop_restarts: config.cmaes.bipop_restarts,
            total_evals_budget: config.cmaes.total_evals_budget,
            use_subrun_budgeting: config.cmaes.use_subrun_budgeting,
            alpha_mu: None,
            hsig_threshold_factor: None,
            bipop_small_population_factor: None,
            bipop_small_budget_factor: None,
            bipop_large_budget_factor: None,
            bipop_large_pop_increase_factor: None,
            max_bound_iterations: None,
            eig_precision_threshold: None,
            min_eig_value: None,
            matrix_op_threshold: None,
            stagnation_limit: None,
            min_sigma: None,
        };

        // If we have an initial guess, check if we should run mini CMA-ES or go straight to L-BFGS-B
        if let Some(ref guess) = initial_guess {
            // Check if mini_cmaes_on_refinement is enabled
            let use_mini_cmaes = config.cmaes.mini_cmaes_on_refinement;

            if use_mini_cmaes {
                if config.cmaes.verbosity > 0 {
                    println!(
                        "Using provided initial guess => launching mini CMA-ES around it. \
                         Then local L-BFGS refinement."
                    );
                }

                // Evaluate the guess if you want to log it
                let guess_obj = relaxed_obj_fn(guess);
                if config.cmaes.verbosity > 0 {
                    println!("  Initial guess objective = {:.6}", guess_obj);
                }

                // Run mini-CMA-ES
                let cmaes_result = canonical_cmaes_optimize(
                    relaxed_obj_fn,
                    relaxed_bounds,
                    cmaes_config,
                    // We pass the guess as the initial distribution center
                    Some(guess.clone()),
                );

                // Get best solution from relaxed objective, evaluate with standard objective
                let (_, relaxed_params) = cmaes_result.best_solution;
                let standard_obj = obj_fn(&relaxed_params);
                (standard_obj, relaxed_params)
            } else {
                // Skip mini CMA-ES and use the initial guess directly for L-BFGS-B
                if config.cmaes.verbosity > 0 {
                    println!(
                        "Using provided initial guess => skipping mini CMA-ES and proceeding directly to L-BFGS-B."
                    );
                }

                // Evaluate the guess to get initial objective value
                let guess_obj = obj_fn(guess);
                if config.cmaes.verbosity > 0 {
                    println!("  Initial guess objective = {:.6}", guess_obj);
                }

                (guess_obj, guess.clone())
            }
        } else {
            // Otherwise do the full CMA-ES as before
            if config.cmaes.verbosity > 0 {
                println!("No initial guess provided => running full CMA-ES with BIPOP restarts");
            }

            let cmaes_result =
                canonical_cmaes_optimize(relaxed_obj_fn, relaxed_bounds, cmaes_config, None);

            // Get best solution from relaxed objective, evaluate with standard objective
            let (_, relaxed_params) = cmaes_result.best_solution;
            let standard_obj = obj_fn(&relaxed_params);
            (standard_obj, relaxed_params)
        }
    };

    // 2) Local refinement of the best solution with L-BFGS-B (if enabled)
    if config.cmaes.lbfgsb_enabled {
        if config.cmaes.verbosity > 0 {
            println!("Running L-BFGS-B refinement on best CMA-ES solution...");
        }

        let mut refined_solution = best_sol.clone();
        let refine_res = lbfgsb_optimize(
            &mut refined_solution,
            bounds,
            &obj_fn,
            config.cmaes.lbfgsb_max_iterations,
            config.tolerance,
            if config.cmaes.verbosity >= 1 {
                Some(|_current_x: &[f64], current_obj: f64| {
                    println!("L-BFGS-B iteration => objective = {:.6}", current_obj);
                })
            } else {
                None
            },
            None, // Use default config
        );

        match refine_res {
            Ok((loc_obj, loc_sol)) => {
                if loc_obj < best_obj {
                    if config.cmaes.verbosity > 0 {
                        println!(
                            "L-BFGS-B improved objective: {:.6} -> {:.6}",
                            best_obj, loc_obj
                        );
                    }
                    (loc_obj, loc_sol)
                } else {
                    if config.cmaes.verbosity > 0 {
                        println!("L-BFGS-B did not improve objective, keeping CMA-ES solution");
                    }
                    (best_obj, best_sol)
                }
            }
            Err(e) => {
                if config.cmaes.verbosity > 0 {
                    println!("L-BFGS-B failed: {:?}, keeping CMA-ES solution", e);
                }
                (best_obj, best_sol)
            }
        }
    } else {
        if config.cmaes.verbosity > 0 {
            println!("L-BFGS-B refinement disabled, using CMA-ES solution directly");
        }
        (best_obj, best_sol)
    }
}

/// Generic adaptive calibration wrapper
pub fn calibrate_model_adaptive(
    mut model: Box<dyn ModelCalibrator>,
    market_data: &[MarketDataRow],
    config: &OptimizationConfig,
    initial_guess: Option<Vec<f64>>,
) -> (f64, Vec<f64>, Vec<(f64, f64)>) {
    if !config.adaptive_bounds.enabled {
        let (obj, params) = calibrate_model(&*model, market_data, config, initial_guess);
        let bounds = model.param_bounds().to_vec();
        return (obj, params, bounds);
    }

    let mut best_obj = f64::MAX;
    let mut best_params = Vec::new();

    for iter in 0..config.adaptive_bounds.max_iterations {
        let (obj, params) = calibrate_model(&*model, market_data, config, initial_guess.clone());
        if obj < best_obj {
            best_obj = obj;
            best_params = params.clone();
        }
        let adjusted = model.expand_bounds_if_needed(
            &params,
            config.adaptive_bounds.proximity_threshold,
            config.adaptive_bounds.expansion_factor,
        );

        if config.cmaes.verbosity > 0 {
            if adjusted {
                println!(
                    "Adaptive iteration {}: Expanded bounds for next iteration",
                    iter + 1
                );
            } else {
                println!(
                    "Adaptive iteration {}: No expansion needed, stopping early",
                    iter + 1
                );
            }
        }

        if !adjusted {
            break;
        }
    }

    let bounds = model.param_bounds().to_vec();
    (best_obj, best_params, bounds)
}
