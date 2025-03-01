//! Common utilities and constants for tests

#[allow(dead_code)]
pub const SQL_SCRIPTS: &[&str] = &[
    r#"
    CREATE TABLE users (
        id SERIAL PRIMARY KEY,
        email VARCHAR(255) UNIQUE NOT NULL,
        name VARCHAR(255) NOT NULL
    );
    "#,
    r#"
    ALTER TABLE users ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    "#,
];

/// Initialize tracing for tests if it hasn't been already
#[allow(dead_code)]
pub fn init_tracing() {
    // Delegate to the central init_tracing function
    db_testkit::init_tracing();
}
