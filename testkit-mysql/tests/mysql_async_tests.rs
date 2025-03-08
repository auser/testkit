#![cfg(feature = "with-mysql-async")]
use mysql_async::prelude::*;
use testkit_core::{DatabaseConfig, TestDatabaseInstance, boxed_async, with_boxed_database};
use testkit_mysql::{DatabasePool, MySqlError, mysql_backend_with_config};

fn test_config() -> DatabaseConfig {
    DatabaseConfig::new(
        std::env::var("ADMIN_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://root:root@mysql:3306/mysql".to_string()),
        std::env::var("MYSQL_URL").unwrap_or_else(|_| "mysql://root:root@mysql:3306".to_string()),
    )
}

fn is_connection_error(err: &MySqlError) -> bool {
    match err {
        MySqlError::ConnectionError(_) => true,
        MySqlError::Generic(msg) => {
            msg.contains("connection")
                || msg.contains("network")
                || msg.contains("timeout")
                || msg.contains("refused")
        }
        _ => false,
    }
}

#[tokio::test]
#[ignore]
async fn test_mysql_backend_creation() {
    let config = test_config();
    let _backend = match mysql_backend_with_config(config).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };
}

#[tokio::test]
#[ignore]
async fn test_mysql_backend_with_table() {
    let config = test_config();
    let backend = match mysql_backend_with_config(config.clone()).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };

    // Create a database instance directly
    let db = match TestDatabaseInstance::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create test database: {:?}", e);
        }
    };

    // Get a connection
    let conn = db.pool.acquire().await.unwrap();

    // Create schema
    conn.query_drop(
        "CREATE TABLE test_table (id INT AUTO_INCREMENT PRIMARY KEY, value VARCHAR(255))",
    )
    .await
    .unwrap();

    // Insert data
    conn.exec_drop("INSERT INTO test_table (value) VALUES (?)", ("test value",))
        .await
        .unwrap();

    // Query data
    let result = conn
        .query_map("SELECT id, value FROM test_table", |row| {
            let id: i32 = row.get(0).unwrap();
            let value: String = row.get(1).unwrap();
            (id, value)
        })
        .await
        .unwrap();

    // Assertions
    assert_eq!(result.len(), 1, "Should have one record");
    let (id, value) = result[0].clone();
    assert_eq!(id, 1, "ID should be 1");
    assert_eq!(value, "test value", "Value should match what was inserted");
}

#[tokio::test]
#[ignore]
async fn test_mysql_transaction() {
    let config = test_config();
    let backend = match mysql_backend_with_config(config.clone()).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };

    // Create a database instance directly
    let db = match TestDatabaseInstance::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create test database: {:?}", e);
        }
    };

    // Get a connection and access the underlying mysql_async connection
    let conn = db.pool.acquire().await.unwrap();

    // Drop the table first if it exists
    conn.query_drop("DROP TABLE IF EXISTS test_transaction")
        .await
        .unwrap();

    // Create schema
    conn.query_drop(
        "CREATE TABLE test_transaction (id INT AUTO_INCREMENT PRIMARY KEY, value VARCHAR(255))",
    )
    .await
    .unwrap();

    // Get the raw mysql_async connection
    let mut raw_conn = { db.pool.pool.get_conn().await.unwrap() };

    // Start a transaction using the native mysql_async API
    let mut transaction = raw_conn
        .start_transaction(mysql_async::TxOpts::default())
        .await
        .unwrap();

    // Insert data in transaction
    transaction
        .exec_drop(
            "INSERT INTO test_transaction (value) VALUES (?)",
            ("will be rolled back",),
        )
        .await
        .unwrap();

    // Query within transaction
    let count: i64 = transaction
        .query_first("SELECT COUNT(*) FROM test_transaction")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(count, 1, "Should have one record in the transaction");

    // Rollback transaction
    transaction.rollback().await.unwrap();

    // Verify rollback
    let count: Option<i64> = raw_conn
        .query_first("SELECT COUNT(*) FROM test_transaction")
        .await
        .unwrap();

    assert_eq!(
        count.unwrap_or(0),
        0,
        "Should have no records after rollback"
    );
}

#[tokio::test]
#[ignore]
async fn test_mysql_fluent_api() {
    let config = test_config();
    let backend = match mysql_backend_with_config(config).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };

    // Use the fluent API with boxed_async macro for clean syntax
    let ctx = match with_boxed_database(backend)
        .setup(|conn| {
            boxed_async!(async move {
                // Create a products table
                conn.query_drop(
                    "CREATE TABLE products (
                        id INT AUTO_INCREMENT PRIMARY KEY,
                        name VARCHAR(255) NOT NULL,
                        price DECIMAL(10,2) NOT NULL
                    )",
                )
                .await
                .unwrap();
                Ok(())
            })
        })
        .with_transaction(|conn| {
            boxed_async!(async move {
                // Insert test products - these will be rolled back after test
                conn.exec_drop(
                    "INSERT INTO products (name, price) VALUES (?, ?), (?, ?)",
                    ("Widget", 19.99, "Gadget", 24.95),
                )
                .await
                .unwrap();
                Ok(())
            })
        })
        .execute()
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to setup test database: {:?}", e);
        }
    };

    // Verify products exist during the test
    let conn = ctx.db.pool.acquire().await.unwrap();

    let products = conn
        .query_map("SELECT id, name, price FROM products ORDER BY id", |row| {
            let id: i32 = row.get(0).unwrap();
            let name: String = row.get(1).unwrap();
            let price: f64 = row.get(2).unwrap();
            (id, name, price)
        })
        .await
        .unwrap();

    // Verify test data
    assert_eq!(products.len(), 2, "Should have two products");

    let (id1, name1, price1) = &products[0];
    assert_eq!(*id1, 1, "First product ID should be 1");
    assert_eq!(name1, "Widget", "First product name should be Widget");
    assert!(
        (price1 - 19.99).abs() < 0.01,
        "First product price should be 19.99"
    );

    let (id2, name2, price2) = &products[1];
    assert_eq!(*id2, 2, "Second product ID should be 2");
    assert_eq!(name2, "Gadget", "Second product name should be Gadget");
    assert!(
        (price2 - 24.95).abs() < 0.01,
        "Second product price should be 24.95"
    );

    // When the ctx is dropped, the transaction is automatically rolled back
    // and the database is dropped, cleanup is automatic!
}
