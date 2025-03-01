#[cfg(feature = "sqlx-sqlite")]
use db_testkit::prelude::*;
#[cfg(feature = "sqlx-sqlite")]
use tracing::info;

#[cfg(feature = "sqlx-sqlite")]
#[tokio::main]
async fn main() -> std::result::Result<(), db_testkit::DbError> {
    // Setup logging
    std::env::set_var("RUST_LOG", "sqlx=debug,db_testkit=info");
    let _ = tracing_subscriber::fmt::try_init();

    info!("Starting SQLite test...");

    // Create a SQLite backend
    let backend = SqliteBackend::new("/tmp/simple_sqlite_test")
        .await
        .expect("Failed to create SQLite backend");

    // Create a test database template
    let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
        .await
        .expect("Failed to create template");

    info!("Created template database: {}", template.name());

    // Create a test database from the template
    let test_db = template
        .create_test_database()
        .await
        .expect("Failed to create test database");
    info!("Created test database: {}", test_db.db_name);

    // Get a connection
    let mut conn = test_db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Create a table
    conn.execute("CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .await
        .expect("Failed to create table");

    info!("Created table");

    // Insert some data
    conn.execute("INSERT INTO test_items (name) VALUES ('Test Item 1')")
        .await
        .expect("Failed to insert data");

    info!("Inserted data");

    info!("SQLite test completed successfully!");
    Ok(())
}

#[cfg(not(feature = "sqlx-sqlite"))]
fn main() {
    // Use println here since tracing may not be initialized in this case
    println!("This example requires the sqlx-sqlite feature");
}
