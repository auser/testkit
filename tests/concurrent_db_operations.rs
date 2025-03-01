//! Integration tests for concurrent database operations

mod common;

#[cfg(feature = "postgres")]
mod postgres_concurrent_tests {
    use db_testkit::{
        backend::{Connection, DatabaseBackend, DatabasePool},
        backends::PostgresBackend,
        env::get_postgres_url,
        error::DbError,
        migrations::RunSql,
        PoolConfig, SqlSource, TestDatabase, TestDatabaseTemplate,
    };
    use tracing::{error, warn};

    use crate::common::{init_tracing, SQL_SCRIPTS};

    #[tokio::test]
    async fn test_postgres_parallel_databases() {
        init_tracing();

        // Create a single database with the necessary schema
        let backend = PostgresBackend::new(&get_postgres_url().unwrap())
            .await
            .unwrap();

        // Create a single test database
        let db = TestDatabase::new(backend, PoolConfig::default())
            .await
            .unwrap();

        // Set up the schema in the test database
        let mut conn = db.pool.acquire().await.unwrap();
        conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
            .await
            .unwrap();

        // Save the db backend and name for potential cleanup
        let backend_copy = db.backend.clone();
        let db_name = db.db_name.clone();

        // Wrap test logic in catch_unwind for panic safety
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
            // Create multiple connections to the same database
            let mut handles = vec![];
            for i in 0..5 {
                let pool = db.pool.clone();
                handles.push(tokio::spawn(async move {
                    let mut conn = pool.acquire().await.unwrap();

                    // Use transactions for isolation
                    let tx = conn.begin().await.unwrap();

                    // Insert data in the transaction
                    let email = format!("test{}@example.com", i);
                    let name = format!("Test User {}", i);
                    let sql = format!(
                        "INSERT INTO users (email, name) VALUES ('{}', '{}')",
                        email, name
                    );
                    tx.execute(&sql, &[]).await.unwrap();

                    // Commit the transaction
                    tx.commit().await.unwrap();

                    // Verify the data exists outside the transaction
                    let query_sql = format!("SELECT name FROM users WHERE email = '{}'", email);
                    let row = conn.fetch_one(&query_sql).await.unwrap();

                    let name_from_db: String = row.get("name");
                    assert_eq!(name_from_db, format!("Test User {}", i));

                    i // Return the index for verification
                }));
            }

            // Wait for all operations to complete
            let results: Vec<usize> = futures::future::join_all(handles)
                .await
                .into_iter()
                .map(|r| r.unwrap())
                .collect();

            // Verify all indices were processed
            let mut indices = results.clone();
            indices.sort();
            assert_eq!(indices, vec![0, 1, 2, 3, 4]);

            // Final verification - count the rows
            let mut conn = db.pool.acquire().await.unwrap();
            let row = conn
                .fetch_one("SELECT COUNT(*) as count FROM users")
                .await
                .unwrap();

            let count: i64 = row.get(0);
            assert_eq!(count, 5);

            // Return unit type to avoid mismatched types in the future.await
            Ok::<(), DbError>(())
        }));

        // Handle any panics to ensure database cleanup
        match result {
            Ok(future) => {
                if let Err(e) = future.await {
                    error!("Test failed: {:?}", e);
                    // Explicitly drop the database before panicking
                    if let Err(drop_err) =
                        DatabaseBackend::drop_database(&backend_copy, &db_name).await
                    {
                        warn!("Warning: failed to drop database: {}", drop_err);
                    }
                    panic!("Test failed: {:?}", e);
                }
            }
            Err(e) => {
                // Explicitly drop the database before re-panicking
                error!("Test panicked, ensuring database cleanup");
                if let Err(drop_err) = DatabaseBackend::drop_database(&backend_copy, &db_name).await
                {
                    error!(
                        "Warning: failed to drop database during panic recovery: {}",
                        drop_err
                    );
                }
                // Re-panic with the original error
                std::panic::resume_unwind(e);
            }
        }
    }

    #[tokio::test]
    async fn test_postgres_concurrent_operations() {
        init_tracing();

        let backend = PostgresBackend::new(&get_postgres_url().unwrap())
            .await
            .unwrap();
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        // Initialize template
        template
            .initialize(|mut conn| async move {
                conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
                    .await?;
                Ok(())
            })
            .await
            .unwrap();

        // Get a single database
        let db = template.create_test_database().await.unwrap();

        // Run concurrent operations on the same database
        let mut handles = vec![];
        for i in 0..5 {
            let pool = db.pool.clone();
            handles.push(tokio::spawn(async move {
                let mut conn = pool.acquire().await.unwrap();
                conn.execute(&format!(
                    "INSERT INTO users (email, name) VALUES ('concurrent{}@example.com', 'Concurrent User {}')",
                    i, i
                ))
                .await
                .unwrap();
            }));
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all data was inserted
        let mut conn = db.pool.acquire().await.unwrap();
        for i in 0..5 {
            conn.execute(&format!(
                "SELECT * FROM users WHERE email = 'concurrent{}@example.com'",
                i
            ))
            .await
            .unwrap();
        }
    }
}
