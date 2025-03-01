use db_testkit::with_test_db;

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
async fn test_with_postgres() {
    with_test_db!(|db| async move {
        // Setup database
        db.setup(|mut conn| async move {
            conn.execute(
                "CREATE TABLE users (
                    id SERIAL PRIMARY KEY,
                    email TEXT NOT NULL,
                    name TEXT NOT NULL
                )",
            )
            .await?;

            // Insert a test user
            conn.execute(
                "INSERT INTO users (email, name) VALUES ('test@example.com', 'Test User')",
            )
            .await?;

            Ok(())
        })
        .await?;

        // Execute tests
        db.test(|mut conn| async move {
            // Verify we can query the data
            conn.execute("SELECT * FROM users").await?;

            // Run an additional test query
            conn.execute("SELECT id, email, name FROM users WHERE email = 'test@example.com'")
                .await?;

            info!("Test completed successfully!");
            Ok(())
        })
        .await?;

        Ok(())
    });
}
