//! Integration tests for SQLx SQLite features

mod common;

#[cfg(feature = "sqlx-sqlite")]
mod sqlx_sqlite_tests {
    use db_testkit::{
        backend::{Connection, DatabasePool},
        backends::SqlxSqliteBackend,
        migrations::RunSql,
        PoolConfig, SqlSource, TestDatabaseTemplate,
    };
    use std::path::Path;
    use tempfile::tempdir;
    use tracing::info;

    use crate::common::{init_tracing, SQL_SCRIPTS};

    #[tokio::test]
    async fn test_sqlx_sqlite_template() {
        init_tracing();

        let backend = SqlxSqliteBackend::new().unwrap();

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

        // Log database paths for debugging
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
    async fn test_sqlx_sqlite_custom_dir() {
        init_tracing();

        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap();
        info!("Using temporary directory: {}", dir_path);

        // Create backend with custom directory
        let backend = SqlxSqliteBackend::new_with_dir(dir_path).unwrap();

        // Create a database
        let db = backend.create_database("custom_dir_test").await.unwrap();
        info!("Created database: {}", db.name);

        // Verify the database file exists in our custom directory
        let db_path = Path::new(dir_path).join(format!("{}.db", db.name));
        assert!(
            db_path.exists(),
            "Database file should exist at {:?}",
            db_path
        );

        // Create table and insert data
        let mut conn = db.pool.acquire().await.unwrap();
        conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
            .await
            .unwrap();

        conn.execute(
            "INSERT INTO users (email, name) VALUES ('sqlite@example.com', 'SQLite User')",
        )
        .await
        .unwrap();

        // Query the data back
        let rows = conn.fetch("SELECT email, name FROM users").await.unwrap();

        assert_eq!(rows.len(), 1, "Should have one row");
        assert_eq!(
            rows[0].get::<String>("email").unwrap(),
            "sqlite@example.com",
            "Email should match"
        );
        assert_eq!(
            rows[0].get::<String>("name").unwrap(),
            "SQLite User",
            "Name should match"
        );

        // Drop the connection and database
        drop(conn);
        drop(db);

        // Verify cleanup (the file should still exist since we're using a custom directory)
        assert!(
            db_path.exists(),
            "Database file should still exist after drop"
        );

        // Cleanup temp directory when it goes out of scope
    }
}
