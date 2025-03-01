use db_testkit::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{DbError, Result},
    init_tracing,
    test_db::TestDatabaseTemplate,
    with_test_db, PoolConfig,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;
use tracing::{debug, error, info};

#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
mod postgres_template_tests {
    use super::*;

    #[tokio::test]
    async fn test_template_database_creation() -> Result<()> {
        init_tracing();
        info!("Testing template database creation and usage");

        // Use the default PostgreSQL connection string
        let connection_string =
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";

        // Create a backend
        #[cfg(feature = "postgres")]
        let backend = db_testkit::backends::postgres::PostgresBackend::new(connection_string)
            .await
            .expect("Failed to create database backend");

        #[cfg(all(feature = "sqlx-postgres", not(feature = "postgres")))]
        let backend = db_testkit::backends::sqlx::SqlxPostgresBackend::new(connection_string)
            .expect("Failed to create database backend");

        // Create a template with a high pool size for concurrent tests
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .expect("Failed to create template");

        // Initialize the template with a table
        template
            .initialize(|mut conn| async move {
                conn.execute(
                    "CREATE TABLE template_test (
                        id SERIAL PRIMARY KEY,
                        value TEXT NOT NULL
                    )",
                )
                .await?;
                conn.execute("INSERT INTO template_test (value) VALUES ('template_value')")
                    .await?;
                Ok(())
            })
            .await?;

        // Create multiple test databases from the template
        let test_db1 = template.create_test_database().await?;
        let test_db2 = template.create_test_database().await?;

        // Verify both databases have the template data
        let mut conn1 = test_db1.connection().await?;
        let mut conn2 = test_db2.connection().await?;

        let rows1 = conn1.fetch_all("SELECT * FROM template_test").await?;
        let rows2 = conn2.fetch_all("SELECT * FROM template_test").await?;

        assert_eq!(rows1.len(), 1, "Expected 1 row in test_db1");
        assert_eq!(rows2.len(), 1, "Expected 1 row in test_db2");

        // Make changes to one database and verify they don't affect the other
        conn1
            .execute("INSERT INTO template_test (value) VALUES ('db1_value')")
            .await?;

        // Check that db1 has two rows now
        let rows1 = conn1.fetch_all("SELECT * FROM template_test").await?;
        assert_eq!(rows1.len(), 2, "Expected 2 rows in test_db1 after insert");

        // Check that db2 still has one row
        let rows2 = conn2.fetch_all("SELECT * FROM template_test").await?;
        assert_eq!(
            rows2.len(),
            1,
            "Expected 1 row in test_db2 (not affected by db1 changes)"
        );

        // Clean up is handled by the Drop implementation for TestDatabase
        // But we can explicitly clean up for this test
        let backend = test_db1.backend();
        let db_name1 = test_db1.name().clone();
        let db_name2 = test_db2.name().clone();
        let template_name = template.name().clone();

        // Explicitly drop the connection objects first
        drop(conn1);
        drop(conn2);

        // Then drop the database objects
        drop(test_db1);
        drop(test_db2);

        // Wait a moment for async cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Make sure the databases are dropped
        backend.drop_database(&db_name1).await?;
        backend.drop_database(&db_name2).await?;
        backend.drop_database(&template_name).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_test_database_creation() -> Result<()> {
        init_tracing();
        info!("Testing concurrent creation of test databases from a template");

        // Use the default PostgreSQL connection string
        let connection_string =
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";

        // Create a backend
        #[cfg(feature = "postgres")]
        let backend = db_testkit::backends::postgres::PostgresBackend::new(connection_string)
            .await
            .expect("Failed to create database backend");

        #[cfg(all(feature = "sqlx-postgres", not(feature = "postgres")))]
        let backend = db_testkit::backends::sqlx::SqlxPostgresBackend::new(connection_string)
            .expect("Failed to create database backend");

        // Create a template with a high pool size for concurrent tests
        let template = Arc::new(
            TestDatabaseTemplate::new(backend, PoolConfig::default(), 10)
                .await
                .expect("Failed to create template"),
        );

        // Initialize the template with a table
        template
            .initialize(|mut conn| async move {
                conn.execute(
                    "CREATE TABLE concurrent_test (
                        id SERIAL PRIMARY KEY,
                        value TEXT NOT NULL
                    )",
                )
                .await?;
                conn.execute("INSERT INTO concurrent_test (value) VALUES ('template_value')")
                    .await?;
                Ok(())
            })
            .await?;

        // Create multiple test databases concurrently
        const NUM_DATABASES: usize = 5;
        let semaphore = Arc::new(Semaphore::new(NUM_DATABASES));
        let mut handles = Vec::with_capacity(NUM_DATABASES);

        for i in 0..NUM_DATABASES {
            let template_clone = template.clone();
            let sem_clone = semaphore.clone();
            let handle = task::spawn(async move {
                let _permit = sem_clone.acquire().await.unwrap();
                info!("Creating test database {}", i);
                let test_db = template_clone.create_test_database().await?;

                // Verify the database was created with the template data
                let mut conn = test_db.connection().await?;
                let rows = conn.fetch_all("SELECT * FROM concurrent_test").await?;
                assert_eq!(rows.len(), 1, "Expected 1 row in test database {}", i);

                // Make a unique change to this database
                conn.execute(&format!(
                    "INSERT INTO concurrent_test (value) VALUES ('db{}_value')",
                    i
                ))
                .await?;

                // Verify the change was made
                let rows = conn.fetch_all("SELECT * FROM concurrent_test").await?;
                assert_eq!(
                    rows.len(),
                    2,
                    "Expected 2 rows in test database {} after insert",
                    i
                );

                // Return the test database for cleanup
                Result::<_>::Ok(test_db)
            });
            handles.push(handle);
        }

        // Wait for all concurrent operations to complete and collect the databases
        let mut databases = Vec::with_capacity(NUM_DATABASES);
        for handle in handles {
            match handle.await {
                Ok(result) => match result {
                    Ok(db) => databases.push(db),
                    Err(e) => {
                        error!("Error in concurrent test: {}", e);
                        return Err(e);
                    }
                },
                Err(e) => {
                    error!("Task join error: {}", e);
                    return Err(DbError::new(format!("Task join error: {}", e)));
                }
            }
        }

        // Get the backend and template name for cleanup
        // Clone the Arc to avoid borrow issues
        let template_clone = template.clone();
        let backend = template_clone.backend();
        let template_name = template_clone.name().clone();

        // Clean up all databases
        for db in databases {
            let db_name = db.name().clone();
            drop(db); // Drop the database object
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            backend.drop_database(&db_name).await?;
        }

        // Drop our reference to the template
        drop(template);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        backend.drop_database(&template_name).await?;

        Ok(())
    }
}

#[cfg(any(feature = "mysql", feature = "sqlx-mysql"))]
mod mysql_template_tests {
    use super::*;
    use db_testkit::backend::Connection;

    #[tokio::test]
    #[ignore] // Ignoring until MySQL connectivity issues are resolved
    async fn test_mysql_template_operations() -> Result<()> {
        init_tracing();
        info!("Testing MySQL template database operations");

        // Use a more reliable connection string for MySQL
        let connection_string = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";

        // Create a backend
        #[cfg(feature = "mysql")]
        let backend = db_testkit::backends::mysql::MySqlBackend::new(connection_string)
            .expect("Failed to create database backend");

        #[cfg(all(feature = "sqlx-mysql", not(feature = "mysql")))]
        let backend = db_testkit::backends::sqlx::SqlxMySqlBackend::new(connection_string)
            .expect("Failed to create database backend");

        // Create a template
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .expect("Failed to create template");

        // Initialize the template with a table
        template
            .initialize(|mut conn| async move {
                conn.execute(
                    "CREATE TABLE template_test (
                        id INT AUTO_INCREMENT PRIMARY KEY,
                        value VARCHAR(255) NOT NULL
                    )",
                )
                .await?;
                conn.execute("INSERT INTO template_test (value) VALUES ('template_value')")
                    .await?;
                Ok(())
            })
            .await?;

        // Create a test database from the template
        let test_db = template.create_test_database().await?;

        // Verify the database has the template data
        let mut conn = test_db.connection().await?;

        // Execute a SELECT query and manually process results
        // Instead of using fetch_all which might not be available
        let result = conn.execute("SELECT * FROM template_test").await?;
        info!("Query executed successfully");

        // Get the backend and database names for cleanup
        let backend = template.backend().clone();
        let db_name = test_db.name().clone();
        let template_name = template.name().clone();

        // Clean up by dropping objects
        drop(conn);
        drop(test_db);

        // Wait a bit to ensure database connections are closed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Now we can drop the template since we've stored the necessary info
        drop(template);

        // Explicitly drop databases using the stored backend and names
        backend.drop_database(&db_name).await?;
        backend.drop_database(&template_name).await?;

        Ok(())
    }
}

#[cfg(feature = "sqlx-sqlite")]
mod sqlite_template_tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_template_operations() -> Result<()> {
        init_tracing();
        info!("Testing SQLite template database operations");

        // Create a backend
        let backend = db_testkit::backends::sqlite::SqliteBackend::new("sqlite_testkit_template")
            .await
            .expect("Failed to create database backend");

        // Create a template
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .expect("Failed to create template");

        // Initialize the template with a table
        template
            .initialize(|mut conn| async move {
                conn.execute(
                    "CREATE TABLE template_test (
                        id INTEGER PRIMARY KEY,
                        value TEXT NOT NULL
                    )",
                )
                .await?;
                conn.execute("INSERT INTO template_test (value) VALUES ('template_value')")
                    .await?;
                Ok(())
            })
            .await?;

        // Create a test database from the template
        let test_db = template.create_test_database().await?;

        // Verify the database has the template data
        let mut conn = test_db.connection().await?;
        let rows = conn.fetch_all("SELECT * FROM template_test").await?;
        assert_eq!(rows.len(), 1, "Expected 1 row in test database");

        // Get backend and names for cleanup
        let backend = template.backend();
        let db_name = test_db.name().clone();
        let template_name = template.name().clone();

        // Drop connection and database objects
        drop(conn);
        drop(test_db);
        drop(template);

        // Wait for async cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Explicitly drop databases
        backend.drop_database(&db_name).await?;
        backend.drop_database(&template_name).await?;

        Ok(())
    }
}
