#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
use db_testkit::prelude::*;
#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
use tracing::info;

#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
#[tokio::main]
async fn main() -> std::result::Result<(), db_testkit::DbError> {
    use sqlx::Row;

    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=info");
    }
    let _ = tracing_subscriber::fmt::try_init();

    info!("Running the example with SQLx PostgreSQL backend...");

    // Use the macro without explicit type annotations
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |db| async {
            info!("Created test database: {}", db.name());

            // Create a table - use db directly as it's already a TestDatabase
            sqlx::query("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT)")
                .execute(db.pool.sqlx_pool())
                .await
                .unwrap();

            info!("Created table");

            // Insert data
            sqlx::query("INSERT INTO users (name, email) VALUES ('John Doe', 'john@example.com')")
                .execute(db.pool.sqlx_pool())
                .await
                .unwrap();

            info!("Inserted data");

            // Query data
            let row = sqlx::query("SELECT name, email FROM users WHERE name = 'John Doe'")
                .fetch_one(db.pool.sqlx_pool())
                .await
                .unwrap();

            info!(
                "Name: {}, Email: {}",
                row.get::<String, _>("name"),
                row.get::<String, _>("email")
            );

            // Now we can just return a typed result without annotation
            let result: Result<()> = Ok(());
            result
        }
    )
    .await?;

    Ok(())
}

#[cfg(not(all(feature = "sqlx-backend", not(feature = "postgres"))))]
fn main() {
    // Use println here since tracing may not be initialized in this case
    println!("This example requires the sqlx-backend feature but not the postgres feature");
}
