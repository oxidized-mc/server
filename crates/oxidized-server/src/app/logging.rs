//! Logging initialization for the Oxidized server.
//!
//! Uses [`tracing_subscriber`] with structured logging and env-filter
//! support, providing structured logging with env-filter-based level control.

use tracing_subscriber::EnvFilter;

/// Initializes the global tracing subscriber.
///
/// The log level is determined by:
/// 1. The `RUST_LOG` environment variable (if set)
/// 2. The `default_level` parameter (fallback)
///
/// # Panics
///
/// Panics if a global subscriber has already been set.
pub fn init(default_level: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
