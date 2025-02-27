#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
use db_testkit::prelude::*;

#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
#[tokio::main]
async fn main() -> std::result::Result<(), db_testkit::PoolError> {
    use sqlx::Row;

    println!("Running the example with SQLx PostgreSQL backend...");

    // Use the macro without explicit type annotations
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |db| async {
            println!("Created test database: {}", db.name());

            // Create a test database from the template
            let test_db = db.create_test_database().await.unwrap();
            println!("Created test database");

            // Create a table
            sqlx::query("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT)")
                .execute(&test_db.pool)
                .await
                .unwrap();

            println!("Created table");

            // Insert data
            sqlx::query("INSERT INTO users (name, email) VALUES ('John Doe', 'john@example.com')")
                .execute(&test_db.pool)
                .await
                .unwrap();

            println!("Inserted data");

            // Query data
            let row = sqlx::query("SELECT name, email FROM users WHERE name = 'John Doe'")
                .fetch_one(&test_db.pool)
                .await
                .unwrap();

            println!(
                "Name: {}, Email: {}",
                row.get::<String, _>("name"),
                row.get::<String, _>("email")
            );

            // Note: We still need to specify the Result type for the return value
            // But we don't need to specify any TestDatabaseTemplate types
            Ok(()) as std::result::Result<(), ()>
        }
    );

    Ok(())
}

#[cfg(not(all(feature = "sqlx-backend", not(feature = "postgres"))))]
fn main() {
    println!("This example requires the sqlx-backend feature but not the postgres feature");
}
