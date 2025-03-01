//! Integration tests for MySQL simple usage

#[cfg(feature = "mysql")]
mod mysql_simple_tests {
    use db_testkit::{
        backend::Connection, backends::mysql::MySqlBackend, env::get_mysql_url, init_tracing,
        test_db::TestDatabase, PoolConfig, Result,
    };
    use tracing::info;

    #[tokio::test]
    async fn test_mysql_simple() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,mysql=info");
        }
        init_tracing();

        info!("Starting MySQL simple test");

        // Create the test database directly
        let backend = MySqlBackend::new(&get_mysql_url()?)?;
        let test_db = TestDatabase::new(backend, PoolConfig::default()).await?;

        let db_name = test_db.db_name.clone();
        info!("Created test database: {}", db_name);

        // Get a connection and run some queries
        let mut conn = test_db.connection().await?;

        // Create a table
        conn.execute(
            "CREATE TABLE test_table (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(255))",
        )
        .await?;
        info!("Created test table");

        // Insert data
        conn.execute("INSERT INTO test_table (name) VALUES ('Alice'), ('Bob'), ('Charlie')")
            .await?;
        info!("Inserted test data");

        // Query the data to verify
        let rows = conn
            .fetch("SELECT id, name FROM test_table ORDER BY id")
            .await?;

        // Display the results
        info!("Query results:");
        for row in rows {
            let id: i32 = row.get(0).unwrap();
            let name: String = row.get(1).unwrap();
            info!("  id: {}, name: {}", id, name);
        }

        // Count the rows
        let count_rows = conn.fetch("SELECT COUNT(*) FROM test_table").await?;
        let count: i64 = count_rows[0].get(0).unwrap_or(0);
        info!("Total rows: {}", count);

        assert_eq!(count, 3, "Expected 3 rows in the table");

        info!("MySQL simple test completed successfully");
        Ok(())
    }
}
