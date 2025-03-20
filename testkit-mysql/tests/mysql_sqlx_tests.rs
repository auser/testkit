#![cfg(feature = "with-sqlx")]
use sqlx::{Row, query, query_as};
use testkit_core::{DatabaseConfig, DatabasePool, boxed_async, with_boxed_database};
use testkit_mysql::{MySqlError, sqlx_mysql_backend_with_config};

fn test_config() -> DatabaseConfig {
    DatabaseConfig::new(
        std::env::var("ADMIN_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://root:password@mysql:3306/mysql".to_string()),
        std::env::var("MYSQL_URL")
            .unwrap_or_else(|_| "mysql://testuser:password@mysql:3306/mysql".to_string()),
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
async fn test_sqlx_backend_creation() {
    let config = test_config();
    match sqlx_mysql_backend_with_config(config).await {
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
async fn test_sqlx_backend_with_table() {
    let config = test_config();
    let backend = match sqlx_mysql_backend_with_config(config).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };

    // Test creating a database
    let ctx = match with_boxed_database(backend)
        .setup(|conn| {
            boxed_async!(async move {
                // Create a test table
                query(
                "CREATE TABLE test_table (id INT AUTO_INCREMENT PRIMARY KEY, value VARCHAR(255))",
            )
            .execute(&*conn.pool())
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

    // Test using the database
    let conn = ctx.db.pool.acquire().await.unwrap();

    // Insert some data
    query("INSERT INTO test_table (value) VALUES (?)")
        .bind("test value")
        .execute(&*conn.pool())
        .await
        .unwrap();

    // Query the data with type mapping
    let row = query("SELECT id, value FROM test_table")
        .fetch_one(&*conn.pool())
        .await
        .unwrap();

    let id: i32 = row.get(0);
    let value: String = row.get(1);

    assert_eq!(id, 1, "ID should be 1");
    assert_eq!(value, "test value", "Value should match what was inserted");
}

#[tokio::test]
async fn test_sqlx_transaction() {
    let config = test_config();
    let backend = match sqlx_mysql_backend_with_config(config).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };

    // Test database with transaction
    let ctx = match with_boxed_database(backend)
        .setup(|conn| boxed_async!(async move {
            // Create a test table
            query("CREATE TABLE test_transaction (id INT AUTO_INCREMENT PRIMARY KEY, value VARCHAR(255))")
                .execute(&*conn.pool())
                .await
                .unwrap();
            Ok(())
        }))
        .with_transaction(|conn| boxed_async!(async move {
            // This data will be rolled back after the test
            query("INSERT INTO test_transaction (value) VALUES (?)")
                .bind("will be rolled back")
                .execute(&*conn.pool())
                .await
                .unwrap();
            Ok(())
        }))
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

    // Test that transaction was committed during test
    let conn = ctx.db.pool.acquire().await.unwrap();

    let row = query_as::<_, (i64,)>("SELECT COUNT(*) FROM test_transaction")
        .fetch_one(&*conn.pool())
        .await
        .unwrap();

    assert_eq!(row.0, 1, "Should have one record during test");

    // After the test context is dropped, the transaction should be rolled back
    // We'd need a separate connection to verify this
}

#[tokio::test]
async fn test_multiple_sqlx_databases() {
    let config = test_config();

    // Create two separate database instances
    let backend1 = match sqlx_mysql_backend_with_config(config.clone()).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create first MySQL backend: {:?}", e);
        }
    };

    let backend2 = match sqlx_mysql_backend_with_config(config).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create second MySQL backend: {:?}", e);
        }
    };

    // Set up first database
    let ctx1 = match with_boxed_database(backend1)
        .setup(|conn| boxed_async!(async move {
            query("CREATE TABLE db1_table (id INT AUTO_INCREMENT PRIMARY KEY, value VARCHAR(255))")
                .execute(&*conn.pool())
                .await
                .unwrap();

            query("INSERT INTO db1_table (value) VALUES (?)")
                .bind("db1 value")
                .execute(&*conn.pool())
                .await
                .unwrap();

            Ok(())
        }))
        .execute()
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to setup first database: {:?}", e);
        }
    };

    // Set up second database
    let ctx2 = match with_boxed_database(backend2)
        .setup(|conn| boxed_async!(async move {
            query("CREATE TABLE db2_table (id INT AUTO_INCREMENT PRIMARY KEY, value VARCHAR(255))")
                .execute(&*conn.pool())
                .await
                .unwrap();

            query("INSERT INTO db2_table (value) VALUES (?)")
                .bind("db2 value")
                .execute(&*conn.pool())
                .await
                .unwrap();

            Ok(())
        }))
        .execute()
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to setup second database: {:?}", e);
        }
    };

    // Verify data in first database
    let conn1 = ctx1.db.pool.acquire().await.unwrap();

    let row1 = query_as::<_, (String,)>("SELECT value FROM db1_table")
        .fetch_one(&*conn1.pool())
        .await
        .unwrap();

    assert_eq!(row1.0, "db1 value", "First DB value should match");

    // Try to access the second database's table (should fail)
    let result = query("SELECT * FROM db2_table")
        .execute(&*conn1.pool())
        .await;

    assert!(
        result.is_err(),
        "Should not be able to access tables from another isolated database"
    );

    // Verify data in second database
    let conn2 = ctx2.db.pool.acquire().await.unwrap();

    let row2 = query_as::<_, (String,)>("SELECT value FROM db2_table")
        .fetch_one(&*conn2.pool())
        .await
        .unwrap();

    assert_eq!(row2.0, "db2 value", "Second DB value should match");

    // Try to access the first database's table (should fail)
    let result = query("SELECT * FROM db1_table")
        .execute(&*conn2.pool())
        .await;

    assert!(
        result.is_err(),
        "Should not be able to access tables from another isolated database"
    );
}

#[tokio::test]
async fn test_sqlx_prepared_statements() {
    let config = test_config();
    let backend = match sqlx_mysql_backend_with_config(config).await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: MySQL appears to be unavailable");
                return;
            }
            panic!("Failed to create MySQL backend: {:?}", e);
        }
    };

    // Test creating a database
    let ctx = match with_boxed_database(backend)
        .setup(|conn| boxed_async!(async move {
            // Create a test table
            query("CREATE TABLE users (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), email VARCHAR(255), age INT)")
                .execute(&*conn.pool())
                .await
                .unwrap();
            Ok(())
        }))
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

    // Test using the database with prepared statements
    let conn = ctx.db.pool.acquire().await.unwrap();

    // Insert multiple users
    for (name, email, age) in [
        ("Alice", "alice@example.com", 30),
        ("Bob", "bob@example.com", 25),
        ("Charlie", "charlie@example.com", 35),
    ] {
        query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
            .bind(name)
            .bind(email)
            .bind(age)
            .execute(&*conn.pool())
            .await
            .unwrap();
    }

    // Query with a filter
    let rows =
        query_as::<_, (String, i32)>("SELECT name, age FROM users WHERE age > ? ORDER BY age")
            .bind(27)
            .fetch_all(&*conn.pool())
            .await
            .unwrap();

    assert_eq!(rows.len(), 2, "Should have two users with age > 27");
    assert_eq!(rows[0].0, "Alice", "First user should be Alice");
    assert_eq!(rows[0].1, 30, "Alice's age should be 30");
    assert_eq!(rows[1].0, "Charlie", "Second user should be Charlie");
    assert_eq!(rows[1].1, 35, "Charlie's age should be 35");
}
