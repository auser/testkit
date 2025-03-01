#[cfg(feature = "sqlx-sqlite")]
use db_testkit::prelude::*;
#[cfg(feature = "sqlx-sqlite")]
use tracing::info;

#[cfg(feature = "sqlx-sqlite")]
#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    std::env::set_var("RUST_LOG", "sqlx=debug,db_testkit=info");
    let _ = tracing_subscriber::fmt::try_init();

    info!("Testing the with_test_db! macro with SQLite...");

    // Assign to underscore to avoid unused Future warning
    with_test_db!(|db| async move {
        info!("Created template database: {}", db.name());

        // Use the db directly - it's already a TestDatabase instance
        info!("Using test database: {}", db.name());

        // Get a connection
        let mut conn = db.connection().await.unwrap();

        // Create a table - in SQLite, INTEGER PRIMARY KEY is alias for rowid which auto-increments
        conn.execute("CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .await
            .unwrap();

        info!("Created table");

        // Insert some data with explicit ID
        conn.execute("INSERT INTO test_items (id, name) VALUES (1, 'Test Item 1')")
            .await
            .unwrap();

        info!("Inserted data");

        // Just return Ok(()) and let the macro handle type inference
        Ok(())
    })
    .await?;

    info!("SQLite macro test completed!");
    Ok(())
}

#[cfg(not(feature = "sqlx-sqlite"))]
fn main() {
    // Use println here since tracing may not be initialized in this case
    println!("This example requires the sqlx-sqlite feature");
}
