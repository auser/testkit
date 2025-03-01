//! Tracing utilities for the library

// Re-export the external tracing crate
pub use ::tracing::*;

/// Initialize tracing for the application.
/// Only initializes if RUST_ENV is set to "DEBUG"
pub fn init_tracing() {
    // Only initialize tracing if RUST_ENV is set to "DEBUG"
    if let Ok(env) = std::env::var("RUST_ENV") {
        if env == "DEBUG" {
            // Set RUST_LOG if not already set
            if std::env::var("RUST_LOG").is_err() {
                std::env::set_var("RUST_LOG", "db_testkit=debug,info");
            }
            let _ = ::tracing_subscriber::fmt::try_init();
        }
    }
}
