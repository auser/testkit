#![allow(clippy::all, unused_must_use, unused_lifetimes)]
#![cfg(feature = "postgres")] // This file is specific to tokio-postgres backend

use std::future::Future;
use std::pin::Pin;
use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabasePool, TestDatabaseInstance, with_boxed_database,
};
use testkit_postgres::{PostgresBackend, PostgresError, postgres_backend_with_config};

// Helper function to create a test config with the correct hostname
#[allow(dead_code)]
fn test_config() -> DatabaseConfig {
    let admin_url = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    let user_url = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    DatabaseConfig::new(admin_url, user_url)
}

// Helper function to check if an error is a connection error
#[allow(dead_code)]
fn is_connection_error(err: &PostgresError) -> bool {
    let err_str = err.to_string();
    err_str.contains("connection refused")
        || err_str.contains("timeout")
        || err_str.contains("does not exist")
        || err_str.contains("pool timed out")
}

// Helper to create a test backend
#[allow(dead_code)]
async fn test_backend() -> Result<PostgresBackend, PostgresError> {
    let config = test_config();
    postgres_backend_with_config(config).await
}

// Helper function to create a boxed future
#[allow(dead_code)]
fn boxed_future<T, F, Fut>(
    f: F,
) -> impl FnOnce(T) -> Pin<Box<dyn Future<Output = Result<(), PostgresError>> + Send>>
where
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), PostgresError>> + Send + 'static,
    T: Send + 'static,
{
    move |t| Box::pin(f(t))
}

#[tokio::test]
async fn test_postgres_backend() {
    // Use "postgres" as the hostname for the Docker container
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );

    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Test creating a test database
    let ctx = with_boxed_database(backend)
        .execute()
        .await
        .expect("Failed to create database");

    // Confirm we have a database name
    assert!(!ctx.db.db_name.to_string().is_empty());
}

#[tokio::test]
async fn test_setup_database() {
    // Use "postgres" as the hostname for the Docker container
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );

    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database with setup function
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a table for testing
                conn.client()
                    .execute(
                        "CREATE TABLE test_table (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;

                // Verify the table exists with a query
                let result = conn
                    .client()
                    .query(
                        "SELECT EXISTS (
                            SELECT FROM information_schema.tables 
                            WHERE table_name = 'test_table'
                        )",
                        &[],
                    )
                    .await?;

                let exists: bool = result[0].get(0);
                assert!(exists, "Table should exist after creation");

                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to setup database");

    // Verify we can get a connection
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to get connection");

    // Verify our table exists
    let result = conn
        .client()
        .query(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_name = 'test_table'
            )",
            &[],
        )
        .await
        .expect("Failed to query tables");

    let exists: bool = result[0].get(0);
    assert!(exists, "Table should exist");
}

#[tokio::test]
async fn test_transaction() {
    // Use "postgres" as the hostname for the Docker container
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );

    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database with setup and then transaction
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a table for testing
                conn.client()
                    .execute(
                        "CREATE TABLE test_table (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;
                Ok(())
            })
        })
        .with_transaction(|conn| {
            Box::pin(async move {
                // Start a transaction
                conn.client().execute("BEGIN", &[]).await?;

                // Insert data
                conn.client()
                    .execute(
                        "INSERT INTO test_table (value) VALUES ($1)",
                        &[&"test value"],
                    )
                    .await?;

                // Verify data exists in transaction
                let rows = conn.client().query("SELECT * FROM test_table", &[]).await?;
                assert_eq!(rows.len(), 1, "Should have inserted 1 row");

                // Commit the transaction
                conn.client().execute("COMMIT", &[]).await?;

                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to execute transaction");

    // Get a connection
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to get connection");

    // Verify data exists after transaction
    let rows = conn
        .client()
        .query("SELECT * FROM test_table", &[])
        .await
        .expect("Failed to query table");

    assert_eq!(rows.len(), 1, "Data should exist after transaction");
}

#[tokio::test]
async fn test_transaction_rollback() {
    // Use "postgres" as the hostname for the Docker container
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );

    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database with setup and then transaction
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a table for testing
                conn.client()
                    .execute(
                        "CREATE TABLE test_table (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;
                Ok(())
            })
        })
        .with_transaction(|conn| {
            Box::pin(async move {
                // Begin transaction
                conn.client().execute("BEGIN", &[]).await?;

                // Insert data
                conn.client()
                    .execute(
                        "INSERT INTO test_table (value) VALUES ($1)",
                        &[&"will be rolled back"],
                    )
                    .await?;

                // Verify data exists in transaction
                let rows = conn.client().query("SELECT * FROM test_table", &[]).await?;
                assert_eq!(rows.len(), 1, "Should have inserted 1 row");

                // Roll back the transaction
                conn.client().execute("ROLLBACK", &[]).await?;

                // Verify data was rolled back
                let rows = conn.client().query("SELECT * FROM test_table", &[]).await?;
                assert_eq!(rows.len(), 0, "Data should be rolled back");

                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to execute transaction");

    // Get a connection
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to get connection");

    // Verify data doesn't exist after rollback
    let rows = conn
        .client()
        .query("SELECT * FROM test_table", &[])
        .await
        .expect("Failed to query table");

    assert_eq!(rows.len(), 0, "Data should not exist after rollback");
}

#[tokio::test]
async fn test_multiple_databases() {
    // Use "postgres" as the hostname for the Docker container
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );

    // Create two backends
    let backend1 = postgres_backend_with_config(config.clone())
        .await
        .expect("Failed to create first backend");

    let backend2 = postgres_backend_with_config(config)
        .await
        .expect("Failed to create second backend");

    // Create first database with a table
    let ctx1 = with_boxed_database(backend1)
        .setup(|conn| {
            Box::pin(async move {
                conn.client()
                    .execute(
                        "CREATE TABLE db1_table (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;
                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to create first database");

    // Create second database with a different table
    let ctx2 = with_boxed_database(backend2)
        .setup(|conn| {
            Box::pin(async move {
                conn.client()
                    .execute(
                        "CREATE TABLE db2_table (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;
                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to create second database");

    // Verify databases are separate
    let conn1 = ctx1
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to connect to db1");
    let conn2 = ctx2
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to connect to db2");

    // db1 should have db1_table but not db2_table
    let result = conn1
        .client()
        .query(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_name = 'db1_table'
            )",
            &[],
        )
        .await
        .expect("Failed to query db1");

    let db1_has_db1_table: bool = result[0].get(0);
    assert!(db1_has_db1_table, "db1 should have db1_table");

    let result = conn1
        .client()
        .query(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_name = 'db2_table'
            )",
            &[],
        )
        .await
        .expect("Failed to query db1");

    let db1_has_db2_table: bool = result[0].get(0);
    assert!(!db1_has_db2_table, "db1 should not have db2_table");

    // db2 should have db2_table but not db1_table
    let result = conn2
        .client()
        .query(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_name = 'db2_table'
            )",
            &[],
        )
        .await
        .expect("Failed to query db2");

    let db2_has_db2_table: bool = result[0].get(0);
    assert!(db2_has_db2_table, "db2 should have db2_table");

    let result = conn2
        .client()
        .query(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_name = 'db1_table'
            )",
            &[],
        )
        .await
        .expect("Failed to query db2");

    let db2_has_db1_table: bool = result[0].get(0);
    assert!(!db2_has_db1_table, "db2 should not have db1_table");
}

#[tokio::test]
async fn test_boxed_database_api() {
    // Use "postgres" as the hostname for the Docker container
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );

    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a local variable to capture in the setup closure
    let table_name = String::from("test_table");
    let table_name_clone = table_name.clone(); // Clone the value to avoid move

    // Create a database using the boxed API to handle the captured variable
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a table using our captured variable
                let create_query = format!(
                    "CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT)",
                    table_name_clone
                );
                conn.client().execute(&create_query, &[]).await?;

                // Insert some test data
                let insert_query = format!("INSERT INTO {} (value) VALUES ($1)", table_name_clone);
                conn.client()
                    .execute(&insert_query, &[&"test value"])
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to create context with test database");

    println!(
        "Created test database for transaction test: {}",
        ctx.db.name()
    );

    // Verify the table exists with the expected name
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to get connection");

    // Verify the table exists
    let query = format!(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_name = '{}'
        )",
        table_name
    );

    let result = conn
        .client()
        .query(&query, &[])
        .await
        .expect("Failed to query tables");

    let exists: bool = result[0].get(0);
    assert!(exists, "Table should exist");

    // Now check if our data is there
    let query = format!("SELECT * FROM {}", table_name);
    let rows = conn
        .client()
        .query(&query, &[])
        .await
        .expect("Failed to query table");

    assert_eq!(rows.len(), 1, "Should have one row");
    assert_eq!(rows[0].get::<&str, String>("value"), "test value");
}

#[tokio::test]
async fn test_basic_connection() {
    // Create the backend with our test config
    let backend = match postgres_backend_with_config(test_config()).await {
        Ok(b) => b,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };

    // Create a test database instance with the backend and config
    let db = match TestDatabaseInstance::new(backend.clone(), test_config()).await {
        Ok(db) => db,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create test database: {:?}", e);
        }
    };

    println!(
        "Created test database for basic connection test: {}",
        db.name()
    );

    // Get a connection from the pool
    let conn = match db.pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => panic!("Failed to acquire connection: {:?}", e),
    };

    // Create a test table
    match conn
        .client()
        .execute(
            "CREATE TABLE test_table (id SERIAL PRIMARY KEY, value TEXT)",
            &[],
        )
        .await
    {
        Ok(_) => {}
        Err(e) => panic!("Failed to create table: {:?}", e),
    }

    // Insert some test data
    match conn
        .client()
        .execute(
            "INSERT INTO test_table (value) VALUES ($1)",
            &[&"test value"],
        )
        .await
    {
        Ok(_) => {}
        Err(e) => panic!("Failed to insert data: {:?}", e),
    }

    // Query the data
    let rows = match conn
        .client()
        .query("SELECT value FROM test_table", &[])
        .await
    {
        Ok(rows) => rows,
        Err(e) => panic!("Failed to query data: {:?}", e),
    };

    assert_eq!(rows.len(), 1, "Should have one row");
    assert_eq!(rows[0].get::<&str, String>("value"), "test value");
}

#[tokio::test]
async fn test_with_connection() {
    // Create a backend
    let backend = match test_backend().await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };

    // Create a database with a test table
    let ctx = match with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a test table and insert data
                conn.client()
                    .execute(
                        "CREATE TABLE one_off_test (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;

                conn.client()
                    .execute(
                        "INSERT INTO one_off_test (value) VALUES ($1)",
                        &[&"test_value"],
                    )
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create database: {:?}", e);
        }
    };

    // Test with_connection functionality
    let conn_string = ctx.db.backend().connection_string(ctx.db.name());
    println!("Debug: Using connection string: {}", conn_string);

    // Use one-off connection to verify data
    let result = testkit_postgres::with_postgres_connection(conn_string, |conn| {
        Box::pin(async move {
            let rows = conn
                .client()
                .query("SELECT * FROM one_off_test", &[])
                .await
                .map_err(|e| PostgresError::QueryError(e.to_string()))?;

            assert_eq!(rows.len(), 1, "Expected 1 row");
            Ok::<_, PostgresError>(())
        })
    })
    .await;

    assert!(result.is_ok(), "with_postgres_connection should succeed");
}

#[tokio::test]
async fn test_postgres_with_connection() {
    // Create a backend
    let backend = match test_backend().await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };

    // Create a database with a test table
    let ctx = match with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a test table and insert data
                conn.client()
                    .execute(
                        "CREATE TABLE one_off_test (id SERIAL PRIMARY KEY, value TEXT)",
                        &[],
                    )
                    .await?;

                conn.client()
                    .execute(
                        "INSERT INTO one_off_test (value) VALUES ($1)",
                        &[&"test_value"],
                    )
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create database: {:?}", e);
        }
    };

    // Test with_connection functionality
    let conn_string = ctx.db.backend().connection_string(ctx.db.name());
    println!("Debug: Using connection string: {}", conn_string);

    // Use one-off connection to verify data
    let result = testkit_postgres::with_postgres_connection(conn_string, |conn| {
        Box::pin(async move {
            let rows = conn
                .client()
                .query("SELECT * FROM one_off_test", &[])
                .await
                .map_err(|e| PostgresError::QueryError(e.to_string()))?;

            assert_eq!(rows.len(), 1, "Expected 1 row");
            Ok::<_, PostgresError>(())
        })
    })
    .await;

    assert!(result.is_ok(), "with_postgres_connection should succeed");
}
