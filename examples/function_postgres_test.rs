use db_testkit::with_test_db;
use tracing::info;

#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
use sqlx::Executor;

#[tokio::main]
async fn main() {
    // Initialize logging for better debugging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    // Run our example test
    test_with_postgres().await;
}

/// Example of using the function-based API for database testing
#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
async fn test_with_postgres() {
    with_test_db!(|db| async move {
        // Setup database
        db.setup(|mut conn| async move {
            sqlx::query(
                "CREATE TABLE users (
                    id SERIAL PRIMARY KEY,
                    email TEXT NOT NULL,
                    name TEXT NOT NULL
                )",
            )
            .execute(&conn)
            .await?;

            // Insert a test user
            sqlx::query("INSERT INTO users (email, name) VALUES ('test@example.com', 'Test User')")
                .execute(&conn)
                .await?;

            Ok(())
        })
        .await?;

        // Execute tests
        db.test(|mut conn| async move {
            // Verify we can query the data
            sqlx::query("SELECT * FROM users").execute(&conn).await?;

            // Run an additional test query
            sqlx::query("SELECT id, email, name FROM users WHERE email = 'test@example.com'")
                .execute(&conn)
                .await?;

            info!("Test completed successfully!");
            Ok(())
        })
        .await?;

        Ok(())
    });
}

// Add a placeholder implementation for when features are not enabled
#[cfg(not(any(feature = "postgres", feature = "sqlx-postgres")))]
async fn test_with_postgres() {
    println!("This example requires the 'postgres' or 'sqlx-postgres' feature to be enabled");
}
