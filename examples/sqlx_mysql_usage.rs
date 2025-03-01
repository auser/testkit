//! SQLx MySQL test example
//!
//! This example demonstrates how to use db-testkit with SQLx MySQL.
//! Run this example with: cargo run --features "sqlx-mysql sqlx-backend" --example sqlx_mysql_usage

#[cfg(all(feature = "sqlx-mysql", feature = "sqlx-backend"))]
use db_testkit::prelude::*;
#[cfg(all(feature = "sqlx-mysql", feature = "sqlx-backend"))]
use tracing::info;

#[cfg(all(feature = "sqlx-mysql", feature = "sqlx-backend"))]
#[tokio::main]
async fn main() -> std::result::Result<(), db_testkit::DbError> {
    use sqlx::Row;

    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=info");
    }
    let _ = tracing_subscriber::fmt::try_init();

    info!("Running the example with SQLx MySQL backend...");

    // Use the macro with MySQL connection string
    with_test_db!(
        "mysql://root:@mysql:3306",
        |db| async {
            info!("Created test database: {}", db.name());

            // Create a table - use db directly as it's already a TestDatabase
            sqlx::query("CREATE TABLE users (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(255), email VARCHAR(255))")
                .execute(db.pool.sqlx_pool())
                .await
                .unwrap();

            info!("Created table");

            // Insert data
            sqlx::query("INSERT INTO users (name, email) VALUES (?, ?), (?, ?), (?, ?)")
                .bind("Alice")
                .bind("alice@example.com")
                .bind("Bob")
                .bind("bob@example.com")
                .bind("Charlie")
                .bind("charlie@example.com")
                .execute(db.pool.sqlx_pool())
                .await
                .unwrap();

            info!("Inserted data");

            // Query data
            let rows = sqlx::query("SELECT id, name, email FROM users ORDER BY id")
                .fetch_all(db.pool.sqlx_pool())
                .await
                .unwrap();

            // Display the results
            info!("Query results:");
            for row in rows {
                let id: i32 = row.get("id");
                let name: String = row.get("name");
                let email: String = row.get("email");
                info!("  id: {}, name: {}, email: {}", id, name, email);
            }

            // Count the rows
            let row = sqlx::query("SELECT COUNT(*) as count FROM users")
                .fetch_one(db.pool.sqlx_pool())
                .await
                .unwrap();
            let count: i64 = row.get("count");
            info!("Total rows: {}", count);

            info!("SQLx MySQL example completed successfully");
            
            // Return a typed result
            let result: Result<()> = Ok(());
            result
        }
    )
    .await?;

    Ok(())
}

#[cfg(not(all(feature = "sqlx-mysql", feature = "sqlx-backend")))]
fn main() {
    // Use println here since tracing may not be initialized in this case
    println!("This example requires both sqlx-mysql and sqlx-backend features");
}
