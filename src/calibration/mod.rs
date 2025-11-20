pub mod config;
pub mod pipeline;
pub mod types;

// Re-export optimization algorithms for easy access inside the library
pub use cmaes_lbfgsb::cmaes;
pub use cmaes_lbfgsb::lbfgsb_optimize;
