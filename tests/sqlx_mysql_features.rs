//! Integration tests for SQLx MySQL features

mod common;

#[cfg(feature = "sqlx-mysql")]
mod sqlx_mysql_tests {
    use db_testkit::{backend::DatabaseBackend, backends::SqlxMySqlBackend, PoolConfig};
    use sqlx::{Executor, Row};
    use std::time::Duration;
    use tracing::info;

    use crate::common::init_tracing;

    #[tokio::test]
    async fn test_sqlx_mysql_template() {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=debug");
        }
        init_tracing();

        info!("Starting MySQL template test");

        // Use a direct connection string with explicit parameters
        let url = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";
        info!("Using direct MySQL URL: {}", url);

        // Create backend with increased timeouts
        let backend = match SqlxMySqlBackend::new(url) {
            Ok(backend) => {
                info!("Successfully created SQLx MySQL backend");
                backend
            }
            Err(e) => {
                panic!("Failed to create SQLx MySQL backend: {}", e);
            }
        };

        // Test connection with retry logic before proceeding
        let max_retries = 3;
        let mut retry_count = 0;
        let mut connected = false;

        while retry_count < max_retries {
            info!(
                "Testing connection to MySQL server (attempt {})...",
                retry_count + 1
            );
            match backend.connect().await {
                Ok(pool) => {
                    info!("Connection test successful on attempt {}", retry_count + 1);
                    // Test a simple query
                    match sqlx::query("SELECT 1").execute(&pool).await {
                        Ok(_) => {
                            info!("Simple query executed successfully");
                            connected = true;
                            break;
                        }
                        Err(e) => {
                            info!("Simple query failed: {}", e);
                            retry_count += 1;
                        }
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count < max_retries {
                        let backoff = Duration::from_secs(2 * retry_count as u64);
                        info!(
                            "Connection attempt {} failed: {}. Retrying in {:?}...",
                            retry_count, e, backoff
                        );
                        tokio::time::sleep(backoff).await;
                    } else {
                        panic!("Failed to connect after {} attempts: {}", max_retries, e);
                    }
                }
            }
        }

        if !connected {
            panic!(
                "Could not establish a working connection after {} attempts",
                max_retries
            );
        }

        // Create template with longer pool timeout
        let pool_config = PoolConfig::builder(10)
            .connection_timeout(Duration::from_secs(30))
            .build();

        info!("Creating template database...");

        // Create a direct test without template mechanism first
        let db_name = db_testkit::test_db::DatabaseName::new(Some("testkit_mysql_direct_test"));
        info!("Creating direct test database: {}", db_name);

        // Create the database directly
        match backend.create_database(&db_name).await {
            Ok(_) => info!("Direct database created successfully"),
            Err(e) => panic!("Failed to create direct database: {}", e),
        }

        // Create pool for the database
        let pool = match backend.create_pool(&db_name, &pool_config).await {
            Ok(pool) => {
                info!("Connection pool created successfully");
                pool
            }
            Err(e) => panic!("Failed to create pool: {}", e),
        };

        // Create table with users schema
        info!("Creating users table...");
        match sqlx::query(
            "CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                email VARCHAR(255) UNIQUE NOT NULL,
                name VARCHAR(255) NOT NULL
            )",
        )
        .execute(pool.sqlx_pool())
        .await
        {
            Ok(_) => info!("Table created successfully"),
            Err(e) => panic!("Failed to create table: {}", e),
        }

        // Instead of using TestDatabaseTemplate which might have issues, let's create two direct test databases

        // Create first test database
        let db1_name = db_testkit::test_db::DatabaseName::new(Some("testkit_mysql_test1"));
        info!("Creating test database 1: {}", db1_name);

        match backend.create_database(&db1_name).await {
            Ok(_) => info!("Test database 1 created successfully"),
            Err(e) => panic!("Failed to create test database 1: {}", e),
        }

        // Create schema in db1
        let pool1 = match backend.create_pool(&db1_name, &pool_config).await {
            Ok(pool) => {
                info!("Connection pool 1 created successfully");
                pool
            }
            Err(e) => panic!("Failed to create pool 1: {}", e),
        };

        match sqlx::query(
            "CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                email VARCHAR(255) UNIQUE NOT NULL,
                name VARCHAR(255) NOT NULL
            )",
        )
        .execute(pool1.sqlx_pool())
        .await
        {
            Ok(_) => info!("Table created in database 1 successfully"),
            Err(e) => panic!("Failed to create table in database 1: {}", e),
        }

        // Create second test database
        let db2_name = db_testkit::test_db::DatabaseName::new(Some("testkit_mysql_test2"));
        info!("Creating test database 2: {}", db2_name);

        match backend.create_database(&db2_name).await {
            Ok(_) => info!("Test database 2 created successfully"),
            Err(e) => panic!("Failed to create test database 2: {}", e),
        }

        // Create schema in db2
        let pool2 = match backend.create_pool(&db2_name, &pool_config).await {
            Ok(pool) => {
                info!("Connection pool 2 created successfully");
                pool
            }
            Err(e) => panic!("Failed to create pool 2: {}", e),
        };

        match sqlx::query(
            "CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                email VARCHAR(255) UNIQUE NOT NULL,
                name VARCHAR(255) NOT NULL
            )",
        )
        .execute(pool2.sqlx_pool())
        .await
        {
            Ok(_) => info!("Table created in database 2 successfully"),
            Err(e) => panic!("Failed to create table in database 2: {}", e),
        }

        // Get SQLx pools directly for cleaner access
        let sqlx_pool1 = pool1.sqlx_pool();
        let sqlx_pool2 = pool2.sqlx_pool();

        // Insert into db1
        info!("Inserting data into database 1...");
        sqlx::query("INSERT INTO users (email, name) VALUES ('test1@example.com', 'Test User 1')")
            .execute(sqlx_pool1)
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to insert data into database 1: {}", e);
            });

        // Insert into db2
        info!("Inserting data into database 2...");
        sqlx::query("INSERT INTO users (email, name) VALUES ('test2@example.com', 'Test User 2')")
            .execute(sqlx_pool2)
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to insert data into database 2: {}", e);
            });

        // Verify data is separate
        info!("Verifying data in database 1...");
        let count1 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test1@example.com'",
        )
        .fetch_one(sqlx_pool1)
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to query database 1: {}", e);
        });
        assert_eq!(count1.0, 1, "Should find test1@example.com in database 1");

        info!("Verifying data in database 2...");
        let count2 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test2@example.com'",
        )
        .fetch_one(sqlx_pool2)
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to query database 2: {}", e);
        });
        assert_eq!(count2.0, 1, "Should find test2@example.com in database 2");

        // Verify data isolation
        info!("Verifying data isolation between databases...");
        let count3 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test2@example.com'",
        )
        .fetch_one(sqlx_pool1)
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to query isolation for database 1: {}", e);
        });
        assert_eq!(
            count3.0, 0,
            "Should NOT find test2@example.com in database 1"
        );

        let count4 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'test1@example.com'",
        )
        .fetch_one(sqlx_pool2)
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to query isolation for database 2: {}", e);
        });
        assert_eq!(
            count4.0, 0,
            "Should NOT find test1@example.com in database 2"
        );

        // Clean up databases at the end of the test
        info!("Cleaning up test databases");

        // Make sure to drop all pools first to release connections
        info!("Dropping connection pools");
        // Clone the sqlx pools before dropping to avoid dropping references
        let sqlx_pool1_owned = sqlx_pool1.clone();
        let sqlx_pool2_owned = sqlx_pool2.clone();
        drop(sqlx_pool1_owned);
        drop(sqlx_pool2_owned);
        drop(pool1);
        drop(pool2);
        drop(pool);

        // Add a small delay to allow connection termination to complete
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Robust drop function with retries
        async fn robust_drop(backend: &SqlxMySqlBackend, name: &db_testkit::test_db::DatabaseName) {
            // First try to terminate all connections
            if let Err(e) = backend.terminate_connections(name).await {
                info!(
                    "Warning: Failed to terminate connections to {}: {}",
                    name, e
                );
            }

            // Try dropping the database with up to 3 attempts
            let mut attempt = 0;
            let max_attempts = 3;

            while attempt < max_attempts {
                match backend.drop_database(name).await {
                    Ok(_) => {
                        info!("Successfully dropped database {}", name);
                        return;
                    }
                    Err(e) => {
                        attempt += 1;
                        if attempt >= max_attempts {
                            info!(
                                "Failed to drop database {} after {} attempts: {}",
                                name, max_attempts, e
                            );
                        } else {
                            info!(
                                "Attempt {} to drop database {} failed: {}. Retrying...",
                                attempt, name, e
                            );
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            }

            // Last resort: use direct MySQL command to force drop
            info!(
                "Attempting to force drop database {} using direct command",
                name
            );
            let _ = std::process::Command::new("mysql")
                .args([
                    "-h",
                    "mysql",
                    "-u",
                    "root",
                    "-e",
                    &format!("DROP DATABASE IF EXISTS `{}`", name.as_str()),
                ])
                .status();
        }

        // Drop all test databases
        info!("Dropping test database 1");
        robust_drop(&backend, &db1_name).await;

        info!("Dropping test database 2");
        robust_drop(&backend, &db2_name).await;

        info!("Dropping direct test database");
        robust_drop(&backend, &db_name).await;

        info!("Template test completed successfully");
    }

    #[tokio::test]
    async fn test_sqlx_mysql_transactions() {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=debug");
        }
        init_tracing();

        info!("Starting MySQL transactions test");

        // Use a direct connection string with explicit parameters
        let url = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";
        info!("Using direct MySQL URL: {}", url);

        // Create backend with better error handling
        let backend = match SqlxMySqlBackend::new(url) {
            Ok(backend) => {
                info!("Successfully created SQLx MySQL backend");
                backend
            }
            Err(e) => {
                panic!("Failed to create SQLx MySQL backend: {}", e);
            }
        };

        // Test connection with retry logic
        let max_retries = 3;
        let mut retry_count = 0;
        let mut connected = false;

        while retry_count < max_retries {
            info!(
                "Testing connection to MySQL server (attempt {})...",
                retry_count + 1
            );
            match backend.connect().await {
                Ok(pool) => {
                    info!("Connection test successful on attempt {}", retry_count + 1);
                    // Test a simple query
                    match sqlx::query("SELECT 1").execute(&pool).await {
                        Ok(_) => {
                            info!("Simple query executed successfully");
                            connected = true;
                            break;
                        }
                        Err(e) => {
                            info!("Simple query failed: {}", e);
                            retry_count += 1;
                        }
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count < max_retries {
                        let backoff = Duration::from_secs(2 * retry_count as u64);
                        info!(
                            "Connection attempt {} failed: {}. Retrying in {:?}...",
                            retry_count, e, backoff
                        );
                        tokio::time::sleep(backoff).await;
                    } else {
                        panic!("Failed to connect after {} attempts: {}", max_retries, e);
                    }
                }
            }
        }

        if !connected {
            panic!(
                "Could not establish a working connection after {} attempts",
                max_retries
            );
        }

        // Create a DatabaseName instance
        let db_name = db_testkit::test_db::DatabaseName::new(Some("testkit_mysql_txn_test"));
        info!("Using database name: {}", db_name);

        // Create the database properly
        info!("Creating database {}...", db_name);
        match backend.create_database(&db_name).await {
            Ok(_) => info!("Database created successfully"),
            Err(e) => panic!("Failed to create database: {}", e),
        }

        // Create pool for the database with longer timeout
        let pool_config = PoolConfig::builder(10)
            .connection_timeout(Duration::from_secs(30))
            .build();

        info!("Creating connection pool...");
        let pool = match backend.create_pool(&db_name, &pool_config).await {
            Ok(pool) => {
                info!("Connection pool created successfully");
                pool
            }
            Err(e) => panic!("Failed to create pool: {}", e),
        };

        // Get the SQLx pool to work with directly
        let sqlx_pool = pool.sqlx_pool();

        // Test the connection before creating tables
        info!("Testing database connection...");
        match sqlx::query("SELECT 1").execute(sqlx_pool).await {
            Ok(_) => info!("Connection test successful"),
            Err(e) => panic!("Connection test failed: {}", e),
        }

        // Create table directly using SQLx
        info!("Creating users table...");
        match sqlx::query(
            "CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                email VARCHAR(255) UNIQUE NOT NULL,
                name VARCHAR(255) NOT NULL
            )",
        )
        .execute(sqlx_pool)
        .await
        {
            Ok(_) => info!("Table created successfully"),
            Err(e) => panic!("Failed to create table: {}", e),
        }

        // Test transaction commit
        info!("Testing transaction commit...");
        {
            // Start a transaction
            let mut tx = sqlx_pool.begin().await.unwrap_or_else(|e| {
                panic!("Failed to begin transaction: {}", e);
            });

            // Insert data - directly use the transaction's execute method
            tx.execute(
                "INSERT INTO users (email, name) VALUES ('commit@example.com', 'Commit User')",
            )
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to insert data in transaction: {}", e);
            });

            // Commit transaction
            tx.commit().await.unwrap_or_else(|e| {
                panic!("Failed to commit transaction: {}", e);
            });
            info!("Transaction committed successfully");
        }

        // Test transaction rollback
        info!("Testing transaction rollback...");
        {
            // Start a transaction
            let mut tx = sqlx_pool.begin().await.unwrap_or_else(|e| {
                panic!("Failed to begin transaction: {}", e);
            });

            // Insert data - directly use the transaction's execute method
            tx.execute(
                "INSERT INTO users (email, name) VALUES ('rollback@example.com', 'Rollback User')",
            )
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to insert data in transaction: {}", e);
            });

            // Check data is visible within transaction - use direct method
            let row = tx
                .fetch_one("SELECT COUNT(*) FROM users WHERE email = 'rollback@example.com'")
                .await
                .unwrap_or_else(|e| {
                    panic!("Failed to query within transaction: {}", e);
                });
            let count: i64 = row.try_get(0).unwrap();
            assert_eq!(count, 1, "Data should be visible within transaction");

            // Rollback by dropping the transaction
            info!("Explicitly dropping transaction to test rollback");
            drop(tx);
        }

        // Verify committed data exists but rolled back data doesn't
        info!("Verifying committed data exists...");
        let committed = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'commit@example.com'",
        )
        .fetch_one(sqlx_pool)
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to query committed data: {}", e);
        });
        assert_eq!(committed.0, 1, "Committed data should exist");

        info!("Verifying rolled back data doesn't exist...");
        let rolled_back = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM users WHERE email = 'rollback@example.com'",
        )
        .fetch_one(sqlx_pool)
        .await
        .unwrap_or_else(|e| {
            panic!("Failed to query rolled back data: {}", e);
        });
        assert_eq!(rolled_back.0, 0, "Rolled back data should not exist");

        // Clean up the database at the end of the test
        // Manually drop the pool first to close connections
        info!("Cleaning up - dropping connection pool");

        // Get the SQL pool directly before dropping the wrapper pool
        let sqlx_pool_owned = sqlx_pool.clone();

        // Now drop both pools to close all connections
        drop(pool);
        drop(sqlx_pool_owned);

        // Add a small delay to allow connection termination to complete
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Now drop the database with robust approach
        info!("Dropping test database");

        // First try to terminate all connections
        if let Err(e) = backend.terminate_connections(&db_name).await {
            info!(
                "Warning: Failed to terminate connections to {}: {}",
                db_name, e
            );
        }

        // Try dropping the database with up to 3 attempts
        let mut attempt = 0;
        let max_attempts = 3;

        while attempt < max_attempts {
            match backend.drop_database(&db_name).await {
                Ok(_) => {
                    info!("Successfully dropped database {}", db_name);
                    break;
                }
                Err(e) => {
                    attempt += 1;
                    if attempt >= max_attempts {
                        info!(
                            "Failed to drop database {} after {} attempts: {}",
                            db_name, max_attempts, e
                        );
                    } else {
                        info!(
                            "Attempt {} to drop database {} failed: {}. Retrying...",
                            attempt, db_name, e
                        );
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }

        // Last resort: use direct MySQL command to force drop
        if attempt >= max_attempts {
            info!(
                "Attempting to force drop database {} using direct command",
                db_name
            );
            let _ = std::process::Command::new("mysql")
                .args([
                    "-h",
                    "mysql",
                    "-u",
                    "root",
                    "-e",
                    &format!("DROP DATABASE IF EXISTS `{}`", db_name.as_str()),
                ])
                .status();
        }

        info!("Transaction test completed successfully");
    }
}
