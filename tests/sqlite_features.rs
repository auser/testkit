//! Integration tests for SQLite features

mod common;

#[cfg(feature = "sqlite")]
mod sqlite_tests {
    use db_testkit::{
        backend::{Connection, DatabasePool},
        backends::SqliteBackend,
        migrations::RunSql,
        PoolConfig, SqlSource, TestDatabaseTemplate,
    };
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    use crate::common::{init_tracing, SQL_SCRIPTS};

    #[tokio::test]
    async fn test_sqlite_template() {
        init_tracing();

        let backend = SqliteBackend::new().unwrap();

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
        assert_eq!(rows1.len(), 1);

        let rows2 = conn1
            .fetch("SELECT email FROM users WHERE email = 'test2@example.com'")
            .await
            .unwrap();
        assert_eq!(rows2.len(), 0); // Should not be found in db1

        let rows3 = conn2
            .fetch("SELECT email FROM users WHERE email = 'test2@example.com'")
            .await
            .unwrap();
        assert_eq!(rows3.len(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_concurrent_connections() {
        init_tracing();

        let backend = SqliteBackend::new().unwrap();
        let db = backend.create_database("concurrent_test").await.unwrap();

        // Create table
        let mut conn = db.pool.acquire().await.unwrap();
        conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
            .await
            .unwrap();

        // Test multiple connections with a semaphore
        let semaphore = Arc::new(Semaphore::new(5)); // Allow 5 concurrent operations
        let mut handles = Vec::new();

        for i in 0..10 {
            let db_clone = db.clone();
            let semaphore_clone = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.unwrap();
                let mut conn = db_clone.pool.acquire().await.unwrap();

                // Use a transaction for isolation
                conn.execute("BEGIN").await.unwrap();

                // Insert data
                conn.execute(&format!(
                    "INSERT INTO users (email, name) VALUES ('user{}@example.com', 'User {}')",
                    i, i
                ))
                .await
                .unwrap();

                // Verify data was inserted
                let rows = conn
                    .fetch(&format!(
                        "SELECT * FROM users WHERE email = 'user{}@example.com'",
                        i
                    ))
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);

                conn.execute("COMMIT").await.unwrap();

                i // Return the index
            });

            handles.push(handle);
        }

        // Wait for all operations to complete
        let results = futures::future::join_all(handles).await;

        // Verify all operations completed successfully
        for result in results {
            assert!(result.is_ok());
        }

        // Verify total count
        let mut conn = db.pool.acquire().await.unwrap();
        let rows = conn
            .fetch("SELECT COUNT(*) as count FROM users")
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
    }
}
