#[cfg(feature = "sqlx-sqlite")]
use db_testkit::error::Result;
#[cfg(feature = "sqlx-sqlite")]
use db_testkit::prelude::*;

#[cfg(feature = "sqlx-sqlite")]
#[tokio::main]
async fn main() {
    // Setup logging
    std::env::set_var("RUST_LOG", "sqlx=debug");
    let _ = tracing_subscriber::fmt::try_init();

    println!("Testing the with_test_db! macro with SQLite...");

    with_test_db!(|db| async move {
        println!("Created template database: {}", db.name());

        // Create a test database from the template
        let test_db = db.create_test_database().await.unwrap();
        println!("Created test database: {}", test_db.db_name);

        // Get a connection
        let mut conn = test_db.pool.acquire().await.unwrap();

        // Create a table
        conn.execute("CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .await
            .unwrap();

        println!("Created table");

        // Insert some data
        conn.execute("INSERT INTO test_items (name) VALUES ('Test Item 1')")
            .await
            .unwrap();

        println!("Inserted data");

        // Return successfully
        Ok(()) as Result<()>
    });

    println!("SQLite macro test completed!");
}

#[cfg(not(feature = "sqlx-sqlite"))]
fn main() {
    println!("This example requires the sqlx-sqlite feature");
}
