//! Integration tests for SQLx PostgreSQL features

mod common;

#[cfg(feature = "sqlx-postgres")]
mod sqlx_postgres_tests {
    use db_testkit::{
        backend::DatabaseBackend, backends::SqlxPostgresBackend, env::get_sqlx_postgres_url,
        migrations::RunSql, PoolConfig, SqlSource, TestDatabaseTemplate,
    };
    use sqlx::{Executor, Row};
    use tracing::info;

    use crate::common::{init_tracing, SQL_SCRIPTS};

    #[tokio::test]
    async fn test_sqlx_postgres_template() {
        init_tracing();

        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url().unwrap()).unwrap();

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
        info!("Created database 1: {}", db1.db_name);
        info!("Created database 2: {}", db2.db_name);

        // Get SQLx pools directly for cleaner access
        let pool1 = db1.pool.sqlx_pool();
        let pool2 = db2.pool.sqlx_pool();

        // Insert into db1
        sqlx::query("INSERT INTO users (email, name) VALUES ('test1@example.com', 'Test User 1')")
            .execute(pool1)
            .await
            .unwrap();

        // Insert into db2
        sqlx::query("INSERT INTO users (email, name) VALUES ('test2@example.com', 'Test User 2')")
            .execute(pool2)
            .await
            .unwrap();

        // Verify data is separate
        let count1 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test1@example.com'",
        )
        .fetch_one(pool1)
        .await
        .unwrap();
        assert_eq!(count1.0, 1, "Should find test1@example.com in database 1");

        let count2 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test2@example.com'",
        )
        .fetch_one(pool2)
        .await
        .unwrap();
        assert_eq!(count2.0, 1, "Should find test2@example.com in database 2");

        // Verify data isolation
        let count3 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test2@example.com'",
        )
        .fetch_one(pool1)
        .await
        .unwrap();
        assert_eq!(
            count3.0, 0,
            "Should NOT find test2@example.com in database 1"
        );

        let count4 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test1@example.com'",
        )
        .fetch_one(pool2)
        .await
        .unwrap();
        assert_eq!(
            count4.0, 0,
            "Should NOT find test1@example.com in database 2"
        );
    }

    #[tokio::test]
    async fn test_sqlx_postgres_transactions() {
        init_tracing();

        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url().unwrap()).unwrap();

        // Create a DatabaseName instance
        let db_name = db_testkit::test_db::DatabaseName::new(Some("testkit_txn_test"));

        // Create the database properly
        backend.create_database(&db_name).await.unwrap();

        // Create pool for the database
        let pool_config = PoolConfig::default();
        let pool = backend.create_pool(&db_name, &pool_config).await.unwrap();

        // Get the SQLx pool to work with directly
        let sqlx_pool = pool.sqlx_pool();

        // Create table directly using SQLx
        sqlx::query(
            "CREATE TABLE users (
                id SERIAL PRIMARY KEY,
                email VARCHAR(255) UNIQUE NOT NULL,
                name VARCHAR(255) NOT NULL
            )",
        )
        .execute(sqlx_pool)
        .await
        .unwrap();

        // Test transaction commit
        {
            // Start a transaction
            let mut tx = sqlx_pool.begin().await.unwrap();

            // Insert data - directly use the transaction's execute method
            tx.execute(
                "INSERT INTO users (email, name) VALUES ('commit@example.com', 'Commit User')",
            )
            .await
            .unwrap();

            // Commit transaction
            tx.commit().await.unwrap();
        }

        // Test transaction rollback
        {
            // Start a transaction
            let mut tx = sqlx_pool.begin().await.unwrap();

            // Insert data - directly use the transaction's execute method
            tx.execute(
                "INSERT INTO users (email, name) VALUES ('rollback@example.com', 'Rollback User')",
            )
            .await
            .unwrap();

            // Check data is visible within transaction - use direct method
            let row = tx
                .fetch_one("SELECT COUNT(*) FROM users WHERE email = 'rollback@example.com'")
                .await
                .unwrap();
            let count: i64 = row.try_get(0).unwrap();
            assert_eq!(count, 1, "Data should be visible within transaction");

            // Rollback by dropping the transaction
            // tx.rollback().await.unwrap();
            drop(tx);
        }

        // Verify committed data exists but rolled back data doesn't
        let committed = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'commit@example.com'",
        )
        .fetch_one(sqlx_pool)
        .await
        .unwrap();
        assert_eq!(committed.0, 1, "Committed data should exist");

        let rolled_back = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'rollback@example.com'",
        )
        .fetch_one(sqlx_pool)
        .await
        .unwrap();
        assert_eq!(rolled_back.0, 0, "Rolled back data should not exist");

        // Clean up the database at the end of the test
        // Manually drop the pool first to close connections
        drop(pool);

        // Now drop the database
        let connection_string = backend.connection_string(&db_name);
        if let Err(e) = db_testkit::test_db::sync_drop_database(&connection_string) {
            tracing::error!("Failed to drop test database: {}", e);
        } else {
            tracing::info!("Successfully dropped test database: {}", db_name);
        }
    }
}
