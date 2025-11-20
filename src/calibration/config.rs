use crate::calibration::types::FixedParameters;
use serde::Deserialize;

/// CMA-ES specific configuration parameters
#[derive(Debug, Clone, Deserialize)]
pub struct CmaEsConfig {
    /// Random seed for reproducibility
    pub seed: Option<u64>,
    /// Whether to evaluate the population in parallel
    pub parallel_eval: bool,
    /// Verbosity level (0=silent, 1=minimal, 2=normal)
    pub verbosity: u8,
    /// Number of IPOP restarts (0 = no IPOP)
    pub ipop_restarts: usize,
    /// Factor to increase population size in IPOP restarts
    pub ipop_increase_factor: f64,
    /// Max function evaluations per run (0=unlimited)
    pub max_evaluations: usize,
    /// Initial coordinate-wise standard deviation
    pub sigma0: f64,
    /// Number of BIPOP restarts (0 = no BIPOP)
    pub bipop_restarts: usize,
    /// Enable L-BFGS-B refinement after CMA-ES?
    pub lbfgsb_enabled: bool,
    /// Max iterations for L-BFGS-B
    pub lbfgsb_max_iterations: usize,
    /// Total function evaluations budget
    pub total_evals_budget: usize,
    /// Whether to use advanced sub-run budgeting logic
    pub use_subrun_budgeting: bool,
    /// Use mini CMA-ES on refinement
    pub mini_cmaes_on_refinement: bool,
}

impl Default for CmaEsConfig {
    fn default() -> Self {
        Self {
            seed: Some(123456),
            parallel_eval: true,
            verbosity: 0, // Silent by default for library use
            ipop_restarts: 0,
            ipop_increase_factor: 2.0,
            max_evaluations: 100000,
            sigma0: 0.3,
            bipop_restarts: 5,
            lbfgsb_enabled: true,
            lbfgsb_max_iterations: 200,
            total_evals_budget: 200000,
            use_subrun_budgeting: false,
            mini_cmaes_on_refinement: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdaptiveBoundsConfig {
    pub enabled: bool,
    pub max_iterations: usize,
    pub proximity_threshold: f64,
    pub expansion_factor: f64,
}

impl Default for AdaptiveBoundsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 3,
            proximity_threshold: 0.1, // 10% from edge
            expansion_factor: 0.25,   // expand by 25%
        }
    }
}

/// Main configuration struct for optimization
#[derive(Debug, Deserialize, Clone)]
pub struct OptimizationConfig {
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    #[serde(default = "default_tolerance")]
    pub tolerance: f64,

    #[serde(default)]
    pub fixed_params: FixedParameters,

    /// Population size for genetic algorithms
    #[serde(default = "default_pop_size")]
    pub pop_size: usize,

    /// Maximum generations for evolutionary algorithms
    #[serde(default = "default_max_gen")]
    pub max_gen: usize,

    /// Objective tolerance
    #[serde(default = "default_obj_tol")]
    pub obj_tol: f64,

    /// Covariance matrix adaptation parameter
    #[serde(default = "default_alpha_cov")]
    pub alpha_cov: f64,

    /// Step size adaptation parameter
    #[serde(default = "default_alpha_sigma")]
    pub alpha_sigma: f64,

    /// Target success rate
    #[serde(default = "default_target_sr")]
    pub target_sr: f64,

    /// CMA-ES specific configuration
    #[serde(default)]
    pub cmaes: CmaEsConfig,

    /// Adaptive bounds configuration
    #[serde(default)]
    pub adaptive_bounds: AdaptiveBoundsConfig,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            tolerance: default_tolerance(),
            fixed_params: FixedParameters::default(),
            pop_size: default_pop_size(),
            max_gen: default_max_gen(),
            obj_tol: default_obj_tol(),
            alpha_cov: default_alpha_cov(),
            alpha_sigma: default_alpha_sigma(),
            target_sr: default_target_sr(),
            cmaes: CmaEsConfig::default(),
            adaptive_bounds: AdaptiveBoundsConfig::default(),
        }
    }
}

impl OptimizationConfig {
    /// Default configuration for production calibration with high accuracy
    pub fn production() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-8,
            fixed_params: FixedParameters::default(),
            pop_size: 25,
            max_gen: 50,
            obj_tol: 1e-8,
            alpha_cov: 0.2,
            alpha_sigma: 0.5,
            target_sr: 0.2,
            cmaes: CmaEsConfig {
                verbosity: 0, // Slightly more verbose for production monitoring
                max_evaluations: 100000,
                total_evals_budget: 200000,
                ..CmaEsConfig::default()
            },
            adaptive_bounds: AdaptiveBoundsConfig::default(),
        }
    }

    /// Fast configuration for development and testing
    pub fn fast() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            fixed_params: FixedParameters::default(),
            pop_size: 30,
            max_gen: 50,
            obj_tol: 1e-6,
            alpha_cov: 0.2,
            alpha_sigma: 0.5,
            target_sr: 0.2,
            cmaes: CmaEsConfig {
                verbosity: 2,
                max_evaluations: 10000,
                total_evals_budget: 20000,
                bipop_restarts: 2,
                ..CmaEsConfig::default()
            },
            adaptive_bounds: AdaptiveBoundsConfig::default(),
        }
    }

    /// High-precision configuration for research and backtesting
    pub fn research() -> Self {
        Self {
            max_iterations: 10000,
            tolerance: 1e-9,
            fixed_params: FixedParameters::default(),
            pop_size: 100,
            max_gen: 200,
            obj_tol: 1e-9,
            alpha_cov: 0.15,
            alpha_sigma: 0.3,
            target_sr: 0.15,
            cmaes: CmaEsConfig {
                verbosity: 1,
                max_evaluations: 500000,
                total_evals_budget: 1000000,
                bipop_restarts: 5,
                ipop_restarts: 3,
                ..CmaEsConfig::default()
            },
            adaptive_bounds: AdaptiveBoundsConfig::default(),
        }
    }

    /// Minimal configuration for quick validation and debugging
    pub fn minimal() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-4,
            fixed_params: FixedParameters::default(),
            pop_size: 10,
            max_gen: 20,
            obj_tol: 1e-4,
            alpha_cov: 0.3,
            alpha_sigma: 0.7,
            target_sr: 0.3,
            cmaes: CmaEsConfig {
                verbosity: 0,
                max_evaluations: 1000,
                total_evals_budget: 2000,
                bipop_restarts: 1,
                ..CmaEsConfig::default()
            },
            adaptive_bounds: AdaptiveBoundsConfig::default(),
        }
    }
}

fn default_max_iterations() -> usize {
    5000
}

fn default_tolerance() -> f64 {
    1e-6
}

fn default_pop_size() -> usize {
    50
}

fn default_max_gen() -> usize {
    100
}

fn default_obj_tol() -> f64 {
    1e-8
}

fn default_alpha_cov() -> f64 {
    0.2
}

fn default_alpha_sigma() -> f64 {
    0.5
}

fn default_target_sr() -> f64 {
    0.2
}
