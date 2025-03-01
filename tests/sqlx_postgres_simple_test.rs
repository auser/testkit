//! Integration tests for SQLx Postgres simple usage

#[cfg(feature = "sqlx-postgres")]
mod sqlx_postgres_simple_tests {
    use db_testkit::{
        backends::SqlxPostgresBackend, env::get_sqlx_postgres_url, init_tracing,
        test_db::TestDatabase, PoolConfig, Result,
    };
    use sqlx::Row;
    use tracing::info;

    #[tokio::test]
    async fn test_sqlx_postgres_simple() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=info");
        }
        init_tracing();

        info!("Starting SQLx PostgreSQL simple test");

        // Create the test database directly
        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url()?)?;
        let test_db = TestDatabase::new(backend, PoolConfig::default()).await?;

        let db_name = test_db.db_name.clone();
        info!("Created test database: {}", db_name);

        // Get the SQLx pool directly
        let sqlx_pool = test_db.pool.sqlx_pool();

        // Create a table
        sqlx::query("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT)")
            .execute(sqlx_pool)
            .await?;
        info!("Created table");

        // Insert data
        sqlx::query("INSERT INTO users (name, email) VALUES ('John Doe', 'john@example.com')")
            .execute(sqlx_pool)
            .await?;
        info!("Inserted data");

        // Query data
        let row = sqlx::query("SELECT name, email FROM users WHERE name = 'John Doe'")
            .fetch_one(sqlx_pool)
            .await?;

        let name = row.get::<String, _>("name");
        let email = row.get::<String, _>("email");

        info!("Name: {}, Email: {}", name, email);

        assert_eq!(name, "John Doe", "Expected name to be 'John Doe'");
        assert_eq!(
            email, "john@example.com",
            "Expected email to be 'john@example.com'"
        );

        info!("SQLx PostgreSQL simple test completed successfully");
        Ok(())
    }
}
