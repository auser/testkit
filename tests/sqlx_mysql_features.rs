//! Integration tests for SQLx MySQL features

mod common;

#[cfg(feature = "sqlx-mysql")]
mod sqlx_mysql_tests {
    use db_testkit::{
        backend::{Connection, DatabasePool},
        backends::SqlxMySqlBackend,
        env::get_mysql_url,
        migrations::RunSql,
        PoolConfig, SqlSource, TestDatabaseTemplate,
    };
    use tracing::info;

    use crate::common::{init_tracing, SQL_SCRIPTS};

    #[tokio::test]
    async fn test_sqlx_mysql_template() {
        init_tracing();

        let backend = SqlxMySqlBackend::new(&get_mysql_url().unwrap()).unwrap();

        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .unwrap();

        // Initialize template with SQL scripts
        template
            .initialize(|mut conn| async move {
                conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
                    .await?;
                Ok(())
            })
            .await
            .unwrap();

        // Get two separate databases
        let db1 = template.create_test_database().await.unwrap();
        let db2 = template.create_test_database().await.unwrap();

        // Log database names for debugging
        info!("Created database 1: {}", db1.name);
        info!("Created database 2: {}", db2.name);

        // Verify they are separate
        let mut conn1 = db1.pool.acquire().await.unwrap();
        let mut conn2 = db2.pool.acquire().await.unwrap();

        // Insert into db1
        conn1
            .execute("INSERT INTO users (email, name) VALUES ('test1@example.com', 'Test User 1')")
            .await
            .unwrap();

        // Insert into db2
        conn2
            .execute("INSERT INTO users (email, name) VALUES ('test2@example.com', 'Test User 2')")
            .await
            .unwrap();

        // Verify data is separate
        let rows1 = conn1
            .fetch("SELECT email FROM users WHERE email = 'test1@example.com'")
            .await
            .unwrap();
        assert_eq!(
            rows1.len(),
            1,
            "Should find test1@example.com in database 1"
        );

        let rows2 = conn2
            .fetch("SELECT email FROM users WHERE email = 'test2@example.com'")
            .await
            .unwrap();
        assert_eq!(
            rows2.len(),
            1,
            "Should find test2@example.com in database 2"
        );

        // Verify data isolation
        let rows3 = conn1
            .fetch("SELECT email FROM users WHERE email = 'test2@example.com'")
            .await
            .unwrap();
        assert_eq!(
            rows3.len(),
            0,
            "Should NOT find test2@example.com in database 1"
        );

        let rows4 = conn2
            .fetch("SELECT email FROM users WHERE email = 'test1@example.com'")
            .await
            .unwrap();
        assert_eq!(
            rows4.len(),
            0,
            "Should NOT find test1@example.com in database 2"
        );
    }

    #[tokio::test]
    async fn test_sqlx_mysql_auto_cleanup() {
        init_tracing();

        // Generate a unique prefix for this test run
        let prefix = format!("testkit_cleanup_{}", std::process::id());
        info!("Using test prefix: {}", prefix);

        {
            // Create a backend with our custom prefix
            let backend =
                SqlxMySqlBackend::new_with_prefix(&get_mysql_url().unwrap(), &prefix).unwrap();

            // Create a test database
            let db = backend.create_database("auto_cleanup").await.unwrap();
            info!("Created database: {}", db.name);

            // Verify connection works
            let mut conn = db.pool.acquire().await.unwrap();
            conn.execute("SELECT 1").await.unwrap();

            // Database should be dropped when it goes out of scope
        }

        // Connect to MySQL server to verify cleanup
        let backend = SqlxMySqlBackend::new(&get_mysql_url().unwrap()).unwrap();
        let server_conn = backend.connect_to_server().await.unwrap();

        // Check if any databases with our prefix still exist
        let mut conn = server_conn.acquire().await.unwrap();
        let query = format!("SHOW DATABASES LIKE '{}%'", prefix);
        let remaining_dbs = conn.fetch(&query).await.unwrap();

        // Assert that no test databases remain
        assert_eq!(
            remaining_dbs.len(),
            0,
            "All databases with prefix '{}' should have been cleaned up, found: {:?}",
            prefix,
            remaining_dbs
        );
    }
}
